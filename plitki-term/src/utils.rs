use std::io::{self, Write as _};
use std::panic;

use rustix::termios;

pub struct RestoreTermMode(pub termios::Termios);

impl Drop for RestoreTermMode {
    fn drop(&mut self) {
        let fd = rustix::stdio::stdout();
        if let Err(err) = termios::tcsetattr(fd, termios::OptionalActions::Now, &self.0) {
            eprintln!("error restoring terminal mode: {err:?}");
        };

        // ratatui suggests doing this after leaving raw mode:
        // https://github.com/ratatui/ratatui/blob/v0.29.0/src/terminal/init.rs#L226-L227
        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        // Pop enable Kitty keyboard protocol. Must be done in the alternate screen since main and
        // alternate screens maintain separate stacks.
        let _ = stdout.write_all(b"\x1B[<u");
        // Disable alternate screen buffer.
        let _ = stdout.write_all(b"\x1B[?1049l");
        // Show cursor.
        let _ = stdout.write_all(b"\x1B[?25h");
        let _ = stdout.flush();
    }
}

pub fn restore_term_mode_on_panic(ios: termios::Termios) {
    let hook = panic::take_hook();

    panic::set_hook(Box::new(move |info| {
        // Restore before running the default hook so the backtrace is printed properly.
        drop(RestoreTermMode(ios.clone()));

        hook(info);
    }));
}
