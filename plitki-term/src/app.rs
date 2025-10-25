use anyhow::ensure;
use calloop::{EventLoop, LoopHandle, LoopSignal};
use rustix::termios::{self, Winsize};

use crate::parser::Event;

// use crate::parser::{Event, Parser};

pub struct App {
    loop_handle: LoopHandle<'static, Self>,
    stop_signal: LoopSignal,
    error: Option<anyhow::Error>,

    got_kitty_keyboard_support: bool,

    size: Winsize,
}

impl App {
    pub fn new(event_loop: &EventLoop<'static, Self>) -> anyhow::Result<Self> {
        let size = termios::tcgetwinsize(rustix::stdio::stdout())?;
        eprintln!("{} x {}\r", size.ws_row, size.ws_col);

        Ok(Self {
            loop_handle: event_loop.handle(),
            stop_signal: event_loop.get_signal(),
            error: None,
            got_kitty_keyboard_support: false,
            size,
        })
    }

    pub fn signal_stop(&self) {
        self.stop_signal.stop();
    }

    pub fn stop_on_error(&mut self, result: anyhow::Result<()>) {
        if let Err(err) = result {
            self.error = Some(err);
            self.signal_stop();
        }
    }

    pub fn into_result(self) -> anyhow::Result<()> {
        match self.error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    pub fn resized(&mut self) -> anyhow::Result<()> {
        let size = termios::tcgetwinsize(rustix::stdio::stdout())?;
        if self.size == size {
            return Ok(());
        }
        self.size = size;

        eprintln!("{} x {}\r", size.ws_row, size.ws_col);
        Ok(())
    }

    pub fn event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::Key('q') => {
                self.signal_stop();
            }
            Event::Key(_) => (),
            Event::KittyKeyboardSupported => self.got_kitty_keyboard_support = true,
            Event::PrimaryDeviceAttributes => {
                ensure!(
                    self.got_kitty_keyboard_support,
                    "terminal doesn't support the Kitty keyboard protocol"
                );
            }
        }

        Ok(())
    }
}
