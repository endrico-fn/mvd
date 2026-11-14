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
use mvd::storage::state_store::QueueStateStore;
use mvd::ui::view::render;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::Duration;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let xdg = XdgPaths::resolve();
    let config = AppConfig::default();
    let recovered_jobs = QueueStateStore::load_snapshot(&xdg.state_queue_json).unwrap_or_default();
    let mut model = AppModel::new(config, recovered_jobs);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let loop_res = run_tui_loop(&mut terminal, &mut model);

    // Restore terminal state cleanly (including clearing Kitty graphics placements)
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
) -> io::Result<()> {
    loop {
        // Dirty-flag rendering: draw to Kitty ONLY when model.dirty is true
        if model.dirty {
            terminal.draw(|frame| render(frame, model))?;
            model.dirty = false;
        }

        if model.exit_transition != ExitTransition::StayInTui {
            break;
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    let _ = update(model, Message::KeyPressed(key));
                }
                Event::Resize(w, h) => {
                    let _ = update(model, Message::TerminalResized(w, h));
                }
                _ => {}
            }
        }
    }
    Ok(())
}
