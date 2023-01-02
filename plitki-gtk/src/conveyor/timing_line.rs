use gtk::glib;
use gtk::subclass::prelude::*;
use plitki_core::scroll::Position;

use crate::conveyor::widget::{ConveyorWidget, ConveyorWidgetExt};

mod imp {
    use gtk::prelude::*;
    use once_cell::unsync::OnceCell;

    use super::*;
    use crate::conveyor::widget::ConveyorWidgetImpl;

    #[derive(Debug, Default)]
    pub struct TimingLine {
        separator: OnceCell<gtk::Separator>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TimingLine {
        const NAME: &'static str = "PlitkiTimingLine";
        type Type = super::TimingLine;
        type ParentType = ConveyorWidget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
            klass.set_css_name("plitki-timing-line");
        }
    }

    impl ObjectImpl for TimingLine {
        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();

            let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
            separator.set_parent(&*obj);
            self.separator.set(separator).unwrap();
        }

        fn dispose(&self) {
            self.separator.get().unwrap().unparent();
        }
    }

    impl WidgetImpl for TimingLine {}
    impl ConveyorWidgetImpl for TimingLine {}
}

glib::wrapper! {
    pub struct TimingLine(ObjectSubclass<imp::TimingLine>)
        @extends ConveyorWidget, gtk::Widget;
}

impl TimingLine {
    pub fn new(position: Position) -> Self {
        let widget: Self = glib::Object::builder().build();
        widget.set_position(position);
        widget
    }
}
