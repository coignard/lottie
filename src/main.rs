mod app;
mod config;
mod formatting;
mod layout;
mod parser;
mod types;

use app::{App, draw};
use clap::Parser;
use config::Cli;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, panic, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        default_panic(info);
    }));

    let path = cli.file.clone();
    let mut app = App::new(path, cli);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    loop {
        term.draw(|f| draw(f, &mut app))?;

        let ev = event::read()?;
        let mut update_target_x = false;
        let mut text_changed = false;
        let mut cursor_moved = false;

        if app.handle_event(
            ev,
            &mut update_target_x,
            &mut text_changed,
            &mut cursor_moved,
        )? {
            break;
        }

        while event::poll(Duration::from_millis(0))? {
            let next_ev = event::read()?;
            if app.handle_event(
                next_ev,
                &mut update_target_x,
                &mut text_changed,
                &mut cursor_moved,
            )? {
                disable_raw_mode()?;
                execute!(
                    term.backend_mut(),
                    LeaveAlternateScreen,
                    DisableMouseCapture
                )?;
                term.show_cursor()?;
                return Ok(());
            }
        }

        if text_changed {
            app.parse_document();
        }

        if text_changed || cursor_moved {
            app.update_autocomplete();
            app.update_layout();
        }

        if update_target_x {
            app.target_visual_x = app.current_visual_x();
        }
    }

    disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    term.show_cursor()?;
    Ok(())
}
