use bitflags::bitflags;

use crate::app::App;

pub struct Parser {
    vte: vte::Parser,
}

#[derive(Debug)]
pub enum Key {
    Char(char),
    F3,
    F4,
}

#[derive(Debug)]
pub enum Event {
    Key {
        key: Key,
        mods: Modifier,
        release: bool,
    },
    KittyKeyboardSupported,
    PrimaryDeviceAttributes,
}

bitflags! {
    #[derive(Debug, PartialEq, Eq)]
    pub struct Modifier : u16 {
        const Shift = 1;
        const Alt = 1 << 1;
        const Ctrl = 1 << 2;
        const Super = 1 << 3;
        const Hyper = 1 << 4;
        const Meta = 1 << 5;

        // We don't care about these two.
        // const CapsLock = 1 << 6;
        // const NumLock = 1 << 7;
    }
}

struct Performer<'a>(&'a mut App);

impl Parser {
    pub fn new() -> Self {
        Self {
            vte: vte::Parser::new(),
        }
    }

    pub fn advance(&mut self, app: &mut App, bytes: &[u8]) {
        self.vte.advance(&mut Performer(app), bytes);
    }
}

impl vte::Perform for Performer<'_> {
    fn print(&mut self, _c: char) {}

    fn execute(&mut self, _byte: u8) {}

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {
    }

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        // eprintln!(
        //     "csi dispatch params={params:?} intermediates={intermediates:?} \
        //      ignore={ignore} action={action}\r"
        // );

        if ignore {
            return;
        }

        let event = match action {
            'u' => {
                if intermediates == b"?" {
                    Event::KittyKeyboardSupported
                } else if intermediates.is_empty() {
                    // Key event.
                    let mut params = params.iter();
                    let Some(key) = params.next() else {
                        eprintln!("no unicode-key-code param\r");
                        return;
                    };

                    // Try to get base-layout-key, fall back to unicode-key-code.
                    let Some(code) = key.get(2).or(key.get(0)) else {
                        eprintln!("no unicode-key-code argument\r");
                        return;
                    };
                    let Ok(key) = char::try_from(u32::from(*code)) else {
                        eprintln!("invalid codepoint\r");
                        return;
                    };

                    let Some((mods, release)) = parse_mods_release(params.next()) else {
                        return;
                    };

                    Event::Key {
                        key: Key::Char(key),
                        mods,
                        release,
                    }
                } else {
                    return;
                }
            }
            '~' if intermediates.is_empty() => {
                let mut params = params.iter();
                let Some([key, ..]) = params.next() else {
                    return;
                };

                let key = match key {
                    13 => Key::F3,
                    14 => Key::F4,
                    _ => return,
                };

                let Some((mods, release)) = parse_mods_release(params.next()) else {
                    return;
                };

                Event::Key { key, mods, release }
            }
            'S' if intermediates.is_empty() => {
                let mut params = params.iter();
                let Some([key, ..]) = params.next() else {
                    return;
                };

                let key = match key {
                    0 | 1 => Key::F4,
                    _ => return,
                };

                let Some((mods, release)) = parse_mods_release(params.next()) else {
                    return;
                };

                Event::Key { key, mods, release }
            }
            'c' => {
                if intermediates == b"?" {
                    Event::PrimaryDeviceAttributes
                } else {
                    return;
                }
            }
            _ => return,
        };

        let res = self.0.event(event);
        self.0.stop_on_error(res);
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}

    fn terminated(&self) -> bool {
        false
    }
}

fn parse_mods_release(opts: Option<&[u16]>) -> Option<(Modifier, bool)> {
    let mut mods = Modifier::empty();
    let mut release = false;
    if let Some(opts) = opts {
        if let Some(x) = opts.get(0) {
            if *x == 0 {
                eprintln!("invalid key modifier = 0\r");
            } else {
                mods = Modifier::from_bits_truncate(x - 1);
            }
        }

        match opts.get(1) {
            // Key repeat.
            Some(2) => return None,
            // Key release.
            Some(3) => release = true,
            // Key press.
            Some(1) | None => (),
            // Unrecognized.
            Some(x) => {
                eprintln!("unrecognized event type: {x}\r");
                return None;
            }
        }
    }
    Some((mods, release))
}
