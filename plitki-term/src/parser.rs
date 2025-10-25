use crate::app::App;

pub struct Parser {
    vte: vte::Parser,
}

pub enum Event {
    Key(char),
    KittyKeyboardSupported,
    PrimaryDeviceAttributes,
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
        eprintln!(
            "csi dispatch params={params:?} intermediates={intermediates:?} \
             ignore={ignore} action={action}\r"
        );

        if ignore {
            return;
        }

        let event = match action {
            'u' => {
                if intermediates == b"?" {
                    Event::KittyKeyboardSupported
                } else {
                    return;
                }
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
