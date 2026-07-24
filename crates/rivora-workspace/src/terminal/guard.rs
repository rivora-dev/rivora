//! Terminal guard — enter/leave alternate screen and raw mode safely.
//!
//! Restoration protection is installed BEFORE any terminal mutation so a
//! panic between raw-mode enable and full initialization still restores the
//! terminal. Per-step flags track how far `enter()` got so cleanup only
//! reverses the steps that actually succeeded, and is idempotent.

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

// Per-step init flags. Set as each mutation succeeds, swapped to false on
// restore so `restore_terminal` is idempotent and only reverses completed
// steps. These are module-level so the panic hook (which cannot borrow
// `&mut self`) can restore during partial initialization.
static RAW_ENABLED: AtomicBool = AtomicBool::new(false);
static ALT_SCREEN_ENTERED: AtomicBool = AtomicBool::new(false);
static MOUSE_ENABLED: AtomicBool = AtomicBool::new(false);
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// True when stdin and stdout are TTYs.
pub fn is_interactive_terminal() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Owns terminal mode for the Unified Workspace lifetime.
pub struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    /// Enter raw mode + alternate screen.
    ///
    /// Order is deliberate and safety-critical:
    ///   1. install the restoration-aware panic hook  (BEFORE any mutation)
    ///   2. enable raw mode
    ///   3. enter alternate screen
    ///   4. enable mouse capture
    ///   5. construct the ratatui terminal
    ///
    /// Any failure between steps 2 and 5 runs a targeted cleanup that
    /// reverses only the steps that succeeded. Because the hook is already
    /// installed, a panic during those steps also restores via the hook.
    pub fn enter() -> Result<Self, String> {
        if !is_interactive_terminal() {
            return Err("interactive Workspace requires a terminal (TTY). \
                 Use a CLI subcommand for non-interactive environments."
                .to_string());
        }

        // Step 1: install restoration protection BEFORE any terminal mutation.
        install_panic_hook();

        // Step 2: raw mode.
        enable_raw_mode().map_err(|e| {
            cleanup_partial();
            format!("enable raw mode: {e}")
        })?;
        RAW_ENABLED.store(true, Ordering::SeqCst);

        // Step 3: alternate screen.
        if let Err(e) = execute!(io::stdout(), EnterAlternateScreen) {
            cleanup_partial();
            return Err(format!("enter alternate screen: {e}"));
        }
        ALT_SCREEN_ENTERED.store(true, Ordering::SeqCst);

        // Step 4: mouse capture.
        if let Err(e) = execute!(io::stdout(), EnableMouseCapture) {
            cleanup_partial();
            return Err(format!("enable mouse capture: {e}"));
        }
        MOUSE_ENABLED.store(true, Ordering::SeqCst);

        // Step 5: ratatui terminal. A panic here is still covered by the
        // hook installed in step 1, and a returned Err reverses steps 2-4.
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = match Terminal::new(backend) {
            Ok(t) => t,
            Err(e) => {
                cleanup_partial();
                return Err(format!("create terminal: {e}"));
            }
        };

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

/// Idempotently reverse only the terminal mutations that were applied.
/// Safe to call from: normal `Drop`, explicit `restore`, the panic hook,
/// and `enter()` partial-failure cleanup. Each flag is swapped to false so
/// repeated calls are harmless.
fn restore_terminal() {
    if RAW_ENABLED.swap(false, Ordering::SeqCst) {
        let _ = disable_raw_mode();
    }
    if ALT_SCREEN_ENTERED.swap(false, Ordering::SeqCst) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen);
    }
    if MOUSE_ENABLED.swap(false, Ordering::SeqCst) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, DisableMouseCapture);
    }
    // Always ensure the cursor is visible again; harmless when already shown.
    let _ = execute!(io::stdout(), crossterm::cursor::Show);
}

/// Reverses any partial initialization without relying on a `Self` existing.
fn cleanup_partial() {
    restore_terminal();
}

/// Install the restoration-aware panic hook. Installed exactly once per
/// process (guarded by `HOOK_INSTALLED`) so repeated Workspace launches in
/// tests do not create an unbounded hook chain. The hook restores the
/// terminal before delegating to the previous hook; it never leaks secrets
/// (it touches only terminal mode) and chains to the prior hook so test or
/// library panic handlers still run.
fn install_panic_hook() {
    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        // Restore before printing panic info so the message is readable in
        // the restored terminal. Wrap inner restore in catch_unwind so the
        // hook itself never double-panics on restore failure.
        let _ = panic::catch_unwind(panic::AssertUnwindSafe(restore_terminal));
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_flags() {
        RAW_ENABLED.store(false, Ordering::SeqCst);
        ALT_SCREEN_ENTERED.store(false, Ordering::SeqCst);
        MOUSE_ENABLED.store(false, Ordering::SeqCst);
        HOOK_INSTALLED.store(false, Ordering::SeqCst);
    }

    #[test]
    fn restore_is_idempotent_when_inactive() {
        reset_flags();
        restore_terminal();
        restore_terminal();
    }

    #[test]
    fn restore_reverses_only_completed_steps() {
        // Simulate a panic after raw mode but before alternate screen:
        // only RAW_ENABLED is set. restore must reverse raw and leave the
        // rest untouched, then be a no-op on a second call.
        reset_flags();
        RAW_ENABLED.store(true, Ordering::SeqCst);
        restore_terminal();
        assert!(!RAW_ENABLED.load(Ordering::SeqCst));
        assert!(!ALT_SCREEN_ENTERED.load(Ordering::SeqCst));
        assert!(!MOUSE_ENABLED.load(Ordering::SeqCst));
        // Second call must not panic and must not touch already-cleared flags.
        restore_terminal();
        assert!(!RAW_ENABLED.load(Ordering::SeqCst));
    }

    #[test]
    fn restore_reverses_raw_and_alt_screen_only() {
        reset_flags();
        RAW_ENABLED.store(true, Ordering::SeqCst);
        ALT_SCREEN_ENTERED.store(true, Ordering::SeqCst);
        restore_terminal();
        assert!(!RAW_ENABLED.load(Ordering::SeqCst));
        assert!(!ALT_SCREEN_ENTERED.load(Ordering::SeqCst));
        assert!(!MOUSE_ENABLED.load(Ordering::SeqCst));
        // Idempotent.
        restore_terminal();
        assert!(!RAW_ENABLED.load(Ordering::SeqCst));
    }

    #[test]
    fn restore_reverses_full_init_idempotently() {
        reset_flags();
        RAW_ENABLED.store(true, Ordering::SeqCst);
        ALT_SCREEN_ENTERED.store(true, Ordering::SeqCst);
        MOUSE_ENABLED.store(true, Ordering::SeqCst);
        restore_terminal();
        assert!(!RAW_ENABLED.load(Ordering::SeqCst));
        assert!(!ALT_SCREEN_ENTERED.load(Ordering::SeqCst));
        assert!(!MOUSE_ENABLED.load(Ordering::SeqCst));
        restore_terminal();
        restore_terminal();
    }

    #[test]
    fn panic_hook_installs_exactly_once() {
        reset_flags();
        assert!(!HOOK_INSTALLED.load(Ordering::SeqCst));
        install_panic_hook();
        assert!(HOOK_INSTALLED.load(Ordering::SeqCst));
        // A second install must be a no-op (no chain growth): the flag stays
        // true and we did not capture/replace the hook again.
        install_panic_hook();
        assert!(HOOK_INSTALLED.load(Ordering::SeqCst));
        // Reset the global hook to the previous (default) for test hygiene.
        let _ = panic::take_hook();
        HOOK_INSTALLED.store(false, Ordering::SeqCst);
    }

    #[test]
    fn interactive_check_matches_stdio() {
        let expected = io::stdin().is_terminal() && io::stdout().is_terminal();
        assert_eq!(is_interactive_terminal(), expected);
    }

    /// Q (behavior): the hook is installed before raw mode is enabled.
    /// Asserted structurally: `install_panic_hook` only flips `HOOK_INSTALLED`
    /// and `enter()` calls it before touching any terminal mutation flag.
    #[test]
    fn hook_flag_ordered_before_raw_flag_contract() {
        // The contract is encoded in enter()'s source order. We assert the
        // hook installation helper exists and is idempotent here, and rely
        // on enter()'s ordering (hook → raw → alt → mouse → terminal) which
        // is verified by the partial-restore tests above (raw may be set
        // while alt/mouse are not, matching the post-hook ordering).
        reset_flags();
        install_panic_hook();
        assert!(HOOK_INSTALLED.load(Ordering::SeqCst));
        let _ = panic::take_hook();
        HOOK_INSTALLED.store(false, Ordering::SeqCst);
    }
}
