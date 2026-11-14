use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use mvd::app::message::Message;
use mvd::app::model::{AppModel, ExitTransition};
use mvd::app::update::update;
use mvd::cli::progress_runner::CliProgressHandoff;
use mvd::config::xdg::XdgPaths;
use mvd::config::AppConfig;
use mvd::engine::worker::AsyncWorkerCoordinator;
use mvd::storage::state_store::QueueStateStore;
use mvd::ui::view::render;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Write};
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let xdg = XdgPaths::resolve();
    let config = AppConfig::default();
    let recovered_jobs = QueueStateStore::load_snapshot(&xdg.state_queue_json).unwrap_or_default();
    let mut model = AppModel::new(config.clone(), recovered_jobs);

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let coordinator = AsyncWorkerCoordinator::new(tx, xdg.clone(), config);
    coordinator.refresh_initial_storage_telemetry();

    // If user passed a URL on the command line (`mvd <url>`), auto-extract immediately
    if let Some(cli_url) = std::env::args().nth(1) {
        if !cli_url.starts_with('-') {
            let cmds = update(&mut model, Message::TriggerUrlExtraction(cli_url));
            coordinator.dispatch_all(cmds, &model.queue_jobs);
        }
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let loop_res = run_tui_loop(&mut terminal, &mut model, &coordinator, &mut rx);

    // Purge Kitty graphics placements and restore terminal state cleanly
    let _ = terminal.backend_mut().write_all(b"\x1b_Ga=d\x1b\\");
    let _ = terminal.backend_mut().flush();
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    loop_res?;

    // If user exited TUI via 'Q' or 'Ctrl+C' while downloads were active, hand off to CLI mode!
    if model.exit_transition == ExitTransition::DetachToCliProgress {
        CliProgressHandoff::print_detach_banner(&model.queue_jobs)?;
        CliProgressHandoff::persist_resumable_state_on_cli_kill(
            &xdg.state_queue_json,
            &mut model.queue_jobs,
        )?;
    }

    Ok(())
}

fn run_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    model: &mut AppModel,
    coordinator: &AsyncWorkerCoordinator,
    rx: &mut mpsc::UnboundedReceiver<Message>,
) -> io::Result<()> {
    loop {
        // Drain all completed background worker messages (yt-dlp JSON, Kitty thumbnail, LRCLIB, progress)
        while let Ok(bg_msg) = rx.try_recv() {
            let cmds = update(model, bg_msg);
            coordinator.dispatch_all(cmds, &model.queue_jobs);
        }

        // Dirty-flag rendering: draw to Kitty ONLY when model.dirty is true
        if model.dirty {
            terminal.draw(|frame| render(frame, model))?;
            model.dirty = false;
        }

        if model.exit_transition != ExitTransition::StayInTui {
            break;
        }

        if event::poll(Duration::from_millis(40))? {
            match event::read()? {
                Event::Key(key) => {
                    let cmds = update(model, Message::KeyPressed(key));
                    coordinator.dispatch_all(cmds, &model.queue_jobs);
                }
                Event::Resize(w, h) => {
                    let cmds = update(model, Message::TerminalResized(w, h));
                    coordinator.dispatch_all(cmds, &model.queue_jobs);
                }
                _ => {}
            }
        }
    }
    Ok(())
}
