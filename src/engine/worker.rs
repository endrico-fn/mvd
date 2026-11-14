use crate::app::message::{Command, Message};
use crate::config::xdg::XdgPaths;
use crate::config::AppConfig;
use crate::domain::error::Severity;
use crate::domain::media::{
    AspectRatioKind, ContainerFormat, MediaArchetype, PlatformKind, StreamCandidate,
};
use crate::domain::metadata::EditableMetadata;
use crate::domain::playlist::{GlobalPlaylistPolicy, PlaylistItemEntry, PlaylistWorkspace};
use crate::domain::queue_item::QueueJob;
use crate::domain::subtitle::SubtitleResolutionPlan;
use crate::engine::lrclib::{LrclibCleaner, LyricsDocument};
use crate::engine::parser::{ParsedMediaUrl, UrlParser};
use crate::engine::spotify::{SpotifyBridge, SpotifyTrackMetadata, YtSearchAudioCandidate};
use crate::engine::tagger::AudioMetadataTagger;
use crate::engine::ytdlp::{SmartThumbnailSelector, YtDlpThumbnailJson};
use crate::security::cookie_vault::{CookieVault, EphemeralCookieFileGuard};
use crate::security::profile_guard::ProfileGuard;
use crate::storage::cache::BoundedCacheManager;
use crate::storage::ledger::HistoryLedger;
use crate::storage::state_store::QueueStateStore;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, Semaphore};

/// Coordinates all background asynchronous tasks (`yt-dlp`, `ffmpeg`, `lrclib`, `reqwest` Kitty thumbnails,
/// Wayland clipboard, `pbf03` ephemeral cookie jars, and SQLite ledger updates).
pub struct AsyncWorkerCoordinator {
    tx: mpsc::UnboundedSender<Message>,
    xdg: XdgPaths,
    config: AppConfig,
    http: reqwest::Client,
    download_sem: Arc<Semaphore>,
    cancelled_jobs: Arc<Mutex<HashMap<String, bool>>>,
}

impl AsyncWorkerCoordinator {
    #[must_use]
    pub fn new(tx: mpsc::UnboundedSender<Message>, xdg: XdgPaths, config: AppConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("mvd/0.1.0 (https://github.com/endrico-fn/mvd)")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        let max_jobs = config.max_concurrent_workers.max(1);
        Self {
            tx,
            xdg,
            config,
            http,
            download_sem: Arc::new(Semaphore::new(max_jobs)),
            cancelled_jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Loads initial SQLite history records and cache disk usage on startup.
    pub fn refresh_initial_storage_telemetry(&self) {
        if let Ok(ledger) = HistoryLedger::open(&self.xdg.data_history_db) {
            if let Ok(records) = ledger.list_recent(200) {
                let _ = self.tx.send(Message::HistoryRecordsRefreshed(records));
            }
        }
        let cache_bytes = BoundedCacheManager::compute_usage_bytes(&self.xdg.cache_root);
        let _ = self.tx.send(Message::CacheUsageUpdated(cache_bytes));
    }

    /// Dispatches a list of TEA `Command`s onto the Tokio runtime without blocking the UI thread.
    pub fn dispatch_all(&self, commands: Vec<Command>, current_queue_snapshot: &[QueueJob]) {
        for cmd in commands {
            self.dispatch_one(cmd, current_queue_snapshot);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn dispatch_one(&self, cmd: Command, current_queue_snapshot: &[QueueJob]) {
        match cmd {
            Command::ReadWaylandClipboard => {
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    if let Some(text) = read_clipboard_text().await {
                        let _ = tx.send(Message::PasteClipboardText(text));
                    } else {
                        let _ = tx.send(Message::LogEvent {
                            severity: Severity::Warning,
                            module: "clipboard".to_string(),
                            summary: "Clipboard is empty or wl-paste unavailable".to_string(),
                        });
                    }
                });
            }
            Command::SpawnExtractor {
                normalized_url,
                use_pbf03_cookies,
            } => {
                let tx = self.tx.clone();
                let xdg = self.xdg.clone();
                let config = self.config.clone();
                let http = self.http.clone();
                tokio::spawn(async move {
                    run_extractor_task(tx, xdg, config, http, normalized_url, use_pbf03_cookies)
                        .await;
                });
            }
            Command::FetchCollectionRowPreview {
                media_id,
                canonical_url,
                title,
                artist,
                duration_secs,
                thumbnail_url,
                is_audio,
                needs_hydration,
                use_pbf03_cookies,
            } => {
                if let Some(thumb_url) = thumbnail_url {
                    spawn_thumbnail_fetch(
                        self.tx.clone(),
                        self.http.clone(),
                        self.xdg.cache_thumbs.clone(),
                        thumb_url,
                    );
                }
                if is_audio {
                    spawn_lrclib_fetch(
                        self.tx.clone(),
                        self.http.clone(),
                        self.xdg.cache_lyrics.clone(),
                        media_id.clone(),
                        artist.unwrap_or_default(),
                        title,
                        duration_secs,
                    );
                }
                if needs_hydration {
                    let tx = self.tx.clone();
                    let xdg = self.xdg.clone();
                    let config = self.config.clone();
                    let http = self.http.clone();
                    tokio::spawn(async move {
                        hydrate_playlist_item_task(
                            tx,
                            xdg,
                            config,
                            http,
                            media_id,
                            canonical_url,
                            is_audio,
                            use_pbf03_cookies,
                        )
                        .await;
                    });
                }
            }
            Command::EnqueueJobs(jobs) => {
                for job in jobs {
                    let tx = self.tx.clone();
                    let xdg = self.xdg.clone();
                    let config = self.config.clone();
                    let http = self.http.clone();
                    let sem = Arc::clone(&self.download_sem);
                    let cancelled = Arc::clone(&self.cancelled_jobs);
                    tokio::spawn(async move {
                        let Ok(_permit) = sem.acquire_owned().await else {
                            return;
                        };
                        run_download_job_worker(tx, xdg, config, http, cancelled, job).await;
                    });
                }
            }
            Command::CancelJobInTui { job_id } => {
                if let Ok(mut map) = self.cancelled_jobs.lock() {
                    map.insert(job_id, true);
                }
            }
            Command::PauseOrResumeJob { job_id } => {
                let _ = self.tx.send(Message::LogEvent {
                    severity: Severity::Info,
                    module: "queue".to_string(),
                    summary: format!("Toggled pause/resume state for job {job_id}"),
                });
            }
            Command::ResumeRecoveredQueue | Command::RestartRecoveredQueueFromScratch => {
                let resumable: Vec<QueueJob> = current_queue_snapshot
                    .iter()
                    .filter(|j| j.status.is_unfinished_resumable())
                    .cloned()
                    .collect();
                if !resumable.is_empty() {
                    self.dispatch_one(Command::EnqueueJobs(resumable), current_queue_snapshot);
                }
            }
            Command::ClearHistoryLedgerDbOnly => {
                if let Ok(ledger) = HistoryLedger::open(&self.xdg.data_history_db) {
                    let _ = ledger.clear_all_records_only();
                    let _ = self.tx.send(Message::HistoryRecordsRefreshed(Vec::new()));
                    let _ = self.tx.send(Message::LogEvent {
                        severity: Severity::Success,
                        module: "ledger".to_string(),
                        summary: "Cleared SQLite history.db records (downloaded files preserved)"
                            .to_string(),
                    });
                }
            }
            Command::PurgeXdgCache => {
                if let Ok(reclaimed) = BoundedCacheManager::purge_all(&self.xdg.cache_root) {
                    let _ = self.tx.send(Message::CacheUsageUpdated(0));
                    let _ = self.tx.send(Message::LogEvent {
                        severity: Severity::Success,
                        module: "cache".to_string(),
                        summary: format!("Purged XDG cache ({} KiB reclaimed)", reclaimed / 1024),
                    });
                }
            }
            Command::FlushQueueStateSnapshot => {
                let _ = QueueStateStore::save_snapshot(
                    &self.xdg.state_queue_json,
                    current_queue_snapshot,
                );
            }
            Command::TransitionToCliProgressMode => {}
        }
    }
}

async fn read_clipboard_text() -> Option<String> {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Ok(txt) = cb.get_text() {
            let trimmed = txt.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    let out = tokio::process::Command::new("wl-paste")
        .arg("--no-newline")
        .output()
        .await
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn prepare_ephemeral_cookie_guard(
    xdg: &XdgPaths,
    config: &AppConfig,
    use_pbf03: bool,
    domain: &str,
) -> Option<EphemeralCookieFileGuard> {
    if !use_pbf03 {
        return None;
    }
    let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
    let firefox_root = home.join(".mozilla").join("firefox");
    let profiles_ini = firefox_root.join("profiles.ini");
    let ini_text = std::fs::read_to_string(profiles_ini).ok()?;
    let validated = ProfileGuard::resolve_sandbox_profile(
        &firefox_root,
        &ini_text,
        &config.allowed_firefox_profile,
    )
    .ok()?;
    let job_tag = uuid::Uuid::new_v4().to_string();
    CookieVault::materialize_scoped_jar_for_job(
        &validated,
        &xdg.runtime_cookie_vault,
        &job_tag,
        domain,
    )
    .ok()
}

async fn run_extractor_task(
    tx: mpsc::UnboundedSender<Message>,
    xdg: XdgPaths,
    config: AppConfig,
    http: reqwest::Client,
    normalized_url: String,
    use_pbf03_cookies: bool,
) {
    let parsed = match UrlParser::parse_and_normalize(&normalized_url) {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(Message::ExtractionFailed {
                target_url: normalized_url,
                error: e.to_string(),
            });
            return;
        }
    };

    if parsed.platform == PlatformKind::Spotify {
        extract_spotify_bridge_task(tx, xdg, config, http, parsed, use_pbf03_cookies).await;
        return;
    }

    let cookie_guard =
        prepare_ephemeral_cookie_guard(&xdg, &config, use_pbf03_cookies, &parsed.host_domain);

    let is_collection = parsed.initial_archetype.is_collection();
    let mut args = vec![
        "--dump-single-json".to_string(),
        "--no-warnings".to_string(),
    ];
    if is_collection {
        args.push("--flat-playlist".to_string());
    } else {
        args.push("--no-playlist".to_string());
    }
    if let Some(guard) = &cookie_guard {
        args.push("--cookies".to_string());
        args.push(guard.path().to_string_lossy().into_owned());
    }
    args.push(parsed.normalized_url.clone());

    let _ = tx.send(Message::RawSubprocessLine(format!(
        "[extractor] yt-dlp {}",
        args.join(" ")
    )));

    let output = match tokio::process::Command::new("yt-dlp")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
    {
        Ok(out) => out,
        Err(e) => {
            let _ = tx.send(Message::ExtractionFailed {
                target_url: parsed.normalized_url,
                error: format!("Failed to spawn yt-dlp: {e}"),
            });
            return;
        }
    };

    drop(cookie_guard);

    if !output.status.success() {
        let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let first_err = stderr_str
            .lines()
            .last()
            .unwrap_or("yt-dlp exited with non-zero status")
            .to_string();
        let _ = tx.send(Message::RawSubprocessLine(format!(
            "[yt-dlp stderr] {first_err}"
        )));
        let _ = tx.send(Message::ExtractionFailed {
            target_url: parsed.normalized_url,
            error: first_err,
        });
        return;
    }

    let json_val: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(e) => {
            let _ = tx.send(Message::ExtractionFailed {
                target_url: parsed.normalized_url,
                error: format!("JSON parse error: {e}"),
            });
            return;
        }
    };

    let is_playlist_json = json_val
        .get("_type")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|t| t == "playlist" || t == "multi_video");

    if is_collection || is_playlist_json {
        handle_playlist_json(tx, xdg, parsed, &json_val);
    } else {
        handle_single_media_json(tx, xdg, config, http, parsed, &json_val);
    }
}

#[allow(clippy::too_many_lines)]
fn handle_playlist_json(
    tx: mpsc::UnboundedSender<Message>,
    xdg: XdgPaths,
    parsed: ParsedMediaUrl,
    json_val: &serde_json::Value,
) {
    let playlist_id = json_val
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("playlist")
        .to_string();
    let title = json_val
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Untitled Playlist")
        .to_string();
    let uploader = json_val
        .get("uploader")
        .or_else(|| json_val.get("channel"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);

    let archetype = if parsed.platform == PlatformKind::YouTubeMusic {
        MediaArchetype::AudioPlaylist
    } else {
        parsed.initial_archetype
    };

    let global_policy = if archetype == MediaArchetype::AudioPlaylist {
        GlobalPlaylistPolicy::default_audio_album(&title)
    } else {
        GlobalPlaylistPolicy::default_video_playlist(&title)
    };

    let ledger = HistoryLedger::open(&xdg.data_history_db).ok();
    let mut entries = Vec::new();

    if let Some(arr) = json_val
        .get("entries")
        .and_then(serde_json::Value::as_array)
    {
        for (idx, item) in arr.iter().enumerate() {
            let Some(media_id) = item.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let item_title = item
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Untitled Track")
                .to_string();
            if item_title == "[Deleted video]" || item_title == "[Private video]" {
                continue;
            }
            let channel = item
                .get("channel")
                .or_else(|| item.get("uploader"))
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
            let duration_secs = item
                .get("duration")
                .and_then(serde_json::Value::as_f64)
                .map(|d| d.round() as u64);

            let thumbs = parse_thumbnails_array(item);
            let (is_art_1x1, is_vevo_16x9) = SmartThumbnailSelector::classify_ytmusic_visual_kind(
                channel.as_deref(),
                None,
                &[],
                &thumbs,
            );
            let selected_thumb = SmartThumbnailSelector::select_best(
                &thumbs,
                archetype == MediaArchetype::AudioPlaylist && is_art_1x1,
                is_vevo_16x9 || archetype != MediaArchetype::AudioPlaylist,
            );

            let aspect_ratio = selected_thumb.as_ref().map_or_else(
                || {
                    if archetype == MediaArchetype::AudioPlaylist && is_art_1x1 {
                        AspectRatioKind::Square1x1
                    } else {
                        AspectRatioKind::Landscape16x9
                    }
                },
                |t| t.aspect_kind,
            );

            let canonical_url = if archetype == MediaArchetype::AudioPlaylist {
                format!("https://music.youtube.com/watch?v={media_id}")
            } else {
                item.get("url")
                    .and_then(serde_json::Value::as_str)
                    .map_or_else(
                        || format!("https://www.youtube.com/watch?v={media_id}"),
                        |u| {
                            if u.starts_with("http") {
                                u.to_string()
                            } else {
                                format!("https://www.youtube.com/watch?v={media_id}")
                            }
                        },
                    )
            };

            let in_ledger = ledger.as_ref().is_some_and(|l| {
                l.is_duplicate(parsed.platform.label(), media_id, &canonical_url)
                    .unwrap_or(false)
            });

            entries.push(PlaylistItemEntry {
                index_1_based: idx + 1,
                media_id: media_id.to_string(),
                canonical_url,
                title: item_title,
                channel_or_artist: channel,
                duration_secs,
                selected_for_download: true,
                hydrated: false,
                aspect_ratio,
                best_thumbnail_url: selected_thumb.map(|t| t.url),
                available_formats: Vec::new(),
                manual_sub_keys: Vec::new(),
                auto_sub_keys: Vec::new(),
                already_in_history_ledger: in_ledger,
            });
        }
    }

    let workspace = PlaylistWorkspace {
        playlist_id,
        title,
        uploader,
        global_policy,
        entries,
        item_overrides: BTreeMap::new(),
    };

    let _ = tx.send(Message::ExtractionSucceededCollection {
        platform: parsed.platform,
        archetype,
        workspace,
    });
}

#[allow(clippy::too_many_lines)]
fn handle_single_media_json(
    tx: mpsc::UnboundedSender<Message>,
    xdg: XdgPaths,
    config: AppConfig,
    http: reqwest::Client,
    parsed: ParsedMediaUrl,
    json_val: &serde_json::Value,
) {
    let media_id = json_val
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("media")
        .to_string();
    let title = json_val
        .get("track")
        .or_else(|| json_val.get("title"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Untitled Media")
        .to_string();
    let artist = json_val
        .get("artist")
        .or_else(|| json_val.get("creator"))
        .or_else(|| json_val.get("uploader"))
        .or_else(|| json_val.get("channel"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let album = json_val
        .get("album")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let release_year = json_val
        .get("release_year")
        .and_then(serde_json::Value::as_u64)
        .map(|y| y.to_string())
        .or_else(|| {
            json_val
                .get("upload_date")
                .and_then(serde_json::Value::as_str)
                .and_then(|s| s.get(0..4))
                .map(ToString::to_string)
        });
    let duration_secs = json_val
        .get("duration")
        .and_then(serde_json::Value::as_f64)
        .map(|d| d.round() as u64);

    let formats = parse_stream_candidates(json_val, parsed.initial_archetype);
    let resolutions: Vec<String> = formats.iter().map(|f| f.resolution_label.clone()).collect();
    let thumbs = parse_thumbnails_array(json_val);

    let (is_art_1x1, is_vevo_16x9) = SmartThumbnailSelector::classify_ytmusic_visual_kind(
        artist.as_deref(),
        album.as_deref(),
        &resolutions,
        &thumbs,
    );
    let selected_thumb = SmartThumbnailSelector::select_best(
        &thumbs,
        parsed.initial_archetype == MediaArchetype::SingleAudio && is_art_1x1,
        is_vevo_16x9 || parsed.initial_archetype != MediaArchetype::SingleAudio,
    );

    let aspect_ratio = selected_thumb.as_ref().map_or_else(
        || {
            if parsed.platform == PlatformKind::YouTubeShorts {
                AspectRatioKind::Vertical9x16
            } else if parsed.initial_archetype == MediaArchetype::SingleAudio && is_art_1x1 {
                AspectRatioKind::Square1x1
            } else {
                AspectRatioKind::Landscape16x9
            }
        },
        |t| t.aspect_kind,
    );

    let manual_sub_keys = extract_object_keys(json_val.get("subtitles"));
    let auto_sub_keys = extract_object_keys(json_val.get("automatic_captions"));

    let target_container = if parsed.initial_archetype == MediaArchetype::SingleAudio {
        config.default_audio_container
    } else {
        config.default_video_container
    };

    let subtitle_plan = SubtitleResolutionPlan::resolve(
        &manual_sub_keys,
        &auto_sub_keys,
        config.prefer_sub_en,
        config.prefer_sub_id,
        config.allow_auto_sub_fallback,
        target_container,
    );

    let chapters = parse_chapters(json_val);
    let mut metadata = EditableMetadata::default_for_title(&title);
    metadata.artist_or_creator.clone_from(&artist);
    metadata.album = album;
    metadata.release_year = release_year;

    let is_duplicate_in_ledger = HistoryLedger::open(&xdg.data_history_db)
        .ok()
        .is_some_and(|l| {
            l.is_duplicate(parsed.platform.label(), &media_id, &parsed.normalized_url)
                .unwrap_or(false)
        });

    let thumb_url = selected_thumb.map(|t| t.url);

    let _ = tx.send(Message::ExtractionSucceededSingle {
        media_id: media_id.clone(),
        canonical_url: parsed.normalized_url,
        platform: parsed.platform,
        archetype: parsed.initial_archetype,
        aspect_ratio,
        thumbnail_url: thumb_url.clone(),
        metadata,
        formats,
        target_container,
        subtitle_plan,
        chapters,
        is_duplicate_in_ledger,
    });

    if let Some(url) = thumb_url {
        spawn_thumbnail_fetch(tx.clone(), http.clone(), xdg.cache_thumbs.clone(), url);
    }

    if parsed.initial_archetype == MediaArchetype::SingleAudio {
        spawn_lrclib_fetch(
            tx,
            http,
            xdg.cache_lyrics,
            media_id,
            artist.unwrap_or_default(),
            title,
            duration_secs,
        );
    }
}

#[allow(clippy::too_many_lines)]
async fn extract_spotify_bridge_task(
    tx: mpsc::UnboundedSender<Message>,
    xdg: XdgPaths,
    config: AppConfig,
    http: reqwest::Client,
    parsed: ParsedMediaUrl,
    use_pbf03_cookies: bool,
) {
    // Extract track ID from https://open.spotify.com/track/<id>
    let track_id = parsed
        .normalized_url
        .split("/track/")
        .nth(1)
        .and_then(|s| s.split(['?', '/']).next())
        .unwrap_or("spotify")
        .to_string();

    let embed_url = format!("https://open.spotify.com/embed/track/{track_id}");
    let _ = tx.send(Message::RawSubprocessLine(format!(
        "[spotify-bridge] Fetching embed metadata: {embed_url}"
    )));

    let html = match http.get(&embed_url).send().await {
        Ok(resp) => resp.text().await.unwrap_or_default(),
        Err(e) => {
            let _ = tx.send(Message::ExtractionFailed {
                target_url: parsed.normalized_url,
                error: format!("Spotify embed fetch failed: {e}"),
            });
            return;
        }
    };

    let Some(sp_meta) = parse_spotify_embed_html(&track_id, &html) else {
        let _ = tx.send(Message::ExtractionFailed {
            target_url: parsed.normalized_url,
            error: "Could not parse __NEXT_DATA__ from Spotify embed page".to_string(),
        });
        return;
    };

    let primary_artist = sp_meta
        .artists
        .first()
        .cloned()
        .unwrap_or_else(|| "Artist".to_string());
    let search_query = format!("ytsearch5:{primary_artist} - {}", sp_meta.title);

    let _ = tx.send(Message::RawSubprocessLine(format!(
        "[spotify-bridge] Bridging via yt-dlp: {search_query} (target {}ms)",
        sp_meta.duration_ms
    )));

    let search_out = match tokio::process::Command::new("yt-dlp")
        .args([
            "--dump-single-json",
            "--flat-playlist",
            "--no-warnings",
            &search_query,
        ])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            let _ = tx.send(Message::ExtractionFailed {
                target_url: parsed.normalized_url,
                error: format!("yt-dlp ytsearch5 failed: {e}"),
            });
            return;
        }
    };

    let mut candidates = Vec::new();
    if let Ok(search_json) = serde_json::from_slice::<serde_json::Value>(&search_out.stdout) {
        if let Some(entries) = search_json
            .get("entries")
            .and_then(serde_json::Value::as_array)
        {
            for e in entries {
                if let Some(vid) = e.get("id").and_then(serde_json::Value::as_str) {
                    let c_title = e
                        .get("title")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let c_chan = e
                        .get("channel")
                        .or_else(|| e.get("uploader"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let c_dur = e
                        .get("duration")
                        .and_then(serde_json::Value::as_f64)
                        .map_or(0, |d| d.round() as u64);
                    candidates.push(YtSearchAudioCandidate {
                        video_id: vid.to_string(),
                        title: c_title,
                        channel: c_chan,
                        duration_secs: c_dur,
                    });
                }
            }
        }
    }

    let Some(best_match) = SpotifyBridge::pick_best_audio_match(&sp_meta, &candidates) else {
        let _ = tx.send(Message::ExtractionFailed {
            target_url: parsed.normalized_url,
            error: "No matching audio stream found on YouTube Music bridge".to_string(),
        });
        return;
    };

    let bridged_url = format!("https://music.youtube.com/watch?v={}", best_match.video_id);
    let cookie_guard =
        prepare_ephemeral_cookie_guard(&xdg, &config, use_pbf03_cookies, "youtube.com");

    let mut args = vec![
        "--dump-single-json".to_string(),
        "--no-warnings".to_string(),
        "--no-playlist".to_string(),
    ];
    if let Some(guard) = &cookie_guard {
        args.push("--cookies".to_string());
        args.push(guard.path().to_string_lossy().into_owned());
    }
    args.push(bridged_url.clone());

    let formats = if let Ok(out) = tokio::process::Command::new("yt-dlp")
        .args(&args)
        .output()
        .await
    {
        serde_json::from_slice::<serde_json::Value>(&out.stdout)
            .ok()
            .map(|v| parse_stream_candidates(&v, MediaArchetype::SingleAudio))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    drop(cookie_guard);

    let mut metadata = EditableMetadata::default_for_title(&sp_meta.title);
    metadata.artist_or_creator = Some(sp_meta.artists.join(", "));
    metadata.release_year = sp_meta
        .release_date
        .as_deref()
        .and_then(|d| d.get(0..4))
        .map(ToString::to_string);

    let thumb_url = sp_meta.cover_art_640_url.clone();

    let _ = tx.send(Message::ExtractionSucceededSingle {
        media_id: best_match.video_id.clone(),
        canonical_url: bridged_url,
        platform: PlatformKind::Spotify,
        archetype: MediaArchetype::SingleAudio,
        aspect_ratio: AspectRatioKind::Square1x1,
        thumbnail_url: thumb_url.clone(),
        metadata,
        formats,
        target_container: config.default_audio_container,
        subtitle_plan: SubtitleResolutionPlan {
            tracks: Vec::new(),
            delivery_mode: crate::domain::subtitle::SubtitleDeliveryMode::Disabled,
        },
        chapters: Vec::new(),
        is_duplicate_in_ledger: false,
    });

    if let Some(url) = thumb_url {
        spawn_thumbnail_fetch(tx.clone(), http.clone(), xdg.cache_thumbs.clone(), url);
    }

    spawn_lrclib_fetch(
        tx,
        http,
        xdg.cache_lyrics,
        best_match.video_id.clone(),
        primary_artist,
        sp_meta.title,
        Some(sp_meta.duration_ms / 1000),
    );
}

fn parse_spotify_embed_html(track_id: &str, html: &str) -> Option<SpotifyTrackMetadata> {
    let marker = "<script id=\"__NEXT_DATA__\" type=\"application/json\">";
    let start = html.find(marker)? + marker.len();
    let end = html[start..].find("</script>")? + start;
    let json_val: serde_json::Value = serde_json::from_str(&html[start..end]).ok()?;
    let entity = json_val
        .get("props")?
        .get("pageProps")?
        .get("state")?
        .get("data")?
        .get("entity")?;

    let title = entity.get("title")?.as_str()?.to_string();
    let duration_ms = entity
        .get("duration")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(180_000);
    let release_date = entity
        .get("releaseDate")
        .and_then(|r| r.get("isoString"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);

    let mut artists = Vec::new();
    if let Some(arr) = entity.get("artists").and_then(serde_json::Value::as_array) {
        for a in arr {
            if let Some(name) = a.get("name").and_then(serde_json::Value::as_str) {
                artists.push(name.to_string());
            }
        }
    }

    let mut cover_art_640_url = None;
    if let Some(imgs) = entity
        .get("visualIdentity")
        .and_then(|v| v.get("image"))
        .and_then(serde_json::Value::as_array)
    {
        if let Some(best_img) = imgs.iter().max_by_key(|i| {
            i.get("maxWidth")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
        }) {
            cover_art_640_url = best_img
                .get("url")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string);
        }
    }

    Some(SpotifyTrackMetadata {
        spotify_id: track_id.to_string(),
        title,
        artists,
        duration_ms,
        release_date,
        cover_art_640_url,
    })
}

async fn hydrate_playlist_item_task(
    tx: mpsc::UnboundedSender<Message>,
    xdg: XdgPaths,
    config: AppConfig,
    http: reqwest::Client,
    media_id: String,
    canonical_url: String,
    is_audio: bool,
    use_pbf03_cookies: bool,
) {
    let cookie_guard =
        prepare_ephemeral_cookie_guard(&xdg, &config, use_pbf03_cookies, "youtube.com");
    let mut args = vec![
        "--dump-single-json".to_string(),
        "--no-warnings".to_string(),
        "--no-playlist".to_string(),
    ];
    if let Some(guard) = &cookie_guard {
        args.push("--cookies".to_string());
        args.push(guard.path().to_string_lossy().into_owned());
    }
    args.push(canonical_url);

    let Ok(out) = tokio::process::Command::new("yt-dlp")
        .args(&args)
        .output()
        .await
    else {
        return;
    };
    drop(cookie_guard);

    if !out.status.success() {
        return;
    }
    let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return;
    };

    let archetype = if is_audio {
        MediaArchetype::SingleAudio
    } else {
        MediaArchetype::SingleVideo
    };
    let formats = parse_stream_candidates(&json_val, archetype);
    let resolutions: Vec<String> = formats.iter().map(|f| f.resolution_label.clone()).collect();
    let thumbs = parse_thumbnails_array(&json_val);
    let channel = json_val
        .get("channel")
        .or_else(|| json_val.get("uploader"))
        .and_then(serde_json::Value::as_str);
    let album = json_val.get("album").and_then(serde_json::Value::as_str);

    let (is_art_1x1, is_vevo_16x9) =
        SmartThumbnailSelector::classify_ytmusic_visual_kind(channel, album, &resolutions, &thumbs);
    let selected_thumb = SmartThumbnailSelector::select_best(
        &thumbs,
        is_audio && is_art_1x1,
        is_vevo_16x9 || !is_audio,
    );

    let aspect_ratio = selected_thumb.as_ref().map_or_else(
        || {
            if is_audio && is_art_1x1 {
                AspectRatioKind::Square1x1
            } else {
                AspectRatioKind::Landscape16x9
            }
        },
        |t| t.aspect_kind,
    );
    let thumb_url = selected_thumb.map(|t| t.url);
    let manual_sub_keys = extract_object_keys(json_val.get("subtitles"));
    let auto_sub_keys = extract_object_keys(json_val.get("automatic_captions"));

    if let Some(url) = &thumb_url {
        spawn_thumbnail_fetch(tx.clone(), http, xdg.cache_thumbs, url.clone());
    }

    let _ = tx.send(Message::PlaylistItemHydrated {
        media_id,
        aspect_ratio,
        thumbnail_url: thumb_url,
        formats,
        manual_sub_keys,
        auto_sub_keys,
    });
}

fn spawn_thumbnail_fetch(
    tx: mpsc::UnboundedSender<Message>,
    http: reqwest::Client,
    cache_thumbs_dir: std::path::PathBuf,
    url: String,
) {
    tokio::spawn(async move {
        let _ = std::fs::create_dir_all(&cache_thumbs_dir);
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash_hex = hex::encode(hasher.finalize());
        let cache_file = cache_thumbs_dir.join(format!("{hash_hex}.img"));

        let bytes = if let Ok(cached) = tokio::fs::read(&cache_file).await {
            cached
        } else {
            let Ok(resp) = http.get(&url).send().await else {
                return;
            };
            let Ok(b) = resp.bytes().await else {
                return;
            };
            let vec = b.to_vec();
            let _ = tokio::fs::write(&cache_file, &vec).await;
            vec
        };

        if let Ok(image) = image::load_from_memory(&bytes) {
            let _ = tx.send(Message::ThumbnailDecoded { url, image });
        }
    });
}

fn spawn_lrclib_fetch(
    tx: mpsc::UnboundedSender<Message>,
    http: reqwest::Client,
    cache_lyrics_dir: std::path::PathBuf,
    media_id: String,
    raw_artist: String,
    raw_title: String,
    duration_secs: Option<u64>,
) {
    tokio::spawn(async move {
        let _ = std::fs::create_dir_all(&cache_lyrics_dir);
        let (artist, track) = LrclibCleaner::normalize_artist_and_track(&raw_artist, &raw_title);
        let cache_file = cache_lyrics_dir.join(format!("{media_id}.json"));

        if let Ok(cached_str) = tokio::fs::read_to_string(&cache_file).await {
            if let Ok(doc) = serde_json::from_str::<LyricsDocument>(&cached_str) {
                let _ = tx.send(Message::LyricsFetched {
                    media_id,
                    lyrics: Some(doc),
                });
                return;
            }
        }

        let search_url = reqwest::Url::parse_with_params(
            "https://lrclib.net/api/search",
            &[
                ("q", format!("{artist} {track}")),
                ("track_name", track.clone()),
            ],
        );

        let Ok(url) = search_url else {
            let _ = tx.send(Message::LyricsFetched {
                media_id,
                lyrics: None,
            });
            return;
        };

        let Ok(resp) = http.get(url).send().await else {
            let _ = tx.send(Message::LyricsFetched {
                media_id,
                lyrics: None,
            });
            return;
        };

        let Ok(arr) = resp.json::<Vec<serde_json::Value>>().await else {
            let _ = tx.send(Message::LyricsFetched {
                media_id,
                lyrics: None,
            });
            return;
        };

        let best = arr.iter().min_by_key(|item| {
            let has_synced = item
                .get("syncedLyrics")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|s| !s.is_empty());
            let item_dur = item
                .get("duration")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            let dur_diff = duration_secs.map_or(0, |target| {
                ((item_dur - (target as f64)).abs().round()) as u64
            });
            (u8::from(!has_synced), dur_diff)
        });

        let Some(chosen) = best else {
            let _ = tx.send(Message::LyricsFetched {
                media_id,
                lyrics: None,
            });
            return;
        };

        let synced_str = chosen
            .get("syncedLyrics")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.trim().is_empty());
        let plain_str = chosen
            .get("plainLyrics")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.trim().is_empty());

        let (raw_lrc, is_synced) = if let Some(s) = synced_str {
            (s.to_string(), true)
        } else if let Some(p) = plain_str {
            (p.to_string(), false)
        } else {
            let _ = tx.send(Message::LyricsFetched {
                media_id,
                lyrics: None,
            });
            return;
        };

        let lines = LrclibCleaner::parse_lrc_lines(&raw_lrc, is_synced);
        let doc = LyricsDocument {
            lrclib_id: chosen.get("id").and_then(serde_json::Value::as_u64),
            track_name: chosen
                .get("trackName")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&track)
                .to_string(),
            artist_name: chosen
                .get("artistName")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&artist)
                .to_string(),
            is_synced,
            is_instrumental: chosen
                .get("instrumental")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            lines,
            raw_lrc,
        };

        if let Ok(serialized) = serde_json::to_string(&doc) {
            let _ = tokio::fs::write(&cache_file, serialized).await;
        }

        let _ = tx.send(Message::LyricsFetched {
            media_id,
            lyrics: Some(doc),
        });
    });
}

#[allow(clippy::too_many_lines)]
async fn run_download_job_worker(
    tx: mpsc::UnboundedSender<Message>,
    xdg: XdgPaths,
    config: AppConfig,
    http: reqwest::Client,
    cancelled_jobs: Arc<Mutex<HashMap<String, bool>>>,
    job: QueueJob,
) {
    if cancelled_jobs
        .lock()
        .ok()
        .is_some_and(|m| m.get(&job.job_id).copied().unwrap_or(false))
    {
        return;
    }

    if let Some(parent) = job.final_output_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let cookie_guard =
        prepare_ephemeral_cookie_guard(&xdg, &config, job.use_pbf03_cookies, "youtube.com");

    let mut args = vec![
        "--newline".to_string(),
        "--progress".to_string(),
        "--no-warnings".to_string(),
        "--no-playlist".to_string(),
        "--continue".to_string(),
    ];

    if let Some(guard) = &cookie_guard {
        args.push("--cookies".to_string());
        args.push(guard.path().to_string_lossy().into_owned());
    }

    if job.archetype == MediaArchetype::SingleAudio {
        let afmt = match job.target_container {
            ContainerFormat::Opus => "opus",
            ContainerFormat::Flac => "flac",
            ContainerFormat::M4a => "m4a",
            ContainerFormat::Mp3 => "mp3",
            _ => "opus",
        };
        args.extend([
            "-f".to_string(),
            job.audio_format_expr
                .clone()
                .unwrap_or_else(|| "bestaudio".to_string()),
            "-x".to_string(),
            "--audio-format".to_string(),
            afmt.to_string(),
            "--embed-metadata".to_string(),
        ]);
    } else {
        let vfmt = job
            .video_format_expr
            .clone()
            .unwrap_or_else(|| "bestvideo".to_string());
        let afmt = job
            .audio_format_expr
            .clone()
            .unwrap_or_else(|| "bestaudio".to_string());
        args.extend([
            "-f".to_string(),
            format!("{vfmt}+{afmt}/best"),
            "--merge-output-format".to_string(),
            job.target_container.extension().to_string(),
            "--embed-metadata".to_string(),
            "--embed-chapters".to_string(),
        ]);

        if !job.subtitle_plan.tracks.is_empty() {
            let sub_langs: Vec<String> = job
                .subtitle_plan
                .tracks
                .iter()
                .map(|t| t.raw_lang_key.clone())
                .collect();
            let has_manual = job
                .subtitle_plan
                .tracks
                .iter()
                .any(|t| t.origin == crate::domain::subtitle::SubtitleOrigin::Manual);
            let has_auto = job
                .subtitle_plan
                .tracks
                .iter()
                .any(|t| t.origin == crate::domain::subtitle::SubtitleOrigin::AutoGeneratedAsr);
            if has_manual {
                args.push("--write-subs".to_string());
            }
            if has_auto {
                args.push("--write-auto-subs".to_string());
            }
            args.extend([
                "--sub-langs".to_string(),
                sub_langs.join(","),
                "--convert-subs".to_string(),
                "srt".to_string(),
            ]);
            if job.target_container == ContainerFormat::Mkv {
                args.push("--embed-subs".to_string());
            }
        }
    }

    args.extend([
        "-o".to_string(),
        job.final_output_path.to_string_lossy().into_owned(),
        job.source_url.clone(),
    ]);

    let _ = tx.send(Message::RawSubprocessLine(format!(
        "[worker:{}] yt-dlp {}",
        job.job_id,
        args.join(" ")
    )));

    let Ok(mut child) = tokio::process::Command::new("yt-dlp")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        let _ = tx.send(Message::JobFailed {
            job_id: job.job_id,
            error: "Failed to spawn yt-dlp download worker".to_string(),
        });
        return;
    };

    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if cancelled_jobs
                .lock()
                .ok()
                .is_some_and(|m| m.get(&job.job_id).copied().unwrap_or(false))
            {
                let _ = child.kill().await;
                return;
            }
            if let Some((pct_x10, speed_bps)) = parse_ytdlp_progress_line(&line) {
                let _ = tx.send(Message::JobProgressUpdated {
                    job_id: job.job_id.clone(),
                    percent_x10: pct_x10,
                    speed_bps,
                    eta_secs: None,
                });
            }
            let _ = tx.send(Message::RawSubprocessLine(line));
        }
    }

    let status = child.wait().await;
    drop(cookie_guard);

    if status.is_ok_and(|s| s.success()) {
        // Post-process audio tags + LRCLIB lyrics + cover art via lofty if audio
        if job.archetype == MediaArchetype::SingleAudio && job.final_output_path.exists() {
            let cover_bytes = if let Some(url) = &job.cover_art_url {
                if let Ok(resp) = http.get(url).send().await {
                    resp.bytes().await.ok().map(|b| b.to_vec())
                } else {
                    None
                }
            } else {
                None
            };
            let _ = AudioMetadataTagger::write_tags_and_lyrics(
                &job.final_output_path,
                &job.metadata,
                job.synced_lyrics_lrc.as_deref(),
                cover_bytes.as_deref(),
            );
        }

        // Record in SQLite History Ledger unless Ephemeral Privacy Mode ([I]) is active
        if !job.ephemeral_privacy_mode {
            record_job_in_ledger(&xdg.data_history_db, &job, &tx);
        }

        let _ = tx.send(Message::JobCompleted { job_id: job.job_id });
    } else {
        let _ = tx.send(Message::JobFailed {
            job_id: job.job_id,
            error: "yt-dlp download process exited with error (check [3:RAW LOGS])".to_string(),
        });
    }
}

fn record_job_in_ledger(db_path: &Path, job: &QueueJob, tx: &mpsc::UnboundedSender<Message>) {
    let Ok(ledger) = HistoryLedger::open(db_path) else {
        return;
    };
    let size_bytes = std::fs::metadata(&job.final_output_path).map_or(0, |m| m.len());
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let _ = ledger.record_completed(
        job.platform.label(),
        &job.media_id,
        &job.source_url,
        &job.metadata.title,
        job.target_container.extension(),
        &job.final_output_path.to_string_lossy(),
        size_bytes,
        now_epoch,
    );
    if let Ok(records) = ledger.list_recent(200) {
        let _ = tx.send(Message::HistoryRecordsRefreshed(records));
    }
}

fn parse_ytdlp_progress_line(line: &str) -> Option<(u16, u64)> {
    if !line.contains("[download]") || !line.contains('%') {
        return None;
    }
    let pct_part = line.split('%').next()?.split_whitespace().last()?;
    let pct_f: f64 = pct_part.parse().ok()?;
    let pct_x10 = ((pct_f * 10.0).round() as u16).clamp(0, 1000);

    let speed_bps = if let Some(at_part) = line.split(" at ").nth(1) {
        let sp_tok = at_part.split_whitespace().next().unwrap_or("");
        if let Some(num) = sp_tok.strip_suffix("MiB/s") {
            num.parse::<f64>()
                .ok()
                .map_or(0, |v| (v * 1024.0 * 1024.0) as u64)
        } else if let Some(num) = sp_tok.strip_suffix("KiB/s") {
            num.parse::<f64>().ok().map_or(0, |v| (v * 1024.0) as u64)
        } else {
            0
        }
    } else {
        0
    };

    Some((pct_x10, speed_bps))
}

fn parse_thumbnails_array(val: &serde_json::Value) -> Vec<YtDlpThumbnailJson> {
    let mut out = Vec::new();
    if let Some(arr) = val.get("thumbnails").and_then(serde_json::Value::as_array) {
        for item in arr {
            if let Some(url) = item.get("url").and_then(serde_json::Value::as_str) {
                out.push(YtDlpThumbnailJson {
                    id: item
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string),
                    url: url.to_string(),
                    width: item
                        .get("width")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|v| u32::try_from(v).ok()),
                    height: item
                        .get("height")
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|v| u32::try_from(v).ok()),
                    preference: item
                        .get("preference")
                        .and_then(serde_json::Value::as_i64)
                        .and_then(|v| i32::try_from(v).ok()),
                });
            }
        }
    }
    out
}

fn parse_stream_candidates(
    val: &serde_json::Value,
    archetype: MediaArchetype,
) -> Vec<StreamCandidate> {
    let mut out = Vec::new();
    if let Some(arr) = val.get("formats").and_then(serde_json::Value::as_array) {
        for f in arr {
            let Some(fmt_id) = f.get("format_id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let ext = f
                .get("ext")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("bin")
                .to_string();
            if ext == "mhtml" {
                continue;
            }
            let width = f
                .get("width")
                .and_then(serde_json::Value::as_u64)
                .and_then(|v| u32::try_from(v).ok());
            let height = f
                .get("height")
                .and_then(serde_json::Value::as_u64)
                .and_then(|v| u32::try_from(v).ok());
            let vcodec = f
                .get("vcodec")
                .and_then(serde_json::Value::as_str)
                .filter(|&s| s != "none")
                .map(ToString::to_string);
            let acodec = f
                .get("acodec")
                .and_then(serde_json::Value::as_str)
                .filter(|&s| s != "none")
                .map(ToString::to_string);
            let abr_kbps = f
                .get("abr")
                .and_then(serde_json::Value::as_f64)
                .map(|v| v.round() as u32);
            let tbr_kbps = f
                .get("tbr")
                .and_then(serde_json::Value::as_f64)
                .map(|v| v.round() as u32);
            let fps = f
                .get("fps")
                .and_then(serde_json::Value::as_f64)
                .map(|v| v.round() as u32);
            let filesize_bytes = f
                .get("filesize")
                .or_else(|| f.get("filesize_approx"))
                .and_then(serde_json::Value::as_u64);

            if archetype == MediaArchetype::SingleAudio && vcodec.is_some() && acodec.is_none() {
                continue;
            }

            let resolution_label = match (width, height) {
                (Some(w), Some(h)) => format!("{w}x{h}"),
                _ => abr_kbps.map_or_else(|| "audio".to_string(), |a| format!("{a}kbps")),
            };

            out.push(StreamCandidate {
                format_id: fmt_id.to_string(),
                ext,
                resolution_label,
                width,
                height,
                fps,
                vcodec,
                acodec,
                abr_kbps,
                tbr_kbps,
                filesize_bytes,
            });
        }
    }

    // Sort highest quality first (for audio: highest abr/tbr; for video: highest height/tbr)
    out.sort_by(|a, b| {
        if archetype == MediaArchetype::SingleAudio {
            let score_a = (
                u8::from(a.vcodec.is_none()),
                a.abr_kbps.unwrap_or(0),
                a.tbr_kbps.unwrap_or(0),
            );
            let score_b = (
                u8::from(b.vcodec.is_none()),
                b.abr_kbps.unwrap_or(0),
                b.tbr_kbps.unwrap_or(0),
            );
            score_b.cmp(&score_a)
        } else {
            let score_a = (
                a.height.unwrap_or(0),
                a.fps.unwrap_or(0),
                a.tbr_kbps.unwrap_or(0),
            );
            let score_b = (
                b.height.unwrap_or(0),
                b.fps.unwrap_or(0),
                b.tbr_kbps.unwrap_or(0),
            );
            score_b.cmp(&score_a)
        }
    });

    out
}

fn extract_object_keys(val: Option<&serde_json::Value>) -> Vec<String> {
    val.and_then(serde_json::Value::as_object)
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

fn parse_chapters(val: &serde_json::Value) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    if let Some(arr) = val.get("chapters").and_then(serde_json::Value::as_array) {
        for c in arr {
            let title = c
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Chapter")
                .to_string();
            let start = c
                .get("start_time")
                .and_then(serde_json::Value::as_f64)
                .map_or(0, |s| s.round() as u64);
            out.push((title, start));
        }
    }
    out
}
