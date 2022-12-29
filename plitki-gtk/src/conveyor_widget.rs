//! Widget that can appear on a [`Conveyor`].
//!
//! Examples are regular and long notes and timing lines.
//!
//! [`ConveyorWidget`]s have a position, which tells [`Conveyor`] where to draw them. They also know
//! whether they are hit (in which case they should be hidden) and whether they are missed (which
//! they track to set a `missed` CSS class on themselves).

use glib::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;
use plitki_core::scroll::Position;

mod imp {
    use std::cell::Cell;

    use gtk::prelude::*;

    use super::*;

    #[derive(Debug)]
    pub struct ConveyorWidget {
        position: Cell<Position>,
        is_hit: Cell<bool>,
        is_missed: Cell<bool>,
    }

    impl Default for ConveyorWidget {
        fn default() -> Self {
            Self {
                position: Cell::new(Position::zero()),
                is_hit: Cell::new(false),
                is_missed: Cell::new(false),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ConveyorWidget {
        const NAME: &'static str = "PlitkiConveyorWidget";
        type Type = super::ConveyorWidget;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for ConveyorWidget {}
    impl WidgetImpl for ConveyorWidget {}

    impl ConveyorWidget {
        pub fn position(&self) -> Position {
            self.position.get()
        }

        pub fn set_position(&self, value: Position) {
            self.position.set(value);
        }

        pub fn is_hit(&self) -> bool {
            self.is_hit.get()
        }

        pub fn set_hit(&self, value: bool) {
            self.is_hit.set(value);
        }

        pub fn is_missed(&self) -> bool {
            self.is_missed.get()
        }

        pub fn set_missed(&self, value: bool) {
            if self.is_missed.get() == value {
                return;
            }

            self.is_missed.set(value);

            if value {
                self.obj().add_css_class("missed");
            } else {
                self.obj().remove_css_class("missed");
            }
        }
    }
}

glib::wrapper! {
    pub struct ConveyorWidget(ObjectSubclass<imp::ConveyorWidget>)
        @extends gtk::Widget;
}

pub trait ConveyorWidgetExt {
    fn position(&self) -> Position;
    fn set_position(&self, value: Position);
    fn is_hit(&self) -> bool;
    fn set_hit(&self, value: bool);
    fn is_missed(&self) -> bool;
    fn set_missed(&self, value: bool);
}

impl<T: IsA<ConveyorWidget>> ConveyorWidgetExt for T {
    fn position(&self) -> Position {
        self.as_ref().imp().position()
    }

    fn set_position(&self, value: Position) {
        self.as_ref().imp().set_position(value);
    }

    fn is_hit(&self) -> bool {
        self.as_ref().imp().is_hit()
    }

    fn set_hit(&self, value: bool) {
        self.as_ref().imp().set_hit(value);
    }

    fn is_missed(&self) -> bool {
        self.as_ref().imp().is_missed()
    }

    fn set_missed(&self, value: bool) {
        self.as_ref().imp().set_missed(value);
    }
}

pub trait ConveyorWidgetImpl: WidgetImpl {}
unsafe impl<T: ConveyorWidgetImpl> IsSubclassable<T> for ConveyorWidget {}
