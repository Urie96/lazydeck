use std::io::{stdout, Stdout};
use anyhow::{bail, Result};
use crossterm::{execute, terminal::*, tty::IsTty};
use ratatui::prelude::*;

pub type Term = Terminal<CrosstermBackend<Stdout>>;

pub fn init() -> Result<Term> {
    // Check if stdout is a TTY
    if !stdout().is_tty() {
        bail!("lazydeck requires a terminal (TTY) to run. Make sure you're not piping output or running in a non-interactive environment.");
    }

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;
    // Don't hide cursor globally - let app control it based on mode
    Ok(terminal)
}

pub fn restore() {
    execute!(stdout(), crossterm::cursor::Show).ok();
    disable_raw_mode().ok();
    execute!(stdout(), LeaveAlternateScreen).ok();
}
