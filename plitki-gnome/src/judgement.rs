use gtk::glib;
use gtk::subclass::prelude::*;
use plitki_core::state::Hit;
use plitki_core::timing::GameTimestamp;

mod imp {
    use std::cell::Cell;

    use gtk::prelude::*;
    use gtk::{gdk, graphene};

    use super::*;

    #[derive(Debug)]
    pub struct Judgement {
        timestamp: Cell<GameTimestamp>,
        last_hit: Cell<Option<Hit>>,
    }

    impl Default for Judgement {
        fn default() -> Self {
            Self {
                timestamp: Cell::new(GameTimestamp::zero()),
                last_hit: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Judgement {
        const NAME: &'static str = "PlitkiJudgement";
        type Type = super::Judgement;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("plitki-judgement");
        }
    }

    impl ObjectImpl for Judgement {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().set_overflow(gtk::Overflow::Hidden);
        }
    }

    impl WidgetImpl for Judgement {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let (min, nat) = match orientation {
                gtk::Orientation::Horizontal => (1, 300),
                gtk::Orientation::Vertical => (1, 100),
                _ => unreachable!(),
            };

            (min, nat, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();

            let hit = match self.last_hit.get() {
                Some(x) => x,
                None => return,
            };

            let timestamp = self.timestamp.get();

            let alpha = (1. - (timestamp - hit.timestamp).into_milli_hundredths() as f32 / 25000.)
                .clamp(0., 1.);

            let diff = hit.difference.into_milli_hundredths();

            // Quaver Standard judgements.
            let color = match diff.abs() / 100 {
                // Marvellous judgements are hidden.
                0..=18 => gdk::RGBA::new(0.98, 1., 0.71, alpha),
                19..=43 => gdk::RGBA::new(1., 0.91, 0.42, alpha),
                44..=76 => gdk::RGBA::new(0.34, 1., 0.43, alpha),
                77..=106 => gdk::RGBA::new(0., 0.82, 1., alpha),
                107..=127 => gdk::RGBA::new(0.85, 0.42, 0.81, alpha),
                128..=164 => gdk::RGBA::new(0.98, 0.39, 0.36, alpha),
                _ => gdk::RGBA::new(1., 1., 1., alpha),
            };

            snapshot.append_color(
                &color,
                &graphene::Rect::new(0., 0., widget.width() as _, widget.height() as _),
            );
        }
    }

    impl Judgement {
        pub fn update(&self, timestamp: GameTimestamp, last_hit: Option<Hit>) {
            if let Some(last_hit) = last_hit {
                // Ignore marvellous judgements.
                if last_hit.difference.into_milli_hundredths().abs() / 100 > 18 {
                    self.last_hit.set(Some(last_hit));
                }
            }

            self.timestamp.set(timestamp);
            self.obj().queue_draw();
        }
    }
}

glib::wrapper! {
    pub struct Judgement(ObjectSubclass<imp::Judgement>)
        @extends gtk::Widget;
}

impl Judgement {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn update(&self, timestamp: GameTimestamp, last_hit: Option<Hit>) {
        self.imp().update(timestamp, last_hit);
    }
}

impl Default for Judgement {
    fn default() -> Self {
        Self::new()
    }
}
