use std::io::{stdout, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{bail, Result};
use crossterm::{
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute,
    terminal::*,
    tty::IsTty,
};
use ratatui::prelude::*;

pub type Term = Terminal<CrosstermBackend<Stdout>>;

static KEYBOARD_ENHANCEMENT_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn init() -> Result<Term> {
    // Check if stdout is a TTY
    if !stdout().is_tty() {
        bail!("lazydeck requires a terminal (TTY) to run. Make sure you're not piping output or running in a non-interactive environment.");
    }

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;

    if matches!(
        crossterm::terminal::supports_keyboard_enhancement(),
        Ok(true)
    ) {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS,
            )
        )?;
        KEYBOARD_ENHANCEMENT_ENABLED.store(true, Ordering::Relaxed);
    }

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;
    // Don't hide cursor globally - let app control it based on mode
    Ok(terminal)
}

pub fn restore() {
    execute!(stdout(), crossterm::cursor::Show).ok();
    if KEYBOARD_ENHANCEMENT_ENABLED.swap(false, Ordering::Relaxed) {
        execute!(stdout(), PopKeyboardEnhancementFlags).ok();
    }
    disable_raw_mode().ok();
    execute!(stdout(), LeaveAlternateScreen).ok();
}
