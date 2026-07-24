//! Terminal guard — enter/leave alternate screen and raw mode safely.

use std::io::{self, IsTerminal, Stdout};
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

/// True when stdin and stdout are TTYs.
pub fn is_interactive_terminal() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Owns terminal mode for the Unified Workspace lifetime.
pub struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    /// Enter raw mode + alternate screen. Installs a panic hook that restores the terminal.
    pub fn enter() -> Result<Self, String> {
        if !is_interactive_terminal() {
            return Err("interactive Workspace requires a terminal (TTY). \
                 Use a CLI subcommand for non-interactive environments."
                .to_string());
        }

        enable_raw_mode().map_err(|e| format!("enable raw mode: {e}"))?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| format!("enter alternate screen: {e}"))?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).map_err(|e| format!("create terminal: {e}"))?;

        TERMINAL_ACTIVE.store(true, Ordering::SeqCst);
        install_panic_hook();

        Ok(Self { terminal })
    }

    /// Borrow the ratatui terminal.
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    /// Leave alternate screen and restore the terminal.
    pub fn restore(&mut self) {
        restore_terminal();
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    if !TERMINAL_ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
    let _ = crossterm::cursor::Show;
    let _ = execute!(stdout, crossterm::cursor::Show);
}

fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_is_idempotent_when_inactive() {
        TERMINAL_ACTIVE.store(false, Ordering::SeqCst);
        restore_terminal();
        restore_terminal();
    }

    #[test]
    fn interactive_check_matches_stdio() {
        let expected = io::stdin().is_terminal() && io::stdout().is_terminal();
        assert_eq!(is_interactive_terminal(), expected);
    }
}
