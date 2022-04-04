use gtk::glib;
use gtk::subclass::prelude::*;
use plitki_core::state::Hit;
use plitki_core::timing::GameTimestamp;

mod imp {
    use gtk::prelude::*;
    use plitki_core::timing::GameTimestampDifference;
    use std::cell::{Cell, RefCell};

    use gtk::{gdk, graphene};

    use super::*;

    #[derive(Debug)]
    pub struct HitError {
        timestamp: Cell<GameTimestamp>,
        hits: RefCell<Vec<Hit>>,
    }

    impl Default for HitError {
        fn default() -> Self {
            Self {
                timestamp: Cell::new(GameTimestamp::zero()),
                hits: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HitError {
        const NAME: &'static str = "PlitkiHitError";
        type Type = super::HitError;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for HitError {}

    impl WidgetImpl for HitError {
        fn request_mode(&self, _widget: &Self::Type) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(
            &self,
            _widget: &Self::Type,
            orientation: gtk::Orientation,
            _for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let (min, nat) = match orientation {
                gtk::Orientation::Horizontal => (2, 255),
                gtk::Orientation::Vertical => (3, 32),
                _ => unreachable!(),
            };

            (min, nat, -1, -1)
        }

        fn snapshot(&self, widget: &Self::Type, snapshot: &gtk::Snapshot) {
            let width = widget.width();
            let height = widget.height();

            let mid_bar_w = 2 - width % 2;
            let mid_bar_x = width / 2;

            snapshot.append_color(
                &gdk::RGBA::new(1., 1., 1., 0.5),
                &graphene::Rect::new(mid_bar_x as _, 0 as _, mid_bar_w as _, height as _),
            );

            let timestamp = self.timestamp.get();

            let highest_difference =
                GameTimestampDifference::from_millis(76).into_milli_hundredths();

            for hit in &*self.hits.borrow() {
                let diff = hit.difference.into_milli_hundredths();

                let x = (diff as f32 / ((highest_difference * 2) as f32 / width as f32).round()
                    + (width / 2) as f32)
                    .clamp(0., width as f32);

                let alpha = (1.
                    - (timestamp - hit.timestamp).into_milli_hundredths() as f32 / 100000.)
                    .clamp(0., 1.);

                snapshot.append_color(
                    &gdk::RGBA::new(1., 1., 1., alpha),
                    &graphene::Rect::new(x, (height / 4) as _, 1., (height / 2) as _),
                );
            }
        }
    }

    impl HitError {
        pub fn update(&self, timestamp: GameTimestamp, hits: Vec<Hit>) {
            self.timestamp.set(timestamp);
            *self.hits.borrow_mut() = hits;
            self.instance().queue_draw();
        }
    }
}

glib::wrapper! {
    pub struct HitError(ObjectSubclass<imp::HitError>)
        @extends gtk::Widget;
}

impl HitError {
    pub fn new() -> Self {
        glib::Object::new(&[]).unwrap()
    }

    pub fn update(&self, timestamp: GameTimestamp, hits: Vec<Hit>) {
        self.imp().update(timestamp, hits);
    }
}

impl Default for HitError {
    fn default() -> Self {
        Self::new()
    }
}
