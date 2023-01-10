//! Widget that can appear on a [`Conveyor`].
//!
//! Examples are regular and long notes and timing lines.
//!
//! [`ConveyorWidget`]s have a position, which tells [`Conveyor`] where to draw them. They also know
//! whether they are hidden.
//!
//! [`Conveyor`]: crate::conveyor::Conveyor

use glib::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;
use plitki_core::scroll::Position;

mod imp {
    use std::cell::Cell;

    use super::*;

    #[derive(Debug)]
    pub struct ConveyorWidget {
        position: Cell<Position>,
        is_hidden: Cell<bool>,
    }

    impl Default for ConveyorWidget {
        fn default() -> Self {
            Self {
                position: Cell::new(Position::zero()),
                is_hidden: Cell::new(false),
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

        pub fn is_hidden(&self) -> bool {
            self.is_hidden.get()
        }

        pub fn set_hidden(&self, value: bool) {
            self.is_hidden.set(value);
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
    fn is_hidden(&self) -> bool;
    fn set_hidden(&self, value: bool);
}

impl<T: IsA<ConveyorWidget>> ConveyorWidgetExt for T {
    fn position(&self) -> Position {
        self.as_ref().imp().position()
    }

    fn set_position(&self, value: Position) {
        self.as_ref().imp().set_position(value);
    }

    fn is_hidden(&self) -> bool {
        self.as_ref().imp().is_hidden()
    }

    fn set_hidden(&self, value: bool) {
        self.as_ref().imp().set_hidden(value);
    }
}

pub trait ConveyorWidgetImpl: WidgetImpl {}
unsafe impl<T: ConveyorWidgetImpl> IsSubclassable<T> for ConveyorWidget {}
