use std::{
    cmp::{max, min},
    fs::File,
    io::{self, Write},
    time::Instant,
};

use anyhow::{Context, anyhow, ensure};
use calloop::{EventLoop, LoopHandle, LoopSignal};
use plitki_core::{
    map::Map,
    object::Object,
    scroll::{Position, ScreenPositionDifference, ScrollSpeed},
    state::GameState,
    timing::{GameTimestamp, GameTimestampDifference, MapTimestamp, Timestamp},
};
use rustix::termios::{self, Winsize};

use crate::{
    frame_clock::FrameClock,
    parser::{Event, Modifier},
};

pub struct App {
    _loop_handle: LoopHandle<'static, Self>,
    stop_signal: LoopSignal,
    error: Option<anyhow::Error>,

    got_kitty_keyboard_support: bool,

    size: Winsize,

    scroll_speed: ScrollSpeed,
    downscroll: bool,
    game_state: Option<GameState>,
    got_sync: bool,
    frame_clock: FrameClock,
    buffer: Vec<i8>,
    start: Option<Instant>,
}

impl App {
    pub fn new(event_loop: &EventLoop<'static, Self>) -> anyhow::Result<Self> {
        let size = termios::tcgetwinsize(rustix::stdio::stdout())?;
        eprintln!("{} x {}\r", size.ws_row, size.ws_col);
        eprintln!("▁▂▃▄▅▆▇█\r");
        eprintln!("▔🮂🮃▀🮄🮅🮆█\r");
        eprintln!(" 🭻🭺🭹🭸🭷🭶▔\r");
        eprintln!("▂🭻▃🭺▄🭹▅🭸▆🭷▇🭶█▔\r");

        Ok(Self {
            _loop_handle: event_loop.handle(),
            stop_signal: event_loop.get_signal(),
            error: None,
            got_kitty_keyboard_support: false,
            size,
            scroll_speed: ScrollSpeed(10),
            downscroll: true,
            game_state: None,
            got_sync: false,
            frame_clock: FrameClock::new(),
            buffer: Vec::new(),
            start: None,
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

        // eprintln!("{} x {}\r", size.ws_row, size.ws_col);

        // Erase all.
        print!("\x1B[2J");

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

                if self.game_state.is_none() {
                    // This finishes initialization, we can do our first render.
                    let qua = if let Some(path) = std::env::args_os().nth(1) {
                        let file =
                            File::open(&path).with_context(|| format!("error opening {path:?}"))?;
                        plitki_map_qua::from_reader(file)
                            .with_context(|| format!("error parsing qua {path:?}"))?
                    } else {
                        let qua = include_bytes!("../../plitki-map-qua/tests/data/actual_map.qua");
                        plitki_map_qua::from_reader(&qua[..])?
                    };
                    let map = Map::from(qua);
                    let hit_window = GameTimestampDifference::from_millis(164);
                    let game_state = GameState::new(map, hit_window)
                        .map_err(|_| anyhow!("map has invalid objects"))?;
                    self.game_state = Some(game_state);

                    self.start = Some(Instant::now());
                }
            }
        }

        Ok(())
    }

    fn key(&mut self, key: char, mods: Modifier) {
        match key {
            'q' | '\x1B' => self.signal_stop(),
            'c' if mods == Modifier::Ctrl => self.signal_stop(),
            '=' if mods.contains(Modifier::Shift) => {
                let c = if mods.contains(Modifier::Ctrl) { 1 } else { 5 };
                self.scroll_speed.0 = self.scroll_speed.0.saturating_add(c);
            }
            '-' => {
                let c = if mods.contains(Modifier::Ctrl) { 1 } else { 5 };
                self.scroll_speed.0 = self.scroll_speed.0.saturating_sub(c);
            }
            'w' => self.downscroll = !self.downscroll,
            _ => (),
        }
    }

    fn key_up(&mut self, _key: char) {}

    pub fn redraw(&mut self) -> io::Result<()> {
        if !self.got_sync {
            return Ok(());
        }

        self.frame_clock.frame();

        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        // Start synchronized update.
        stdout.write_all(b"\x1B[?2026h")?;

        self.draw_borders(&mut stdout)?;
        self.draw_playfield(&mut stdout)?;

        // Print FPS.
        if let Some(fps) = self.frame_clock.fps() {
            write!(stdout, "\x1B[HFPS: {fps:>5.0}")?;
        }

        // End synchronized update.
        stdout.write_all(b"\x1B[?2026l")?;

        self.request_sync(&mut stdout)?;
        stdout.flush()?;

        Ok(())
    }

    fn draw_borders(&mut self, stdout: &mut io::StdoutLock) -> io::Result<()> {
        let game_state = self.game_state.as_mut().unwrap();
        let lane_width = 8;

        let lane_count = game_state.lane_count();
        let Ok(playfield_width) = u16::try_from(lane_count * lane_width) else {
            return Ok(());
        };
        if self.size.ws_col < playfield_width + 2 {
            return Ok(());
        }
        let x = (self.size.ws_col - playfield_width) / 2;

        stdout.write_all(b"\x1B[90m")?;

        for y in 1..=self.size.ws_row {
            write!(stdout, "\x1B[{y};{x}H▐\x1B[{playfield_width}C▌")?;
        }

        // Default color.
        stdout.write_all(b"\x1B[39m")?;

        Ok(())
    }

    fn draw_playfield(&mut self, stdout: &mut io::StdoutLock) -> io::Result<()> {
        self.render();

        let game_state = self.game_state.as_mut().unwrap();

        let lane_width = 8;

        let lane_count = game_state.lane_count();
        let Ok(playfield_width) = u16::try_from(lane_count * lane_width) else {
            return Ok(());
        };
        if self.size.ws_col < playfield_width {
            return Ok(());
        }
        let x = (self.size.ws_col - playfield_width) / 2 + 1;

        let mut last_color = 39;

        let mut iter = self.buffer.chunks(lane_count);
        let mut rev;
        let iter: &mut dyn Iterator<Item = &[i8]> = if self.downscroll {
            rev = iter.rev();
            &mut rev
        } else {
            &mut iter
        };

        for (i, row) in iter.enumerate() {
            write!(stdout, "\x1B[{y};{x}H", y = i + 1)?;

            for (lane, fill) in row.iter().enumerate() {
                let fill = if self.downscroll { -*fill } else { *fill };
                let c = match fill {
                    -8 => "█",
                    -7 => "▇",
                    -6 => "▆",
                    -5 => "▅",
                    -4 => "▄",
                    -3 => "▃",
                    -2 => "▂",
                    -1 => "▁",
                    1 => "▔",
                    2 => "🮂",
                    3 => "🮃",
                    4 => "▀",
                    5 => "🮄",
                    6 => "🮅",
                    7 => "🮆",
                    8 => "█",
                    _ => " ",
                };

                if c != " " {
                    let color = lane_color(lane_count, lane);
                    if color != last_color {
                        write!(stdout, "\x1B[{color}m")?;
                        last_color = color;
                    }
                }

                for _ in 0..lane_width {
                    stdout.write_all(c.as_bytes())?;
                }
            }
        }

        // Restore color.
        if last_color != 39 {
            stdout.write_all(b"\x1B[39m")?;
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

    fn render(&mut self) {
        let game_state = self.game_state.as_mut().unwrap();
        let lane_count = game_state.lane_count();
        self.buffer
            .resize(lane_count * usize::from(self.size.ws_row), 0);
        self.buffer.fill(0);

        const SPD_PER_SUBROW: i64 = 4_000_000;
        let spd_to_subrow = |spd: ScreenPositionDifference| {
            spd.0.checked_add(SPD_PER_SUBROW - 1).unwrap() / SPD_PER_SUBROW
        };
        // Compute positions against zero to avoid flicker.
        let pos_to_subrow =
            |pos: Position| spd_to_subrow((pos - Position::zero()) * self.scroll_speed);

        let elapsed = self.start.unwrap().elapsed();
        let elapsed = GameTimestamp(Timestamp::try_from(elapsed).unwrap());

        while let Some(_event) = game_state.update(elapsed) {}

        let elapsed = game_state.timestamp_converter.game_to_map(elapsed);
        let first_ts = game_state.first_timestamp().unwrap() + (elapsed - MapTimestamp::zero());
        let first_pos = first_ts.no_scroll_speed_change_position();
        let first_pos_subrow = pos_to_subrow(first_pos);
        let num_rows = i64::from(self.size.ws_row);

        for (i, lane) in game_state.immutable.map.lanes.iter().enumerate() {
            for obj in lane.objects.iter().rev() {
                let (start, end) = match obj {
                    Object::Regular { timestamp } => {
                        let pos = timestamp.no_scroll_speed_change_position();
                        let subrow = pos_to_subrow(pos);
                        (subrow, subrow + 8)
                    }
                    Object::LongNote { start, end } => {
                        let start = start.no_scroll_speed_change_position();
                        let end = end.no_scroll_speed_change_position();
                        (pos_to_subrow(start), pos_to_subrow(end))
                    }
                };

                let start = start - first_pos_subrow;
                let end = end - first_pos_subrow;

                let start_row = start.div_euclid(8);
                let end_row = end.div_euclid(8);

                if end_row < 0 || start_row >= num_rows {
                    continue;
                }

                if start_row >= 0 {
                    let start_subrow = start.rem_euclid(8) as i8;
                    self.buffer[start_row as usize * lane_count + i] = start_subrow - 8;
                }

                for row in max(0, start_row + 1)..min(end_row, num_rows) {
                    self.buffer[row as usize * lane_count + i] = -8;
                }

                let end_subrow = end.rem_euclid(8) as i8;
                if end_subrow > 0 && end_row < num_rows {
                    self.buffer[end_row as usize * lane_count + i] = end_subrow;
                }
            }
        }
    }
}

fn lane_color(lane_count: usize, lane: usize) -> u8 {
    let alt = match (lane * 2 + 1).cmp(&lane_count) {
        std::cmp::Ordering::Less => lane % 2 == 1,
        std::cmp::Ordering::Equal => return 33,
        std::cmp::Ordering::Greater => (lane_count - lane - 1) % 2 == 1,
    };
    if alt { 36 } else { 39 }
}
