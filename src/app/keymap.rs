use crate::app::model::{ActiveModal, AppModel, InputMode, PreviewInspectionState, WorkspacePage};

/// Provides context-sensitive Footer action hints while strictly obeying the rule:
/// NEVER display universal default keys (`j/k`, `Up/Down`, `Esc`) in the Footer Bar.
pub struct ConditionalKeyHints;

impl ConditionalKeyHints {
    #[must_use]
    pub fn footer_hints_for(model: &AppModel) -> &'static str {
        if let Some(modal) = &model.active_modal {
            return match modal {
                ActiveModal::ConfirmDetachOrExit { .. } => "Enter/Y:DetachToCLI  N:StayInTUI",
                ActiveModal::ResumeUnfinishedQueuePrompt { .. } => {
                    "Enter:ResumeFromPart  R:RestartFromScratch  d:Discard"
                }
                ActiveModal::UnqueuedEditsGuard { .. } => {
                    "Enter:QueueCurrent&LoadNew  d:DiscardEdits&LoadNew"
                }
                ActiveModal::QuickEnqueueDialog { .. } => "Tab:CyclePreset  Enter:EnqueueNow",
                ActiveModal::MetadataEditor { .. } => "Tab:NextField  Enter:SaveOverride[*]",
                ActiveModal::GranularStreamOverride { .. } => {
                    "Space:SelectStream  s:ToggleSub(EN/ID)  Enter:ApplyOverride[*]"
                }
                ActiveModal::ConfirmClearHistoryDbOnly => {
                    "Enter/Y:PurgeSqliteRowsOnly(KeepFiles)  N:Abort"
                }
                ActiveModal::ErrorDiagnosticInspector { .. } => "r:RetryJobWithCookie(pbf03)",
            };
        }

        match model.input_mode {
            InputMode::InsertUrl => "Enter:ExtractURL  Ctrl+V:PasteClipboard",
            InputMode::FilterSearch => "Enter:ApplyFilter",
            InputMode::ModalDialog => "Enter:Confirm",
            InputMode::Normal => match model.active_page {
                WorkspacePage::ExtractorPreview => match &model.inspection {
                    PreviewInspectionState::Empty
                    | PreviewInspectionState::FetchingSkeleton { .. } => {
                        "n:InputURL  p:PasteURL  D:QuickEnqueue  C:Cookie(pbf03)  I:Privacy  Tab:Page  Q:DetachCLI"
                    }
                    PreviewInspectionState::LoadedSingle { .. } => {
                        "Enter:Queue  f:Stream/Sub  m:EditMeta  p:NewPaste  D:QuickDl  C:Cookie(pbf03)  I:Privacy  Q:DetachCLI"
                    }
                    PreviewInspectionState::LoadedCollection { .. } => {
                        "Enter:QueueSelected  Space:ToggleItem  a:All  f:ItemFormat[*]  m:ItemMeta[*]  M:Folder  Q:DetachCLI"
                    }
                },
                WorkspacePage::QueueManager => {
                    "p:Pause/Resume  c:CancelJob  r:Retry  +/-:Priority  L:RateLimit  x:ClearDone  Q:DetachCLI"
                }
                WorkspacePage::RawTelemetryLogs => {
                    "/:FilterRegex  x:ClearLogsInMemory  Tab:Page  Q:DetachCLI"
                }
                WorkspacePage::HistoryLedger => {
                    "/:SearchLedger  Enter:ReQueue  x:RemoveRowDbOnly  X:ClearAllHistoryDbOnly  Q:DetachCLI"
                }
                WorkspacePage::SettingsSecurity => {
                    "C:VerifySandbox(pbf03)  s:CycleSubPolicy(EN/ID)  X:PurgeCache  Q:DetachCLI"
                }
            },
        }
    }
}
