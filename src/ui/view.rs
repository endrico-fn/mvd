use crate::app::keymap::ConditionalKeyHints;
use crate::app::model::{ActiveModal, AppModel, InputMode, PreviewInspectionState, WorkspacePage};
use crate::domain::media::{AspectRatioKind, MediaArchetype};
use crate::ui::layout::ConditionalExtractorLayout;
use crate::ui::theme::{
    sharp_centered_panel, COLOR_ACCENT_EMERALD, COLOR_STATUS_ERROR, COLOR_STATUS_INFO,
    COLOR_STATUS_WARN, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY, SYM_CURSOR,
};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, Paragraph};
use ratatui::Frame;
use ratatui_image::{FilterType, Resize, StatefulImage};

/// Root Ratatui renderer with native Kitty Graphics Protocol support.
pub fn render(frame: &mut Frame<'_>, model: &mut AppModel) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_topbar(frame, root[0], model);

    match model.active_page {
        WorkspacePage::ExtractorPreview => render_extractor_page(frame, root[1], model),
        WorkspacePage::QueueManager => render_queue_page(frame, root[1], model),
        WorkspacePage::RawTelemetryLogs => render_logs_page(frame, root[1], model),
        WorkspacePage::HistoryLedger => render_history_page(frame, root[1], model),
        WorkspacePage::SettingsSecurity => render_settings_page(frame, root[1], model),
    }

    render_footer(frame, root[2], model);

    if let Some(modal) = &model.active_modal {
        render_centered_modal(frame, frame.area(), modal);
    }
}

fn render_topbar(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let cookie_badge = if model.pbf03_cookie_enabled {
        "COOKIE: pbf03 SANDBOX ACTIVE"
    } else {
        "COOKIE: OFF (ANONYMOUS)"
    };
    let privacy_badge = if model.ephemeral_privacy_mode {
        "PRIVACY: [I] EPHEMERAL"
    } else {
        "PRIVACY: STANDARD"
    };

    let tabs: Vec<String> = WorkspacePage::all()
        .iter()
        .map(|&p| {
            if p == model.active_page {
                format!("[* {}]", p.tab_label())
            } else {
                format!("[ {} ]", p.tab_label())
            }
        })
        .collect();

    let header_line = Line::from(vec![
        Span::styled(
            format!(" {} ", tabs.join(" ")),
            Style::default().fg(COLOR_TEXT_PRIMARY),
        ),
        Span::raw(" │ "),
        Span::styled(cookie_badge, Style::default().fg(COLOR_ACCENT_EMERALD)),
        Span::raw(" │ "),
        Span::styled(privacy_badge, Style::default().fg(COLOR_TEXT_MUTED)),
    ]);

    let block = sharp_centered_panel("MVD v0.1.0 — UTILITARIAN MEDIA ENGINE", true);
    frame.render_widget(Paragraph::new(header_line).block(block), area);
}

#[allow(clippy::too_many_lines)]
fn render_extractor_page(frame: &mut Frame<'_>, area: Rect, model: &mut AppModel) {
    let archetype = match &model.inspection {
        PreviewInspectionState::Empty => MediaArchetype::SingleVideo,
        PreviewInspectionState::FetchingSkeleton {
            expected_archetype, ..
        } => *expected_archetype,
        PreviewInspectionState::LoadedSingle { archetype, .. } => *archetype,
        PreviewInspectionState::LoadedCollection { archetype, .. } => *archetype,
    };

    let regions = ConditionalExtractorLayout::compute(area, archetype);

    // 1. URL / Search Input Bar
    let url_focused = model.input_mode == InputMode::InsertUrl;
    let url_display = if model.url_input_buffer.is_empty() {
        format!(
            "{} {} Press 'n' to enter URL, 'p' to paste clipboard, or 'D' for Quick Enqueue",
            model.input_mode.token_badge(),
            SYM_CURSOR
        )
    } else {
        format!(
            "{} {} {}",
            model.input_mode.token_badge(),
            SYM_CURSOR,
            model.url_input_buffer
        )
    };
    frame.render_widget(
        Paragraph::new(url_display).block(sharp_centered_panel("URL / SEARCH INPUT", url_focused)),
        regions.url_bar_area,
    );

    // 2. Left Preview Column + Right Inspector Column
    match &model.inspection {
        PreviewInspectionState::Empty => {
            frame.render_widget(
                Paragraph::new(
                    "\n[i] NO MEDIA LOADED\nPaste a YouTube, YT Music, Spotify, X, Reddit, IG, or Pinterest link.\nPress 'p' (Normal) or 'n' then Ctrl+V to extract.",
                )
                .alignment(Alignment::Center)
                .style(Style::default().fg(COLOR_TEXT_MUTED))
                .block(sharp_centered_panel("KITTY PREVIEW : STANDBY", false)),
                regions.left_thumbnail_area,
            );
            if let Some(sub_area) = regions.left_chapters_subs_area.or(regions.left_lyrics_area) {
                frame.render_widget(
                    Paragraph::new(
                        "\n[i] Subtitles (EN/ID), Chapters & LRCLIB Synced Lyrics\nwill appear here automatically after extraction.",
                    )
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(COLOR_TEXT_MUTED))
                    .block(sharp_centered_panel(
                        "SUBTITLES (EN/ID) & LRCLIB LYRICS : STANDBY",
                        false,
                    )),
                    sub_area,
                );
            }
            frame.render_widget(
                Paragraph::new("Metadata inspector awaits URL extraction.")
                    .style(Style::default().fg(COLOR_TEXT_MUTED))
                    .block(sharp_centered_panel("MEDIA METADATA", false)),
                regions.right_top_summary_area,
            );
            frame.render_widget(
                Paragraph::new("\nNo stream matrix loaded.")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(COLOR_TEXT_MUTED))
                    .block(sharp_centered_panel("GRANULAR STREAM SELECTOR", false)),
                regions.right_main_inspector_area,
            );
        }
        PreviewInspectionState::FetchingSkeleton {
            target_url,
            platform,
            expected_archetype,
        } => {
            frame.render_widget(
                Paragraph::new(format!(
                    "\n░░▒▒▓▓██ FETCHING KITTY PREVIEW ██▓▓▒▒░░\nSource: {} ({})",
                    platform.label(),
                    expected_archetype.badge_token()
                ))
                .alignment(Alignment::Center)
                .style(Style::default().fg(COLOR_TEXT_MUTED))
                .block(sharp_centered_panel("SKELETON : PROBING MEDIA", true)),
                regions.left_thumbnail_area,
            );
            if let Some(lrc_area) = regions.left_lyrics_area {
                frame.render_widget(
                    Paragraph::new(
                        "\n░░▒▒▓▓ PROBING LRCLIB.NET SYNCED LYRICS ▓▓▒▒░░\nMatching Artist + Track + Duration...",
                    )
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(COLOR_TEXT_MUTED))
                    .block(sharp_centered_panel("LRCLIB LYRICS : LOADING", false)),
                    lrc_area,
                );
            }
            if let Some(sub_area) = regions.left_chapters_subs_area {
                frame.render_widget(
                    Paragraph::new(
                        "\n░░▒▒▓▓ PROBING EN/ID SUBTITLES & CHAPTERS ▓▓▒▒░░\nScanning Manual > Auto-ASR tracks...",
                    )
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(COLOR_TEXT_MUTED))
                    .block(sharp_centered_panel(
                        "SUBTITLES (EN/ID) & CHAPTERS : LOADING",
                        false,
                    )),
                    sub_area,
                );
            }
            let latest_log = model
                .raw_subprocess_lines
                .last()
                .map_or("Spawning async yt-dlp extractor worker...", String::as_str);
            frame.render_widget(
                Paragraph::new(format!(
                    "Target : {target_url}\nWorker : ░░▒▒▓▓ {latest_log}"
                ))
                .block(sharp_centered_panel("EXTRACTOR TELEMETRY", false)),
                regions.right_top_summary_area,
            );
            frame.render_widget(
                Paragraph::new(
                    "\n░░▒▒▓▓ Probing video/audio codecs, aspect ratios & EN/ID subtitle tracks...",
                )
                .block(sharp_centered_panel("STREAM MATRIX : LOADING", false)),
                regions.right_main_inspector_area,
            );
        }
        PreviewInspectionState::LoadedSingle {
            media_id,
            platform,
            archetype,
            aspect_ratio,
            metadata,
            formats,
            selected_format_idx,
            target_container,
            subtitle_plan,
            chapters,
            lyrics,
            is_duplicate_in_ledger,
            ..
        } => {
            let media_id_s = media_id.clone();
            let aspect = *aspect_ratio;
            let lrc_clone = lyrics.clone();
            let sub_plan_clone = subtitle_plan.clone();
            let chap_count = chapters.len();
            let meta_title = metadata.title.clone();
            let meta_artist = metadata
                .artist_or_creator
                .clone()
                .unwrap_or_else(|| "---".to_string());
            let plat_label = platform.label();
            let arch_badge = archetype.badge_token();
            let is_dup = *is_duplicate_in_ledger;
            let ext_str = target_container.extension();
            let deliv_mode = subtitle_plan.delivery_mode;
            let sel_fmt_idx = *selected_format_idx;
            let fmt_rows: Vec<String> = formats
                .iter()
                .enumerate()
                .map(|(idx, f)| {
                    let prefix = if idx == sel_fmt_idx { SYM_CURSOR } else { " " };
                    format!(
                        "{prefix} {:<8} {:<5} {:<12} v:{:<12} a:{:<10}",
                        f.format_id,
                        f.ext,
                        f.resolution_label,
                        f.vcodec.as_deref().unwrap_or("none"),
                        f.acodec.as_deref().unwrap_or("none")
                    )
                })
                .collect();

            render_kitty_thumbnail_box(
                frame,
                regions.left_thumbnail_area,
                &format!("THUMBNAIL : {}", aspect.token_label()),
                &media_id_s,
                aspect,
                model,
            );

            if let Some(lrc_area) = regions.left_lyrics_area {
                let lrc_text = match &lrc_clone {
                    Some(doc) => doc
                        .lines
                        .iter()
                        .take(14)
                        .map(|l| {
                            format!(
                                "{} {}",
                                l.timestamp_tag.as_deref().unwrap_or("[PLAIN]"),
                                l.text
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    None => "[i] Querying lrclib.net or no synced lyrics available.".to_string(),
                };
                frame.render_widget(
                    Paragraph::new(lrc_text)
                        .block(sharp_centered_panel("LYRICS : LRCLIB BELOW THUMB", false)),
                    lrc_area,
                );
            }

            if let Some(sub_area) = regions.left_chapters_subs_area {
                let sub_summary = if sub_plan_clone.tracks.is_empty() {
                    format!("[SUBTITLES: NONE DETECTED FOR EN/ID]\nChapters: {chap_count}")
                } else {
                    let tracks_str = sub_plan_clone
                        .tracks
                        .iter()
                        .map(|t| {
                            format!(
                                "▪ {} ({}) -> [{}]",
                                t.language.display_name(),
                                t.language.iso_639_2_mkv(),
                                t.origin.badge()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("{tracks_str}\nChapters Embedded: {chap_count}")
                };
                frame.render_widget(
                    Paragraph::new(sub_summary)
                        .block(sharp_centered_panel("SUBTITLES (EN/ID) & CHAPTERS", false)),
                    sub_area,
                );
            }

            let dup_badge = if is_dup {
                "[!] DUPLICATE IN LEDGER"
            } else {
                "[+] NEW MEDIA"
            };
            let meta_summary = format!(
                "TITLE     : {meta_title}   [m: Edit Meta]\nARTIST    : {meta_artist}\nSOURCE    : {plat_label} ({arch_badge}) | {dup_badge}\nCONTAINER : .{ext_str} | SUB DELIVERY: {deliv_mode:?}"
            );
            frame.render_widget(
                Paragraph::new(meta_summary)
                    .block(sharp_centered_panel("EDITABLE METADATA SUMMARY", false)),
                regions.right_top_summary_area,
            );

            let items: Vec<ListItem<'_>> = fmt_rows.into_iter().map(ListItem::new).collect();
            frame.render_widget(
                List::new(items).block(sharp_centered_panel(
                    "GRANULAR STREAM SELECTOR (Enter:Queue  f:Override)",
                    true,
                )),
                regions.right_main_inspector_area,
            );
        }
        PreviewInspectionState::LoadedCollection {
            workspace,
            selected_row_idx,
            active_row_lyrics,
            ..
        } => {
            let active_entry = workspace.entries.get(*selected_row_idx);
            let aspect = active_entry.map_or(AspectRatioKind::Square1x1, |e| e.aspect_ratio);
            let active_id = active_entry
                .map(|e| e.media_id.clone())
                .unwrap_or_else(|| "playlist".to_string());
            let ratio_token = aspect.token_label();
            let lrc_body = active_row_lyrics
                .as_ref()
                .map(|d| {
                    d.lines
                        .iter()
                        .take(14)
                        .map(|l| {
                            format!(
                                "{} {}",
                                l.timestamp_tag.as_deref().unwrap_or("[PLAIN]"),
                                l.text
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| {
                    "[i] Fetching LRCLIB synced lyrics for highlighted track...".to_string()
                });
            let sub_summary = active_entry.map_or_else(
                || "[i] No episode selected.".to_string(),
                |entry| {
                    let plan = workspace.effective_subtitle_plan(entry);
                    if plan.tracks.is_empty() {
                        if entry.hydrated {
                            "[SUBTITLES: NONE DETECTED FOR EN/ID]".to_string()
                        } else {
                            "░░▒▒▓▓ Probing episode EN/ID subtitles...".to_string()
                        }
                    } else {
                        plan.tracks
                            .iter()
                            .map(|t| {
                                format!(
                                    "▪ {} ({}) -> [{}]",
                                    t.language.display_name(),
                                    t.language.iso_639_2_mkv(),
                                    t.origin.badge()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                },
            );
            let policy_summary = format!(
                "PLAYLIST : {} ({} items) | SUBFOLDER: ~/Downloads/mvd/{}/\nGLOBAL   : Container=.{} | MaxHeight={:?} | EN={} ID={}",
                workspace.title,
                workspace.entries.len(),
                workspace.global_policy.subfolder_name,
                workspace.global_policy.container.extension(),
                workspace.global_policy.max_height_ceiling,
                workspace.global_policy.want_sub_en,
                workspace.global_policy.want_sub_id
            );
            let row_strings: Vec<String> = workspace
                .entries
                .iter()
                .enumerate()
                .map(|(idx, e)| {
                    let cursor = if idx == *selected_row_idx {
                        SYM_CURSOR
                    } else {
                        " "
                    };
                    let sel = if e.selected_for_download {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    let badge = workspace
                        .item_overrides
                        .get(&e.media_id)
                        .map_or("[GLOBAL]", |ov| ov.badge_token());
                    format!(
                        "{cursor} {sel} #{:02} {:<34} {:<14} {badge}",
                        e.index_1_based,
                        truncate_str(&e.title, 33),
                        e.aspect_ratio.token_label()
                    )
                })
                .collect();

            render_kitty_thumbnail_box(
                frame,
                regions.left_thumbnail_area,
                &format!("ITEM THUMBNAIL : {ratio_token}"),
                &active_id,
                aspect,
                model,
            );

            if let Some(lrc_area) = regions.left_lyrics_area {
                frame.render_widget(
                    Paragraph::new(lrc_body)
                        .block(sharp_centered_panel("TRACK LYRICS : LRCLIB", false)),
                    lrc_area,
                );
            }

            if let Some(sub_area) = regions.left_chapters_subs_area {
                frame.render_widget(
                    Paragraph::new(sub_summary)
                        .block(sharp_centered_panel("EPISODE SUBTITLES (EN/ID)", false)),
                    sub_area,
                );
            }

            frame.render_widget(
                Paragraph::new(policy_summary).block(sharp_centered_panel(
                    "PLAYLIST GLOBAL POLICY & SUBFOLDER",
                    false,
                )),
                regions.right_top_summary_area,
            );

            let rows: Vec<ListItem<'_>> = row_strings.into_iter().map(ListItem::new).collect();
            frame.render_widget(
                List::new(rows).block(sharp_centered_panel(
                    "PLAYLIST ITEMS (Space:Toggle  Enter:Queue  f:Format[*]  m:Meta[*])",
                    true,
                )),
                regions.right_main_inspector_area,
            );
        }
    }
}

fn render_kitty_thumbnail_box(
    frame: &mut Frame<'_>,
    area: Rect,
    panel_title: &str,
    media_id: &str,
    aspect: AspectRatioKind,
    model: &mut AppModel,
) {
    let block = sharp_centered_panel(panel_title, false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(thumb) = &mut model.active_thumbnail {
        let fitted = model.cell_geometry.fit_centered_rect(inner, aspect);
        let widget = StatefulImage::default().resize(Resize::Fit(Some(FilterType::CatmullRom)));
        frame.render_stateful_widget(widget, fitted, &mut thumb.protocol);
    } else {
        frame.render_widget(
            Paragraph::new(format!(
                "\n░░▒▒▓▓ DECODING KITTY THUMBNAIL ▓▓▒▒░░\nID: {media_id}\nAspect: {}",
                aspect.token_label()
            ))
            .alignment(Alignment::Center)
            .style(Style::default().fg(COLOR_TEXT_MUTED)),
            inner,
        );
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let taken: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{taken}…")
    }
}

fn render_queue_page(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    if model.queue_jobs.is_empty() {
        frame.render_widget(
            Paragraph::new(
                "\n[i] DOWNLOAD QUEUE IS EMPTY\nExtract a URL on Page [1:EXTRACTOR] and press Enter to dispatch jobs.",
            )
            .alignment(Alignment::Center)
            .style(Style::default().fg(COLOR_TEXT_MUTED))
            .block(sharp_centered_panel(
                "DEDICATED CONCURRENT DOWNLOAD QUEUE (c:Cancel  p:Pause/Resume)",
                true,
            )),
            area,
        );
        return;
    }

    let rows: Vec<ListItem<'_>> = model
        .queue_jobs
        .iter()
        .enumerate()
        .map(|(idx, job)| {
            let cur = if idx == model.selected_queue_idx {
                SYM_CURSOR
            } else {
                " "
            };
            let pct = f64::from(job.progress_percent_x10) / 10.0;
            let speed_mib = (job.speed_bytes_per_sec as f64) / (1024.0 * 1024.0);
            ListItem::new(format!(
                "{cur} {:<9} {:<36} .{:<4} {:>5.1}%  {:>5.2} MiB/s",
                job.status.badge(),
                truncate_str(&job.metadata.title, 35),
                job.target_container.extension(),
                pct,
                speed_mib
            ))
        })
        .collect();

    frame.render_widget(
        List::new(rows).block(sharp_centered_panel(
            "DEDICATED CONCURRENT DOWNLOAD QUEUE (c:Cancel  p:Pause/Resume  x:ClearDone)",
            true,
        )),
        area,
    );
}

fn render_logs_page(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let lines: Vec<ListItem<'_>> = model
        .raw_subprocess_lines
        .iter()
        .rev()
        .take(35)
        .map(|l| ListItem::new(l.as_str()))
        .collect();

    frame.render_widget(
        List::new(lines).block(sharp_centered_panel(
            "RAW SUBPROCESS OUTPUT (yt-dlp + ffmpeg) [x:ClearLogsInMemory]",
            true,
        )),
        area,
    );
}

fn render_history_page(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let rows: Vec<ListItem<'_>> = model
        .history_records
        .iter()
        .enumerate()
        .map(|(idx, rec)| {
            let cur = if idx == model.selected_history_idx {
                SYM_CURSOR
            } else {
                " "
            };
            ListItem::new(format!(
                "{cur} [{:<12}] {:<36} (.{}) -> {}",
                rec.platform,
                truncate_str(&rec.title, 35),
                rec.container,
                rec.output_path
            ))
        })
        .collect();

    frame.render_widget(
        List::new(rows).block(sharp_centered_panel(
            "SQLITE HISTORY LEDGER (X:ClearDbRecordsOnly — Files in ~/Downloads/mvd Kept Safe)",
            true,
        )),
        area,
    );
}

fn render_settings_page(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    let text = format!(
        "SAFE OUTPUT PATH       : {}\n\
         FIREFOX SANDBOX LOCK   : {} (Work Profiles Strictly Blocked)\n\
         EPHEMERAL COOKIE VAULT : /run/user/$UID/mvd/vault (RAM tmpfs 0700/0600, ZeroizeOnDrop)\n\
         DEFAULT CONTAINERS     : Video=.{} | Audio=.{}\n\
         SUBTITLE OPTIMIZATION  : EN={} | ID={} | Auto-ASR Fallback={}\n\
         CACHE QUOTA            : {} MiB Max (Current Usage: {} KiB)",
        model.config.download_dir.display(),
        model.config.allowed_firefox_profile,
        model.config.default_video_container.extension(),
        model.config.default_audio_container.extension(),
        model.config.prefer_sub_en,
        model.config.prefer_sub_id,
        model.config.allow_auto_sub_fallback,
        model.config.max_cache_quota_mib,
        model.cache_usage_bytes / 1024
    );

    frame.render_widget(
        Paragraph::new(text).block(sharp_centered_panel(
            "SETTINGS, XDG PATHS & pbf03 SECURITY AUDIT",
            true,
        )),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
    // Strictly omits universal default keys (j/k, Esc) per user rule
    let hints = ConditionalKeyHints::footer_hints_for(model);
    let (badge_style, status_str) = match &model.status_banner {
        Some((sev, msg)) => {
            let col = match sev {
                crate::domain::error::Severity::Info => COLOR_STATUS_INFO,
                crate::domain::error::Severity::Warning => COLOR_STATUS_WARN,
                crate::domain::error::Severity::Error => COLOR_STATUS_ERROR,
                crate::domain::error::Severity::Success => COLOR_ACCENT_EMERALD,
            };
            (
                Style::default().fg(col).add_modifier(Modifier::BOLD),
                format!("{} {msg}", sev.badge()),
            )
        }
        None => (
            Style::default().fg(COLOR_ACCENT_EMERALD),
            "[+] READY".to_string(),
        ),
    };

    let line = Line::from(vec![
        Span::styled(format!(" {hints} "), Style::default().fg(COLOR_TEXT_MUTED)),
        Span::raw(" │ "),
        Span::styled(status_str, badge_style),
    ]);

    frame.render_widget(
        Paragraph::new(line).block(sharp_centered_panel("CONTEXT ACTIONS", false)),
        area,
    );
}

fn render_centered_modal(frame: &mut Frame<'_>, screen: Rect, modal: &ActiveModal) {
    let popup_area = centered_rect(64, 42, screen);
    frame.render_widget(Clear, popup_area);

    let (title, body) = match modal {
        ActiveModal::ConfirmDetachOrExit { active_job_count } => (
            "DETACH TUI TO CLI PROGRESS MODE",
            format!(
                "\n[!] {active_job_count} download worker(s) currently active.\n\n\
                 Exiting TUI via 'Q' or 'Ctrl+C' will NOT cancel active downloads.\n\
                 MVD will switch to live CLI progress output in your terminal.\n\
                 (Pressing Ctrl+C in CLI mode stops workers and preserves .part\n\
                  files so you can resume single or playlist jobs next time).\n\n\
                 [ ENTER / Y : DETACH TO CLI ]      [ N : STAY IN TUI ]"
            ),
        ),
        ActiveModal::ResumeUnfinishedQueuePrompt { unfinished_count } => (
            "UNFINISHED QUEUE DETECTED",
            format!(
                "\n[i] {unfinished_count} unfinished download job(s) recovered from state ledger.\n\n\
                 Completed playlist tracks will be skipped automatically.\n\
                 Interrupted items have .part checkpoints ready for resume.\n\n\
                 [ ENTER : RESUME FROM .PART ]   [ R : RESTART FROM SCRATCH ]   [ d : DISCARD ]"
            ),
        ),
        ActiveModal::UnqueuedEditsGuard { pending_new_url } => (
            "UNQUEUED EDITS ON CURRENT MEDIA",
            format!(
                "\n[!] You have unsaved metadata/stream overrides on the current preview.\n\
                 New URL: {pending_new_url}\n\n\
                 [ d : DISCARD EDITS & LOAD NEW URL ]"
            ),
        ),
        ActiveModal::MetadataEditor {
            draft,
            active_field_idx,
            ..
        } => {
            let f0 = if *active_field_idx == 0 {
                SYM_CURSOR
            } else {
                " "
            };
            let f1 = if *active_field_idx == 1 {
                SYM_CURSOR
            } else {
                " "
            };
            let f2 = if *active_field_idx == 2 {
                SYM_CURSOR
            } else {
                " "
            };
            let f3 = if *active_field_idx == 3 {
                SYM_CURSOR
            } else {
                " "
            };
            (
                "METADATA & TAG EDITOR [*] (Tab:NextField)",
                format!(
                    "\n{f0} TITLE  : {}\n{f1} ARTIST : {}\n{f2} ALBUM  : {}\n{f3} YEAR   : {}\n\n[ ENTER : APPLY OVERRIDE [*] ]",
                    draft.title,
                    draft.artist_or_creator.as_deref().unwrap_or(""),
                    draft.album.as_deref().unwrap_or(""),
                    draft.release_year.as_deref().unwrap_or("")
                ),
            )
        }
        ActiveModal::GranularStreamOverride {
            playlist_item_index,
            selected_format_idx,
        } => (
            "GRANULAR STREAM & SUBTITLE OVERRIDE [*]",
            format!(
                "\nTarget Item : {playlist_item_index:?}\nSelected Stream Index : #{selected_format_idx}\n\
                 Subtitles : EN + ID (Manual > Auto-ASR Fallback)\n\n\
                 [ ENTER : CONFIRM OVERRIDE [*] ]"
            ),
        ),
        ActiveModal::QuickEnqueueDialog { preset_idx, .. } => (
            "QUICK ENQUEUE FROM CLIPBOARD",
            format!(
                "\nPreset #{preset_idx}: [1080p MKV + EN/ID Subtitles / Opus 160k + LRC]\n\n\
                 [ ENTER : ENQUEUE IMMEDIATELY ]"
            ),
        ),
        ActiveModal::ConfirmClearHistoryDbOnly => (
            "CLEAR SQLITE HISTORY LEDGER ONLY",
            "\n[i] Clears records in ~/.local/share/mvd/history.db ONLY.\n\
             Downloaded media files in ~/Downloads/mvd/ are NEVER deleted.\n\n\
             [ ENTER / Y : PURGE DB RECORDS ]    [ N : CANCEL ]"
                .to_string(),
        ),
        ActiveModal::ErrorDiagnosticInspector {
            job_title,
            error_detail,
        } => (
            "ERROR DIAGNOSTIC INSPECTOR",
            format!(
                "\nJob: {job_title}\nError: {error_detail}\n\n[ r : RETRY WITH pbf03 COOKIES ]"
            ),
        ),
    };

    frame.render_widget(
        Paragraph::new(body)
            .alignment(Alignment::Center)
            .block(sharp_centered_panel(title, true)),
        popup_area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(v[1])[1]
}
