//! Widget that can appear on a [`Conveyor`].
//!
//! Examples are regular and long notes and timing lines.
//!
//! [`ConveyorWidget`]s have a position, which tells [`Conveyor`] where to draw them. They also know
//! whether they are hidden.
//!
//! [`Conveyor`]: crate::conveyor::Conveyor

use gtk::glib;
use gtk::prelude::*;
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

    #[repr(C)]
    pub struct ConveyorWidgetClass {
        pub parent_class: gtk::ffi::GtkWidgetClass,
        pub natural_height_for_width: fn(&super::ConveyorWidget, width: i32) -> i32,
    }

    unsafe impl ClassStruct for ConveyorWidgetClass {
        type Type = ConveyorWidget;
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ConveyorWidget {
        const NAME: &'static str = "PlitkiConveyorWidget";
        type Type = super::ConveyorWidget;
        type ParentType = gtk::Widget;
        type Class = ConveyorWidgetClass;

        fn class_init(klass: &mut Self::Class) {
            klass.natural_height_for_width = super::natural_height_for_width_default_trampoline;
        }
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
    fn natural_height_for_width(&self, width: i32) -> i32;
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

    fn natural_height_for_width(&self, width: i32) -> i32 {
        let obj = self.as_ref();
        (obj.class().as_ref().natural_height_for_width)(obj, width)
    }
}

fn natural_height_for_width_default_trampoline(obj: &ConveyorWidget, width: i32) -> i32 {
    obj.measure(gtk::Orientation::Vertical, width).1
}

pub trait ConveyorWidgetImpl: WidgetImpl {
    fn natural_height_for_width(&self, width: i32) -> i32 {
        self.parent_natural_height_for_width(width)
    }
}

pub trait ConveyorWidgetImplExt: ObjectSubclass {
    fn parent_natural_height_for_width(&self, width: i32) -> i32;
}

impl<T: ConveyorWidgetImpl> ConveyorWidgetImplExt for T {
    fn parent_natural_height_for_width(&self, width: i32) -> i32 {
        unsafe {
            let data = Self::type_data();
            let parent_class = &*(data.as_ref().parent_class() as *mut imp::ConveyorWidgetClass);
            (parent_class.natural_height_for_width)(
                self.obj().unsafe_cast_ref::<ConveyorWidget>(),
                width,
            )
        }
    }
}

unsafe impl<T: ConveyorWidgetImpl> IsSubclassable<T> for ConveyorWidget {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class.upcast_ref_mut());

        let klass = class.as_mut();
        klass.natural_height_for_width = natural_height_for_width_trampoline::<T>;
    }
}

fn natural_height_for_width_trampoline<T: ObjectSubclass + ConveyorWidgetImpl>(
    obj: &ConveyorWidget,
    width: i32,
) -> i32 {
    let imp = obj.dynamic_cast_ref::<T::Type>().unwrap().imp();
    imp.natural_height_for_width(width)
}
