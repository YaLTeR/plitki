//! Note that can appear on a [`Conveyor`].
//!
//! Notes are [`ConveyorWidget`]s that also know whether they are missed (which they track to set a
//! `missed` CSS class on themselves).
//!
//! [`Conveyor`]: crate::conveyor::Conveyor
//! [`ConveyorWidget`]: crate::conveyor::widget::ConveyorWidget

use glib::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;

use super::widget::{ConveyorWidget, ConveyorWidgetImpl};

mod imp {
    use std::cell::Cell;

    use gtk::prelude::*;

    use super::*;

    #[derive(Debug, Default)]
    pub struct Note {
        is_missed: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Note {
        const NAME: &'static str = "PlitkiNote";
        type Type = super::Note;
        type ParentType = ConveyorWidget;
    }

    impl ObjectImpl for Note {}
    impl WidgetImpl for Note {}
    impl ConveyorWidgetImpl for Note {}

    impl Note {
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
    pub struct Note(ObjectSubclass<imp::Note>)
        @extends ConveyorWidget, gtk::Widget;
}

pub trait NoteExt {
    fn is_missed(&self) -> bool;
    fn set_missed(&self, value: bool);
}

impl<T: IsA<Note>> NoteExt for T {
    fn is_missed(&self) -> bool {
        self.as_ref().imp().is_missed()
    }

    fn set_missed(&self, value: bool) {
        self.as_ref().imp().set_missed(value);
    }
}

pub trait NoteImpl: ConveyorWidgetImpl {}
unsafe impl<T: NoteImpl> IsSubclassable<T> for Note {}
