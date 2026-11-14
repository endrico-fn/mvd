use crate::app::keymap::ConditionalKeyHints;
use crate::app::model::{ActiveModal, AppModel, InputMode, PreviewInspectionState, WorkspacePage};
use crate::domain::media::MediaArchetype;
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

/// Pure side-effect-free Ratatui root renderer.
pub fn render(frame: &mut Frame<'_>, model: &AppModel) {
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

fn render_extractor_page(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
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

    // 2. Left Preview Column (Thumbnail + Conditional Lyrics or Chapters/Subs)
    match &model.inspection {
        PreviewInspectionState::Empty => {
            frame.render_widget(
                Paragraph::new("\n[i] NO MEDIA LOADED\nPaste a YouTube, YT Music, Spotify, X, Reddit, IG, or Pinterest link.")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(COLOR_TEXT_MUTED))
                    .block(sharp_centered_panel("KITTY PREVIEW : STANDBY", false)),
                regions.left_thumbnail_area,
            );
            frame.render_widget(
                Paragraph::new("Metadata inspector awaits URL extraction.")
                    .style(Style::default().fg(COLOR_TEXT_MUTED))
                    .block(sharp_centered_panel("MEDIA METADATA", false)),
                regions.right_top_summary_area,
            );
            frame.render_widget(
                Paragraph::new("No stream matrix loaded.")
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
            frame.render_widget(
                Paragraph::new(format!(
                    "Target: {target_url}\n░░▒▒▓▓ Hydrating JSON metadata..."
                ))
                .block(sharp_centered_panel("EXTRACTOR TELEMETRY", false)),
                regions.right_top_summary_area,
            );
            frame.render_widget(
                Paragraph::new("░░▒▒▓▓ Probing video/audio codecs & EN/ID subtitle tracks...")
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
            lyrics,
            is_duplicate_in_ledger,
            ..
        } => {
            let thumb_title = format!("THUMBNAIL : {}", aspect_ratio.token_label());
            frame.render_widget(
                Paragraph::new(format!(
                    "\n[KITTY GRAPHICS PROTOCOL ACTIVE]\nID: {media_id}\nAspect: {}",
                    aspect_ratio.token_label()
                ))
                .alignment(Alignment::Center)
                .block(sharp_centered_panel(&thumb_title, false)),
                regions.left_thumbnail_area,
            );

            if let Some(lrc_area) = regions.left_lyrics_area {
                let lrc_text = match lyrics {
                    Some(doc) => doc
                        .lines
                        .iter()
                        .take(8)
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
                let sub_summary = if subtitle_plan.tracks.is_empty() {
                    "[SUBTITLES: NONE DETECTED FOR EN/ID]".to_string()
                } else {
                    subtitle_plan
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
                        .join("\n")
                };
                frame.render_widget(
                    Paragraph::new(sub_summary)
                        .block(sharp_centered_panel("SUBTITLES (EN/ID) & CHAPTERS", false)),
                    sub_area,
                );
            }

            let dup_badge = if *is_duplicate_in_ledger {
                "[!] DUPLICATE IN LEDGER"
            } else {
                "[+] NEW MEDIA"
            };
            let meta_summary = format!(
                "TITLE     : {}   [m: Edit Meta]\nSOURCE    : {} ({}) | {}\nCONTAINER : .{} | SUB DELIVERY: {:?}",
                metadata.title,
                platform.label(),
                archetype.badge_token(),
                dup_badge,
                target_container.extension(),
                subtitle_plan.delivery_mode
            );
            frame.render_widget(
                Paragraph::new(meta_summary)
                    .block(sharp_centered_panel("EDITABLE METADATA SUMMARY", false)),
                regions.right_top_summary_area,
            );

            let items: Vec<ListItem<'_>> = formats
                .iter()
                .enumerate()
                .map(|(idx, f)| {
                    let prefix = if idx == *selected_format_idx {
                        SYM_CURSOR
                    } else {
                        " "
                    };
                    ListItem::new(format!(
                        "{prefix} {:<6} {:<5} {:<10} v:{:<12} a:{:<10}",
                        f.format_id,
                        f.ext,
                        f.resolution_label,
                        f.vcodec.as_deref().unwrap_or("none"),
                        f.acodec.as_deref().unwrap_or("none")
                    ))
                })
                .collect();
            frame.render_widget(
                List::new(items).block(sharp_centered_panel("GRANULAR STREAM SELECTOR", true)),
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
            let ratio_token = active_entry
                .map(|e| e.aspect_ratio.token_label())
                .unwrap_or("1:1 / 16:9");
            frame.render_widget(
                Paragraph::new(format!(
                    "\n[KITTY GRAPHICS : ITEM PREVIEW]\nRatio: {ratio_token} (Native As-Is)"
                ))
                .alignment(Alignment::Center)
                .block(sharp_centered_panel(
                    &format!("ITEM THUMBNAIL : {ratio_token}"),
                    false,
                )),
                regions.left_thumbnail_area,
            );

            if let Some(lrc_area) = regions.left_lyrics_area {
                let lrc_body = active_row_lyrics
                    .as_ref()
                    .map(|d| d.raw_lrc.clone())
                    .unwrap_or_else(|| "[i] Highlighted track LRCLIB preview.".to_string());
                frame.render_widget(
                    Paragraph::new(lrc_body)
                        .block(sharp_centered_panel("TRACK LYRICS : LRCLIB", false)),
                    lrc_area,
                );
            }

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
            frame.render_widget(
                Paragraph::new(policy_summary).block(sharp_centered_panel(
                    "PLAYLIST GLOBAL POLICY & SUBFOLDER",
                    false,
                )),
                regions.right_top_summary_area,
            );

            let rows: Vec<ListItem<'_>> = workspace
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
                    ListItem::new(format!(
                        "{cursor} {sel} #{:02} {:<34} {:<14} {badge}",
                        e.index_1_based,
                        e.title,
                        e.aspect_ratio.token_label()
                    ))
                })
                .collect();
            frame.render_widget(
                List::new(rows).block(sharp_centered_panel(
                    "PLAYLIST ITEMS (f:OverrideFormat[*]  m:OverrideMeta[*])",
                    true,
                )),
                regions.right_main_inspector_area,
            );
        }
    }
}

fn render_queue_page(frame: &mut Frame<'_>, area: Rect, model: &AppModel) {
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
            ListItem::new(format!(
                "{cur} {:<9} {:<38} .{:<4} {:>5.1}%",
                job.status.badge(),
                job.metadata.title,
                job.target_container.extension(),
                pct
            ))
        })
        .collect();

    frame.render_widget(
        List::new(rows).block(sharp_centered_panel(
            "DEDICATED CONCURRENT DOWNLOAD QUEUE (c:Cancel  p:Pause/Resume)",
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
        .take(30)
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
                rec.platform, rec.title, rec.container, rec.output_path
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
        ActiveModal::MetadataEditor { draft, .. } => (
            "METADATA & TAG EDITOR [*]",
            format!(
                "\nTITLE  : {}\nARTIST : {}\nALBUM  : {}\nYEAR   : {}\n\n[ ENTER : APPLY OVERRIDE [*] ]",
                draft.title,
                draft.artist_or_creator.as_deref().unwrap_or("---"),
                draft.album.as_deref().unwrap_or("---"),
                draft.release_year.as_deref().unwrap_or("---")
            ),
        ),
        ActiveModal::GranularStreamOverride {
            playlist_item_index,
            selected_format_idx,
        } => (
            "GRANULAR STREAM & SUBTITLE OVERRIDE [*]",
            format!(
                "\nTarget Item : {:?}\nSelected Stream Index : #{selected_format_idx}\n\
                 Subtitles : EN + ID (Manual > Auto-ASR Fallback)\n\n\
                 [ ENTER : CONFIRM OVERRIDE [*] ]",
                playlist_item_index
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
            format!("\nJob: {job_title}\nError: {error_detail}\n\n[ r : RETRY WITH pbf03 COOKIES ]"),
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
