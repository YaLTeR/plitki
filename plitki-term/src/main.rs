use std::env;
use std::io::{self, Write as _};

use anyhow::{Context as _, ensure};
use calloop::generic::Generic;
use calloop::signals::{Signal, Signals};
use calloop::{EventLoop, Interest, PostAction};
use rustix::io::{Errno, retry_on_intr};
use rustix::termios;

mod app;
mod frame_clock;
mod gameplay;
mod parser;
mod utils;

use app::App;
use utils::*;

use crate::parser::Parser;

fn main() -> anyhow::Result<()> {
    let fd = rustix::stdio::stdout();
    ensure!(termios::isatty(fd));

    if env::var("RUST_BACKTRACE").is_err() {
        unsafe { env::set_var("RUST_BACKTRACE", "1") };
    }

    // Get current term mode.
    let mut ios = termios::tcgetattr(fd)?;

    // Restore mode on normal exit.
    let _guard = RestoreTermMode(ios.clone());
    // Restore mode on panic.
    restore_term_mode_on_panic(ios.clone());

    // Enter raw mode.
    ios.make_raw();
    termios::tcsetattr(fd, termios::OptionalActions::Now, &ios)?;

    {
        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        // Enable alternate screen buffer.
        stdout.write_all(b"\x1B[?1049h")?;
        // Hide cursor.
        stdout.write_all(b"\x1B[?25l")?;
        // Request Kitty keyboard protocol progressive enhancement status.
        stdout.write_all(b"\x1B[?u")?;
        // Request primary device attributes.
        stdout.write_all(b"\x1B[c")?;
        // Push enable Kitty keyboard protocol with:
        // - Disambiguate escape codes;
        // - Report event types;
        // - Report alternate keys;
        // - Report all keys as escape codes.
        stdout.write_all(b"\x1B[>15u")?;
        stdout.flush()?;
    }

    let mut event_loop: EventLoop<'_, App> = EventLoop::try_new()?;
    let handle = event_loop.handle();

    // Listen for signals.
    let signals = Signals::new(&[
        Signal::SIGINT,
        Signal::SIGTERM,
        Signal::SIGHUP,
        Signal::SIGWINCH,
    ])?;
    // Now signalfd is created, so we won't miss SIGWINCH, and can get the term size.

    let mut app = App::new(&event_loop)?;

    handle.insert_source(signals, |event, _, app| {
        if event.signal() == Signal::SIGWINCH {
            let res = app.resized();
            app.stop_on_error(res);
        } else {
            app.signal_stop();
        }
    })?;

    let mut parser = Parser::new();
    handle.insert_source(
        Generic::new(rustix::stdio::stdin(), Interest::READ, calloop::Mode::Level),
        move |_readiness, _fd, app| {
            let stdin = rustix::stdio::stdin();
            let mut buf = [0u8];
            match retry_on_intr(|| rustix::io::read(stdin, &mut buf)) {
                Ok(0) => (),
                Ok(_n) => parser.advance(app, &buf),
                Err(Errno::WOULDBLOCK) => (),
                Err(err) => Err(err)?,
            }
            Ok(PostAction::Continue)
        },
    )?;

    // handle
    //     .insert_source(
    //         calloop::timer::Timer::from_duration(std::time::Duration::from_millis(16)),
    //         |_, _, app| {
    //             app.redraw();
    //             calloop::timer::TimeoutAction::ToDuration(std::time::Duration::from_millis(16))
    //         },
    //     )
    //     .unwrap();

    event_loop.run(None, &mut app, |app| {
        if !app.has_error() {
            let res = app.redraw().context("error redrawing");
            app.stop_on_error(res);
        }
    })?;

    app.into_result()
}
