use std::cmp::{Ordering, max, min};
use std::io::{self, Write as _};
use std::iter::zip;
use std::time::Duration;

use plitki_core::scroll::{Position, ScreenPositionDifference, ScrollSpeed};
use plitki_core::state::{GameState, ObjectCache};
use plitki_core::timing::{GameTimestamp, GameTimestampDifference, MapTimestampDifference};
use rustix::termios::Winsize;

use crate::parser::{Key, Modifier};

pub struct Gameplay {
    pub state: GameState,
    pub scroll_speed: ScrollSpeed,
    pub downscroll: bool,
    pub now: GameTimestamp,

    pub is_lane_pressed: Vec<bool>,

    pub size: Winsize,
    buffer: Vec<(i8, Color)>,
}

#[derive(Clone, Copy)]
enum Color {
    Normal,
    Missed,
    TimingLine,
    JudgementLine,
}

impl Gameplay {
    pub fn new(state: GameState, size: Winsize) -> Self {
        let lane_count = state.lane_count();

        Self {
            state,
            scroll_speed: ScrollSpeed(32),
            downscroll: true,
            now: GameTimestamp::zero(),
            is_lane_pressed: vec![false; lane_count],
            size,
            buffer: Vec::new(),
        }
    }

    pub fn set_now(&mut self, now: GameTimestamp) {
        self.now = now;
        self.update(now);
    }

    pub fn key(&mut self, key: Key, mods: Modifier) {
        match key {
            Key::F3 => {
                let c = if mods.contains(Modifier::Ctrl) { 1 } else { 5 };
                self.scroll_speed.0 = self.scroll_speed.0.saturating_sub(c);
            }
            Key::F4 => {
                let c = if mods.contains(Modifier::Ctrl) { 1 } else { 5 };
                self.scroll_speed.0 = self.scroll_speed.0.saturating_add(c);
            }
            Key::Char('w') => self.downscroll = !self.downscroll,
            Key::Char('-') => {
                let diff = MapTimestampDifference::from_millis(5);
                self.state.timestamp_converter.local_offset = self
                    .state
                    .timestamp_converter
                    .local_offset
                    .saturating_sub(diff);
                self.state.update(self.now);
            }
            Key::Char('=') | Key::Char('+') => {
                let diff = MapTimestampDifference::from_millis(5);
                self.state.timestamp_converter.local_offset = self
                    .state
                    .timestamp_converter
                    .local_offset
                    .saturating_add(diff);
                self.state.update(self.now);
            }
            Key::Char(key) => {
                if let Some(lane) = lane_for_key(self.state.lane_count(), key)
                    && !self.is_lane_pressed[lane]
                {
                    self.is_lane_pressed[lane] = true;
                    if let Some(event) = self.state.key_press(lane, self.now) {
                        self.event(event);
                    }
                }
            }
        }
    }

    pub fn key_up(&mut self, key: Key) {
        if let Key::Char(key) = key
            && let Some(lane) = lane_for_key(self.state.lane_count(), key)
            && self.is_lane_pressed[lane]
        {
            self.is_lane_pressed[lane] = false;
            if let Some(event) = self.state.key_release(lane, self.now) {
                self.event(event);
            }
        }
    }

    pub fn resize(&mut self, size: Winsize) {
        self.size = size;
    }

    fn lane_width(&self) -> i32 {
        self.note_height() as i32 * 8 / 10
    }

    fn judgement_y(&self) -> i64 {
        self.note_height() * 4 / 8
    }

    fn note_height(&self) -> i64 {
        let v = f32::from(max(7, min(self.size.ws_col / 8, self.size.ws_row / 4)));
        if self.state.lane_count() < 6 {
            (v * 1.2).round() as i64
        } else {
            v as i64
        }
    }

    fn playfield_width(&self) -> i32 {
        self.state.lane_count() as i32 * self.lane_width()
    }

    fn playfield_x(&self) -> i32 {
        // Terminal coordinates are 1-based.
        (i32::from(self.size.ws_col) - self.playfield_width()) / 2 + 1
    }

    pub fn starting_silence(&self) -> Duration {
        let Some(first) = self.state.first_timestamp() else {
            return Duration::ZERO;
        };

        let music_start = GameTimestamp::zero();

        let first = self.state.timestamp_converter.map_to_game(first);
        let start_at = first - GameTimestampDifference::from_millis(1000);
        let start_at = min(music_start, start_at);
        Duration::try_from((music_start - start_at).0).unwrap()
    }

    fn event(&mut self, _event: plitki_core::state::Event) {}

    fn update(&mut self, timestamp: GameTimestamp) {
        while let Some(event) = self.state.update(timestamp) {
            self.event(event);
        }
    }

    fn render(&mut self) {
        let lane_count = self.state.lane_count();
        self.buffer.resize(
            lane_count * usize::from(self.size.ws_row),
            (0, Color::Normal),
        );
        self.buffer.fill((0, Color::Normal));

        let spd_per_subrow = 8_000_000i64 * 7 / self.note_height();
        let spd_to_subrow = |spd: ScreenPositionDifference| {
            spd.0.checked_add(spd_per_subrow - 1).unwrap() / spd_per_subrow
        };
        // Compute positions against zero to avoid flicker.
        let pos_to_subrow =
            |pos: Position| spd_to_subrow((pos - Position::zero()) * self.scroll_speed);

        let judgement_y = self.judgement_y();

        let now = self.state.timestamp_converter.game_to_map(self.now);
        let now_pos = self.state.position_at_time(now);
        let now_pos_subrow = pos_to_subrow(now_pos);
        let first_pos_subrow = now_pos_subrow - judgement_y * 8;
        let num_rows = i64::from(self.size.ws_row);

        for line in &self.state.immutable.timing_lines {
            let pos = pos_to_subrow(line.position) - first_pos_subrow;
            let row = pos.div_euclid(8);
            if row >= 0 && row < num_rows {
                for i in 0..lane_count {
                    let subrow = pos.rem_euclid(8) as i8;
                    self.buffer[row as usize * lane_count + i] = (subrow - 18, Color::TimingLine);
                }
            }
        }

        let iter = zip(
            zip(
                &self.state.immutable.map.lanes,
                &self.state.immutable.lane_caches,
            ),
            &self.state.lane_states,
        );
        for (i, ((lane, cache), state)) in iter.enumerate() {
            let iter = zip(
                zip(&lane.objects, &cache.object_caches),
                &state.object_states,
            );
            for ((_obj, cache), state) in iter.rev() {
                if state.is_hit() {
                    continue;
                };

                let start = self.state.object_start_position(*state, *cache, now_pos);
                let start = pos_to_subrow(start);

                let end = match cache {
                    ObjectCache::Regular(_) => start + self.note_height(),
                    ObjectCache::LongNote(cache) => pos_to_subrow(cache.end_position),
                };

                let start = start - first_pos_subrow;
                let end = end - first_pos_subrow;

                let start_row = start.div_euclid(8);
                let end_row = end.div_euclid(8);

                if end_row < 0 || start_row >= num_rows {
                    continue;
                }

                let color = if state.is_missed() {
                    Color::Missed
                } else {
                    Color::Normal
                };

                if start_row >= 0 {
                    let start_subrow = start.rem_euclid(8) as i8;
                    self.buffer[start_row as usize * lane_count + i] = (start_subrow - 8, color);
                }

                for row in max(0, start_row + 1)..min(end_row, num_rows) {
                    self.buffer[row as usize * lane_count + i] = (-8, color);
                }

                let end_subrow = end.rem_euclid(8) as i8;
                if end_subrow > 0 && end_row < num_rows {
                    self.buffer[end_row as usize * lane_count + i] = (end_subrow, color);
                }
            }
        }

        if judgement_y < num_rows {
            for i in 0..lane_count {
                self.buffer[judgement_y as usize * lane_count + i] = (-2, Color::JudgementLine);
            }
        }
    }

    pub fn draw_borders(&mut self, stdout: &mut io::StdoutLock) -> io::Result<()> {
        let width = self.playfield_width();
        let x = self.playfield_x() - 1;
        if x <= 0 {
            return Ok(());
        }

        stdout.write_all(b"\x1B[90m")?;

        for y in 1..=self.size.ws_row {
            write!(stdout, "\x1B[{y};{x}H▐\x1B[{width}C▌")?;
        }

        // Default color.
        stdout.write_all(b"\x1B[39m")?;

        Ok(())
    }

    pub fn draw_playfield(&mut self, stdout: &mut io::StdoutLock) -> io::Result<()> {
        self.render();

        let x = self.playfield_x();
        if x <= 0 {
            return Ok(());
        }

        let lane_count = self.state.lane_count();
        let lane_width = self.lane_width();

        let mut last_color = 39;
        let mut last_bg = 49;

        let mut iter = self.buffer.chunks(lane_count);
        let mut rev;
        let iter: &mut dyn Iterator<Item = &[(i8, Color)]> = if self.downscroll {
            rev = iter.rev();
            &mut rev
        } else {
            &mut iter
        };

        for (i, row) in iter.enumerate() {
            write!(stdout, "\x1B[{y};{x}H", y = i + 1)?;

            for (lane, (fill, color)) in row.iter().enumerate() {
                let fill = if self.downscroll { -*fill } else { *fill };
                let c = match fill {
                    -18 | 11 => "▔",
                    -17 | 12 => "🭶",
                    -16 | 13 => "🭷",
                    -15 | 14 => "🭸",
                    -14 | 15 => "🭹",
                    -13 | 16 => "🭺",
                    -12 | 17 => "🭻",
                    -11 | 18 => "▁",

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
                    let color = lane_color(lane_count, lane, *color);
                    if color != last_color {
                        write!(stdout, "\x1B[{color}m")?;
                        last_color = color;
                    }
                }

                let pressed = self.is_lane_pressed[lane];
                let past_judgement = if self.downscroll {
                    i64::from(self.size.ws_row) - self.judgement_y() - 1 <= i as i64
                } else {
                    i as i64 <= self.judgement_y()
                };
                let bg = if pressed && past_judgement { 100 } else { 49 };
                if bg != last_bg {
                    write!(stdout, "\x1B[{bg}m")?;
                    last_bg = bg;
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
        if last_bg != 49 {
            stdout.write_all(b"\x1B[49m")?;
        }

        Ok(())
    }
}

fn lane_color(lane_count: usize, lane: usize, color: Color) -> u8 {
    match color {
        Color::TimingLine => return 90,
        Color::JudgementLine => return 39,
        _ => (),
    };

    let alt = match (lane * 2 + 1).cmp(&lane_count) {
        Ordering::Less => lane % 2 == 1,
        Ordering::Equal => return 93,
        Ordering::Greater => (lane_count - lane - 1) % 2 == 1,
    };
    if alt { 96 } else { 39 }
}

fn lane_for_key(lane_count: usize, key: char) -> Option<usize> {
    let lane = match lane_count {
        4 => match key {
            's' => 0,
            'd' => 1,
            'k' => 2,
            'l' => 3,
            _ => return None,
        },
        7 => match key {
            'a' => 0,
            's' => 1,
            'd' => 2,
            ' ' => 3,
            'k' => 4,
            'l' => 5,
            ';' => 6,
            _ => return None,
        },
        _ => return None,
    };
    Some(lane)
}
