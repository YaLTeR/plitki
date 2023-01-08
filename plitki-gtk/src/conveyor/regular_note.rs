use gtk::glib;
use gtk::subclass::prelude::*;
use plitki_core::scroll::Position;

use super::note::Note;
use crate::conveyor::widget::{ConveyorWidget, ConveyorWidgetExt};
use crate::skin::LaneSkin;

mod imp {
    use gtk::prelude::*;
    use once_cell::unsync::OnceCell;

    use super::*;
    use crate::conveyor::note::NoteImpl;
    use crate::conveyor::widget::ConveyorWidgetImpl;

    #[derive(Debug, Default)]
    pub struct RegularNote {
        picture: OnceCell<gtk::Picture>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RegularNote {
        const NAME: &'static str = "PlitkiRegularNote";
        type Type = super::RegularNote;
        type ParentType = Note;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
            klass.set_css_name("plitki-regular-note");
        }
    }

    impl ObjectImpl for RegularNote {
        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();

            let picture = gtk::Picture::new();
            picture.set_parent(&*obj);
            self.picture.set(picture).unwrap();
        }

        fn dispose(&self) {
            self.picture.get().unwrap().unparent();
        }
    }

    impl WidgetImpl for RegularNote {}
    impl ConveyorWidgetImpl for RegularNote {}
    impl NoteImpl for RegularNote {}

    impl RegularNote {
        pub fn set_skin(&self, skin: Option<&LaneSkin>) {
            let texture = skin.map(|s| &s.object);
            self.picture.get().unwrap().set_paintable(texture);
        }
    }
}

glib::wrapper! {
    pub struct RegularNote(ObjectSubclass<imp::RegularNote>)
        @extends Note, ConveyorWidget, gtk::Widget;
}

impl RegularNote {
    pub fn new(position: Position) -> Self {
        let widget: Self = glib::Object::builder().build();
        widget.set_position(position);
        widget
    }

    pub fn set_skin(&self, skin: Option<&LaneSkin>) {
        self.imp().set_skin(skin);
    }
}
