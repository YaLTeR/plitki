use std::cmp::max;
use std::fs::{self, File};
use std::io::{self, Cursor, Write};
use std::path::Path;

use anyhow::{Context, anyhow, ensure};
use calloop::{EventLoop, LoopHandle, LoopSignal};
use plitki_audio::rodio::Source as _;
use plitki_audio::{AudioEngine, rodio};
use plitki_core::map::Map;
use plitki_core::state::GameState;
use plitki_core::timing::{
    GameTimestamp, GameTimestampDifference, MapTimestampDifference, Timestamp,
};
use rustix::termios::{self, Winsize};

use crate::frame_clock::FrameClock;
use crate::gameplay::Gameplay;
use crate::parser::{Event, Key, Modifier};

pub struct App {
    _loop_handle: LoopHandle<'static, Self>,
    stop_signal: LoopSignal,
    error: Option<anyhow::Error>,

    // Pre-init.
    got_kitty_keyboard_support: bool,

    size: Winsize,
    got_sync: bool,
    need_full_redraw: bool,

    audio: AudioEngine,
    frame_clock: FrameClock,
    gameplay: Option<Gameplay>,
}

impl App {
    pub fn new(event_loop: &EventLoop<'static, Self>) -> anyhow::Result<Self> {
        let size = termios::tcgetwinsize(rustix::stdio::stdout())?;

        Ok(Self {
            _loop_handle: event_loop.handle(),
            stop_signal: event_loop.get_signal(),
            error: None,
            got_kitty_keyboard_support: false,
            size,
            got_sync: false,
            need_full_redraw: true,
            audio: AudioEngine::new(),
            frame_clock: FrameClock::new(),
            gameplay: None,
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

    pub fn has_error(&self) -> bool {
        self.error.is_some()
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
        self.need_full_redraw = true;

        // eprintln!("{} x {}\r", size.ws_row, size.ws_col);

        if let Some(gameplay) = &mut self.gameplay {
            gameplay.resize(size);
        }

        Ok(())
    }

    pub fn event(&mut self, event: Event) -> anyhow::Result<()> {
        // eprintln!("{event:?}\r");

        match event {
            Event::Key { key, mods, release } => {
                if release {
                    self.key_up(key);
                } else {
                    self.key(key, mods);
                }
            }
            Event::KittyKeyboardSupported => self.got_kitty_keyboard_support = true,
            Event::PrimaryDeviceAttributes => {
                ensure!(
                    self.got_kitty_keyboard_support,
                    "terminal doesn't support the Kitty keyboard protocol"
                );

                self.got_sync = true;

                if self.gameplay.is_none() {
                    // This finishes initialization, we can do our first render.
                    let (qua, map_dir) = if let Some(path) = std::env::args_os().nth(1) {
                        let file =
                            File::open(&path).with_context(|| format!("error opening {path:?}"))?;
                        let qua = plitki_map_qua::from_reader(file)
                            .with_context(|| format!("error parsing qua {path:?}"))?;
                        let parent = Path::new(&path).parent().map(Path::to_path_buf);
                        (qua, parent)
                    } else {
                        let qua = include_bytes!("../../plitki-map-qua/tests/data/actual_map.qua");
                        (plitki_map_qua::from_reader(&qua[..])?, None)
                    };
                    let map = Map::from(qua);

                    // Load the audio file.
                    let track = if let Some(name) = &map.audio_file {
                        if let Some(mut dir) = map_dir {
                            dir.push(name);
                            match fs::read(&dir) {
                                Ok(contents) => {
                                    let contents = Cursor::new(contents);
                                    match rodio::Decoder::new(contents) {
                                        Ok(x) => Some(x),
                                        Err(err) => {
                                            // warn!("error decoding audio file: {err:?}");
                                            let _ = err;
                                            None
                                        }
                                    }
                                }
                                Err(_err) => {
                                    // warn!("error reading audio file: {err:?}");
                                    None
                                }
                            }
                        } else {
                            // warn!(".qua file has no parent dir");
                            None
                        }
                    } else {
                        // warn!("map has no audio file set");
                        None
                    };

                    let hit_window = GameTimestampDifference::from_millis(164);
                    let mut game_state = GameState::new(map, hit_window)
                        .map_err(|_| anyhow!("map has invalid objects"))?;
                    game_state.timestamp_converter.global_offset =
                        GameTimestampDifference::from_millis(-120);
                    game_state.timestamp_converter.local_offset =
                        MapTimestampDifference::from_millis(25);
                    let mut gameplay = Gameplay::new(game_state, self.size);
                    let starting_silence = gameplay.starting_silence();
                    gameplay.set_now(GameTimestamp(
                        Timestamp::zero()
                            + (Timestamp::zero() - Timestamp::try_from(starting_silence).unwrap()),
                    ));
                    self.gameplay = Some(gameplay);

                    self.audio.set_volume(0.1);
                    if let Some(track) = track {
                        self.audio.play_track(track.delay(starting_silence));
                    } else {
                        self.audio
                            .play_track(rodio::source::Zero::<f32>::new(2, 44100));
                    }
                }
            }
        }

        Ok(())
    }

    fn now(&self) -> GameTimestamp {
        let Some(gameplay) = &self.gameplay else {
            return GameTimestamp::zero();
        };

        let audio_time_passed = self.audio.track_time();
        let audio_time_passed = Timestamp::try_from(audio_time_passed)
            .unwrap()
            .into_milli_hundredths();
        let starting_silence = Timestamp::try_from(gameplay.starting_silence())
            .unwrap()
            .into_milli_hundredths();
        GameTimestamp(Timestamp::from_milli_hundredths(
            audio_time_passed.saturating_sub(starting_silence),
        ))
    }

    fn key(&mut self, key: Key, mods: Modifier) {
        match key {
            Key::Char('q' | '\x1B') => self.signal_stop(),
            Key::Char('c') if mods == Modifier::Ctrl => self.signal_stop(),
            _ => {
                let now = self.now();
                if let Some(gameplay) = &mut self.gameplay {
                    gameplay.set_now(now);
                    gameplay.key(key, mods);
                }
            }
        }
    }

    fn key_up(&mut self, key: Key) {
        let now = self.now();
        if let Some(gameplay) = &mut self.gameplay {
            gameplay.set_now(now);
            gameplay.key_up(key);
        }
    }

    pub fn redraw(&mut self) -> io::Result<()> {
        if !self.got_sync {
            return Ok(());
        }

        self.frame_clock.frame();

        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        // Start synchronized update.
        stdout.write_all(b"\x1B[?2026h")?;

        if self.need_full_redraw {
            // Erase all.
            stdout.write_all(b"\x1B[2J")?;
            self.draw_binds(&mut stdout)?;
        }

        let now = self.now();
        if let Some(gameplay) = &mut self.gameplay {
            if self.need_full_redraw {
                gameplay.draw_borders(&mut stdout)?;
            }

            gameplay.set_now(now);
            gameplay.draw_playfield(&mut stdout)?;
        }

        self.draw_fps(&mut stdout)?;

        // End synchronized update.
        stdout.write_all(b"\x1B[?2026l")?;

        self.request_sync(&mut stdout)?;
        stdout.flush()?;

        self.need_full_redraw = false;

        Ok(())
    }

    fn draw_binds(&self, stdout: &mut io::StdoutLock) -> io::Result<()> {
        let y = max(6, self.size.ws_row) - 7;
        write!(stdout, "\x1B[{y};0H")?;

        write!(stdout, "▁▂▃▄▅▆▇█\x1B[E")?;
        write!(stdout, "▔🮂🮃▀🮄🮅🮆█\x1B[E")?;
        write!(stdout, "▁🭻🭺🭹🭸🭷🭶▔\x1B[E")?;
        write!(stdout, "q quit\x1B[E")?;
        write!(stdout, "w upscroll\x1B[E")?;
        write!(stdout, "F3/F4      speed ±5\x1B[E")?;
        write!(stdout, "Ctrl+F3/F4 speed ±1\x1B[E")?;
        write!(stdout, "-/+ offset ±5 ms\x1B[E")?;

        Ok(())
    }

    fn draw_fps(&self, stdout: &mut io::StdoutLock) -> io::Result<()> {
        if let Some(fps) = self.frame_clock.fps() {
            write!(stdout, "\x1B[HFPS: {fps:>5.0}")?;
        }

        Ok(())
    }

    // In order to avoid filling up the terminal buffer with several rendered
    // frames faster than it can read them, we request the primary device
    // attributes at the end of each frame, and don't render new frames until we
    // get a response.
    fn request_sync(&mut self, stdout: &mut io::StdoutLock) -> io::Result<()> {
        stdout.write_all(b"\x1B[c")?;
        self.got_sync = false;
        Ok(())
    }
}
