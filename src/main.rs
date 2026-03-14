// This file is part of Lottie.
//
// Copyright (c) 2026  René Coignard <contact@renecoignard.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

mod app;
mod config;
mod export;
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

    if let Some(export_path) = cli.export.clone() {
        let in_path = cli.files.first().cloned();

        if in_path.is_none() || !in_path.as_ref().unwrap().exists() {
            eprintln!("Error: Input file does not exist or was not provided.");
            std::process::exit(1);
        }

        let mut app = App::new(cli.clone());

        let fmt = cli.format.to_lowercase();
        let with_ansi = match fmt.as_str() {
            "ansi" => true,
            "ascii" => {
                app.config.force_ascii = true;
                false
            }
            "plain" => false,
            _ => {
                eprintln!(
                    "Error: Unknown export format '{}'. Expected 'plain', 'ascii', or 'ansi'.",
                    fmt
                );
                std::process::exit(1);
            }
        };

        app.cursor_y = usize::MAX;
        app.update_layout();

        let output = export::export_document(&app.layout, &app.lines, &app.config, with_ansi);

        if export_path.as_os_str() == "-" {
            print!("{}", output);
        } else if let Err(e) = std::fs::write(&export_path, output) {
            eprintln!("Error writing to '{}': {}", export_path.display(), e);
            std::process::exit(1);
        }

        return Ok(());
    }

    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let mut stdout = io::stdout();
        let (_, rows) = crossterm::terminal::size().unwrap_or((0, 24));
        let _ = execute!(stdout, crossterm::cursor::MoveTo(0, rows));

        let _ = disable_raw_mode();
        let _ = execute!(
            stdout,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            LeaveAlternateScreen,
            DisableMouseCapture,
            crossterm::cursor::Show
        );
        println!();
        default_panic(info);
    }));

    let mut app = App::new(cli);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    term.clear()?;

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
                break;
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

    term.clear()?;
    let (_, rows) = crossterm::terminal::size().unwrap_or((0, 24));
    execute!(term.backend_mut(), crossterm::cursor::MoveTo(0, rows))?;

    disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    term.show_cursor()?;
    Ok(())
}
