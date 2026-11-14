use crate::app::message::{Command, Message};
use crate::app::model::{
    ActiveModal, AppModel, ExitTransition, InputMode, PreviewInspectionState, WorkspacePage,
};
use crate::domain::error::TelemetryEvent;
use crate::domain::playlist::PlaylistItemOverride;
use crate::domain::queue_item::JobStatus;
use crate::engine::parser::UrlParser;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Pure deterministic Elm Architecture `update` transition function.
pub fn update(model: &mut AppModel, msg: Message) -> Vec<Command> {
    let mut commands = Vec::new();

    match msg {
        Message::TerminalResized(_, _) => {
            model.dirty = true;
        }
        Message::KeyPressed(key) => {
            commands.extend(handle_key_event(model, key));
            model.dirty = true;
        }
        Message::PasteClipboardText(clip_text) => {
            commands.extend(ingest_new_url_candidate(model, clip_text));
            model.dirty = true;
        }
        Message::TriggerUrlExtraction(raw_url) => {
            if let Ok(parsed) = UrlParser::parse_and_normalize(&raw_url) {
                if parsed.platform.recommends_ephemeral_privacy() {
                    model.ephemeral_privacy_mode = true;
                }
                model.url_input_buffer.clone_from(&parsed.normalized_url);
                model.has_unqueued_edits = false;
                model.inspection = PreviewInspectionState::FetchingSkeleton {
                    target_url: parsed.normalized_url.clone(),
                    platform: parsed.platform,
                    expected_archetype: parsed.initial_archetype,
                };
                commands.push(Command::SpawnExtractor {
                    normalized_url: parsed.normalized_url,
                    use_pbf03_cookies: model.pbf03_cookie_enabled,
                });
            }
            model.dirty = true;
        }
        Message::RawSubprocessLine(line) => {
            if model.raw_subprocess_lines.len() >= 10_000 {
                model.raw_subprocess_lines.remove(0);
            }
            model.raw_subprocess_lines.push(line);
            if model.active_page == WorkspacePage::RawTelemetryLogs {
                model.dirty = true;
            }
        }
        Message::LogEvent {
            severity,
            module,
            summary,
        } => {
            model.status_banner = Some((severity, summary.clone()));
            model.telemetry_events.push(TelemetryEvent {
                timestamp_epoch_ms: 0,
                severity,
                source_module: module,
                summary,
                raw_detail: None,
            });
            model.dirty = true;
        }
        Message::JobProgressUpdated {
            job_id,
            percent_x10,
            speed_bps,
            eta_secs,
        } => {
            if let Some(job) = model.queue_jobs.iter_mut().find(|j| j.job_id == job_id) {
                job.status = JobStatus::ActiveDownloading;
                job.progress_percent_x10 = percent_x10;
                job.speed_bytes_per_sec = speed_bps;
                job.eta_seconds = eta_secs;
                model.dirty = true;
            }
        }
        Message::JobCompleted { job_id } => {
            if let Some(job) = model.queue_jobs.iter_mut().find(|j| j.job_id == job_id) {
                job.status = JobStatus::Completed;
                job.progress_percent_x10 = 1000;
                job.speed_bytes_per_sec = 0;
                commands.push(Command::FlushQueueStateSnapshot);
                model.dirty = true;
            }
        }
        Message::JobFailed { job_id, error } => {
            if let Some(job) = model.queue_jobs.iter_mut().find(|j| j.job_id == job_id) {
                job.status = JobStatus::Failed;
                job.speed_bytes_per_sec = 0;
                job.last_error = Some(error);
                commands.push(Command::FlushQueueStateSnapshot);
                model.dirty = true;
            }
        }
    }

    commands
}

fn handle_key_event(model: &mut AppModel, key: KeyEvent) -> Vec<Command> {
    let mut cmds = Vec::new();

    // 1. Ctrl+C inside TUI behaves identically to uppercase 'Q' (opens Detach/Exit Confirmation Modal)
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        open_detach_or_exit_modal(model);
        return cmds;
    }

    // 2. If a floating modal is open, route keys to the modal controller
    if model.active_modal.is_some() {
        return handle_modal_keys(model, key);
    }

    // 3. Handle InsertUrl / FilterSearch text input modes
    match model.input_mode {
        InputMode::InsertUrl => {
            match key.code {
                KeyCode::Esc => {
                    model.input_mode = InputMode::Normal;
                }
                KeyCode::Enter => {
                    model.input_mode = InputMode::Normal;
                    let url = model.url_input_buffer.trim().to_string();
                    if !url.is_empty() {
                        cmds.extend(update(model, Message::TriggerUrlExtraction(url)));
                    }
                }
                KeyCode::Backspace => {
                    model.url_input_buffer.pop();
                }
                KeyCode::Char(ch) => {
                    model.url_input_buffer.push(ch);
                }
                _ => {}
            }
            return cmds;
        }
        InputMode::FilterSearch => {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    model.input_mode = InputMode::Normal;
                }
                KeyCode::Backspace => {
                    model.filter_query_buffer.pop();
                }
                KeyCode::Char(ch) => {
                    model.filter_query_buffer.push(ch);
                }
                _ => {}
            }
            return cmds;
        }
        InputMode::ModalDialog => return cmds,
        InputMode::Normal => {}
    }

    // 4. Normal Mode Conditional Keybindings
    match key.code {
        // Uppercase 'Q': Detach TUI to CLI Progress Mode (with warning modal) or exit
        KeyCode::Char('Q') => {
            open_detach_or_exit_modal(model);
        }
        // Lowercase 'q': Step back to Extractor page (never quits TUI)
        KeyCode::Char('q') => {
            if model.active_page != WorkspacePage::ExtractorPreview {
                model.active_page = WorkspacePage::ExtractorPreview;
            }
        }
        // Page switching (1..5 and Tab)
        KeyCode::Char('1') => model.active_page = WorkspacePage::ExtractorPreview,
        KeyCode::Char('2') => model.active_page = WorkspacePage::QueueManager,
        KeyCode::Char('3') => model.active_page = WorkspacePage::RawTelemetryLogs,
        KeyCode::Char('4') => model.active_page = WorkspacePage::HistoryLedger,
        KeyCode::Char('5') => model.active_page = WorkspacePage::SettingsSecurity,
        KeyCode::Tab => {
            let pages = WorkspacePage::all();
            let idx = pages
                .iter()
                .position(|&p| p == model.active_page)
                .unwrap_or(0);
            model.active_page = pages[(idx + 1) % pages.len()];
        }
        // Toggle Sandbox Cookie (pbf03)
        KeyCode::Char('C') => {
            model.pbf03_cookie_enabled = !model.pbf03_cookie_enabled;
        }
        // Toggle Ephemeral Privacy Mode (Incognito Ledger for adult/sensitive sites)
        KeyCode::Char('I') => {
            model.ephemeral_privacy_mode = !model.ephemeral_privacy_mode;
        }
        // Universal vertical navigation ('j'/'k' or Down/Up)
        KeyCode::Char('j') | KeyCode::Down => navigate_cursor(model, 1),
        KeyCode::Char('k') | KeyCode::Up => navigate_cursor(model, -1),

        // Page-specific actions
        _ => cmds.extend(handle_page_specific_key(model, key.code)),
    }

    cmds
}

fn open_detach_or_exit_modal(model: &mut AppModel) {
    let active_job_count = model.active_download_count();
    model.active_modal = Some(ActiveModal::ConfirmDetachOrExit { active_job_count });
    model.input_mode = InputMode::ModalDialog;
}

fn handle_modal_keys(model: &mut AppModel, key: KeyEvent) -> Vec<Command> {
    let mut cmds = Vec::new();
    // 'Esc' always closes the active floating modal and returns to Normal mode
    if key.code == KeyCode::Esc {
        model.active_modal = None;
        model.input_mode = InputMode::Normal;
        return cmds;
    }

    let Some(modal) = model.active_modal.clone() else {
        return cmds;
    };

    match modal {
        ActiveModal::ConfirmDetachOrExit { active_job_count } => match key.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
                if active_job_count > 0 {
                    model.exit_transition = ExitTransition::DetachToCliProgress;
                    cmds.push(Command::TransitionToCliProgressMode);
                } else {
                    model.exit_transition = ExitTransition::CleanExitImmediately;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
            }
            _ => {}
        },
        ActiveModal::ResumeUnfinishedQueuePrompt { .. } => match key.code {
            KeyCode::Enter => {
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
                cmds.push(Command::ResumeRecoveredQueue);
            }
            KeyCode::Char('R') | KeyCode::Char('r') => {
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
                cmds.push(Command::RestartRecoveredQueueFromScratch);
            }
            KeyCode::Char('d') => {
                model.queue_jobs.clear();
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
                cmds.push(Command::FlushQueueStateSnapshot);
            }
            _ => {}
        },
        ActiveModal::UnqueuedEditsGuard { pending_new_url } => {
            if key.code == KeyCode::Char('d') {
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
                model.has_unqueued_edits = false;
                cmds.extend(update(
                    model,
                    Message::TriggerUrlExtraction(pending_new_url),
                ));
            }
        }
        ActiveModal::MetadataEditor {
            playlist_item_index,
            draft,
            ..
        } => {
            if key.code == KeyCode::Enter {
                apply_metadata_draft(model, playlist_item_index, draft);
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
            }
        }
        ActiveModal::GranularStreamOverride {
            playlist_item_index,
            selected_format_idx,
        } => {
            if key.code == KeyCode::Enter {
                apply_stream_override(model, playlist_item_index, selected_format_idx);
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
            }
        }
        ActiveModal::ConfirmClearHistoryDbOnly => match key.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                model.history_records.clear();
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
                cmds.push(Command::ClearHistoryLedgerDbOnly);
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
            }
            _ => {}
        },
        ActiveModal::QuickEnqueueDialog { .. } | ActiveModal::ErrorDiagnosticInspector { .. } => {
            if key.code == KeyCode::Enter {
                model.active_modal = None;
                model.input_mode = InputMode::Normal;
            }
        }
    }

    cmds
}

fn handle_page_specific_key(model: &mut AppModel, code: KeyCode) -> Vec<Command> {
    let mut cmds = Vec::new();
    match model.active_page {
        WorkspacePage::ExtractorPreview => match code {
            KeyCode::Char('n') => {
                model.url_input_buffer.clear();
                model.input_mode = InputMode::InsertUrl;
            }
            KeyCode::Char('p') => {
                cmds.push(Command::ReadWaylandClipboard);
            }
            KeyCode::Char('D') => {
                model.active_modal = Some(ActiveModal::QuickEnqueueDialog {
                    url_input: String::new(),
                    preset_idx: 0,
                });
                model.input_mode = InputMode::ModalDialog;
            }
            KeyCode::Char('m') => open_metadata_editor_for_selection(model),
            KeyCode::Char('f') => open_stream_override_for_selection(model),
            _ => {}
        },
        WorkspacePage::QueueManager => match code {
            // 'c' cancels the selected job ONLY inside the TUI Queue page
            KeyCode::Char('c') => {
                if let Some(job) = model.queue_jobs.get_mut(model.selected_queue_idx) {
                    job.status = JobStatus::CancelledInTui;
                    job.speed_bytes_per_sec = 0;
                    let job_id = job.job_id.clone();
                    cmds.push(Command::CancelJobInTui { job_id });
                    cmds.push(Command::FlushQueueStateSnapshot);
                }
            }
            KeyCode::Char('p') => {
                if let Some(job) = model.queue_jobs.get(model.selected_queue_idx) {
                    cmds.push(Command::PauseOrResumeJob {
                        job_id: job.job_id.clone(),
                    });
                }
            }
            KeyCode::Char('x') => {
                model.queue_jobs.retain(|j| {
                    !matches!(j.status, JobStatus::Completed | JobStatus::CancelledInTui)
                });
                cmds.push(Command::FlushQueueStateSnapshot);
            }
            _ => {}
        },
        WorkspacePage::RawTelemetryLogs => match code {
            // Non-destructive in-memory log clear
            KeyCode::Char('x') => {
                model.raw_subprocess_lines.clear();
                model.telemetry_events.clear();
            }
            KeyCode::Char('/') => {
                model.input_mode = InputMode::FilterSearch;
            }
            _ => {}
        },
        WorkspacePage::HistoryLedger => match code {
            // Non-destructive SQLite DB clear (never deletes files in ~/Downloads/mvd/)
            KeyCode::Char('X') => {
                model.active_modal = Some(ActiveModal::ConfirmClearHistoryDbOnly);
                model.input_mode = InputMode::ModalDialog;
            }
            KeyCode::Char('/') => {
                model.input_mode = InputMode::FilterSearch;
            }
            _ => {}
        },
        WorkspacePage::SettingsSecurity => {
            if code == KeyCode::Char('X') {
                cmds.push(Command::PurgeXdgCache);
            }
        }
    }
    cmds
}

fn ingest_new_url_candidate(model: &mut AppModel, candidate_url: String) -> Vec<Command> {
    if model.has_unqueued_edits {
        model.active_modal = Some(ActiveModal::UnqueuedEditsGuard {
            pending_new_url: candidate_url,
        });
        model.input_mode = InputMode::ModalDialog;
        Vec::new()
    } else {
        update(model, Message::TriggerUrlExtraction(candidate_url))
    }
}

fn navigate_cursor(model: &mut AppModel, delta: isize) {
    match model.active_page {
        WorkspacePage::ExtractorPreview => match &mut model.inspection {
            PreviewInspectionState::LoadedSingle {
                formats,
                selected_format_idx,
                ..
            } if !formats.is_empty() => {
                *selected_format_idx = step_index(*selected_format_idx, formats.len(), delta);
            }
            PreviewInspectionState::LoadedCollection {
                workspace,
                selected_row_idx,
                ..
            } if !workspace.entries.is_empty() => {
                *selected_row_idx = step_index(*selected_row_idx, workspace.entries.len(), delta);
            }
            _ => {}
        },
        WorkspacePage::QueueManager if !model.queue_jobs.is_empty() => {
            model.selected_queue_idx =
                step_index(model.selected_queue_idx, model.queue_jobs.len(), delta);
        }
        WorkspacePage::HistoryLedger if !model.history_records.is_empty() => {
            model.selected_history_idx = step_index(
                model.selected_history_idx,
                model.history_records.len(),
                delta,
            );
        }
        _ => {}
    }
}

fn step_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let cur = isize::try_from(current).unwrap_or(0);
    let max = isize::try_from(len.saturating_sub(1)).unwrap_or(0);
    let next = (cur + delta).clamp(0, max);
    usize::try_from(next).unwrap_or(0)
}

fn open_metadata_editor_for_selection(model: &mut AppModel) {
    match &model.inspection {
        PreviewInspectionState::LoadedSingle { metadata, .. } => {
            model.active_modal = Some(ActiveModal::MetadataEditor {
                playlist_item_index: None,
                draft: metadata.clone(),
                active_field_idx: 0,
            });
            model.input_mode = InputMode::ModalDialog;
        }
        PreviewInspectionState::LoadedCollection {
            workspace,
            selected_row_idx,
            ..
        } => {
            if let Some(entry) = workspace.entries.get(*selected_row_idx) {
                let existing = workspace
                    .item_overrides
                    .get(&entry.media_id)
                    .and_then(|ov| ov.custom_metadata.clone())
                    .unwrap_or_else(|| {
                        let mut meta = crate::domain::metadata::EditableMetadata::default_for_title(
                            &entry.title,
                        );
                        meta.artist_or_creator.clone_from(&entry.channel_or_artist);
                        meta.track_number = u32::try_from(entry.index_1_based).ok();
                        meta
                    });
                model.active_modal = Some(ActiveModal::MetadataEditor {
                    playlist_item_index: Some(*selected_row_idx),
                    draft: existing,
                    active_field_idx: 0,
                });
                model.input_mode = InputMode::ModalDialog;
            }
        }
        _ => {}
    }
}

fn open_stream_override_for_selection(model: &mut AppModel) {
    match &model.inspection {
        PreviewInspectionState::LoadedSingle {
            selected_format_idx,
            ..
        } => {
            model.active_modal = Some(ActiveModal::GranularStreamOverride {
                playlist_item_index: None,
                selected_format_idx: *selected_format_idx,
            });
            model.input_mode = InputMode::ModalDialog;
        }
        PreviewInspectionState::LoadedCollection {
            selected_row_idx, ..
        } => {
            model.active_modal = Some(ActiveModal::GranularStreamOverride {
                playlist_item_index: Some(*selected_row_idx),
                selected_format_idx: 0,
            });
            model.input_mode = InputMode::ModalDialog;
        }
        _ => {}
    }
}

fn apply_metadata_draft(
    model: &mut AppModel,
    playlist_item_index: Option<usize>,
    draft: crate::domain::metadata::EditableMetadata,
) {
    match &mut model.inspection {
        PreviewInspectionState::LoadedSingle { metadata, .. } => {
            *metadata = draft;
            model.has_unqueued_edits = true;
        }
        PreviewInspectionState::LoadedCollection { workspace, .. } => {
            if let Some(idx) = playlist_item_index {
                if let Some(entry) = workspace.entries.get_mut(idx) {
                    entry.title.clone_from(&draft.title);
                    let ov = workspace
                        .item_overrides
                        .entry(entry.media_id.clone())
                        .or_insert_with(|| PlaylistItemOverride {
                            custom_video_format_id: None,
                            custom_audio_format_id: None,
                            custom_container: None,
                            custom_sub_en: None,
                            custom_sub_id: None,
                            custom_metadata: None,
                        });
                    ov.custom_metadata = Some(draft);
                    model.has_unqueued_edits = true;
                }
            }
        }
        _ => {}
    }
}

fn apply_stream_override(
    model: &mut AppModel,
    playlist_item_index: Option<usize>,
    selected_format_idx: usize,
) {
    match &mut model.inspection {
        PreviewInspectionState::LoadedSingle {
            selected_format_idx: cur,
            ..
        } => {
            *cur = selected_format_idx;
            model.has_unqueued_edits = true;
        }
        PreviewInspectionState::LoadedCollection { workspace, .. } => {
            if let Some(idx) = playlist_item_index {
                if let Some(entry) = workspace.entries.get(idx) {
                    let fmt_id = entry
                        .available_formats
                        .get(selected_format_idx)
                        .map(|f| f.format_id.clone())
                        .or_else(|| Some("bestvideo+bestaudio".to_string()));
                    let ov = workspace
                        .item_overrides
                        .entry(entry.media_id.clone())
                        .or_insert_with(|| PlaylistItemOverride {
                            custom_video_format_id: None,
                            custom_audio_format_id: None,
                            custom_container: None,
                            custom_sub_en: None,
                            custom_sub_id: None,
                            custom_metadata: None,
                        });
                    ov.custom_video_format_id = fmt_id;
                    model.has_unqueued_edits = true;
                }
            }
        }
        _ => {}
    }
}
