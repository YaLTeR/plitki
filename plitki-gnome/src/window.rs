use std::cell::RefCell;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use log::warn;

mod imp {
    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gdk, gdk_pixbuf, CompositeTemplate};
    use plitki_core::map::Map;
    use plitki_gtk::playfield::Playfield;
    use plitki_gtk::skin::{LaneSkin, Skin};

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/plitki-gnome/window.ui")]
    pub struct Window {
        #[template_child]
        stack: TemplateChild<gtk::Stack>,
        #[template_child]
        scrolled_window: TemplateChild<gtk::ScrolledWindow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "PlitkiWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let qua = plitki_map_qua::from_reader(
                &include_bytes!("../../plitki-map-qua/tests/data/actual_map.qua")[..],
            )
            .unwrap();
            let map: Map = qua.try_into().unwrap();
            let playfield = Playfield::new(map, &create_skin("/plitki-gnome/skin/arrows"));

            playfield.set_halign(gtk::Align::Center);
            playfield.set_valign(gtk::Align::End);
            playfield.set_downscroll(true);

            self.scrolled_window.set_child(Some(&playfield));
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}

    impl Window {
        pub fn open_file(&self, _file: &gio::File) {}
    }

    fn create_skin(path: &str) -> Skin {
        let load_texture = |path: &str| {
            // We're loading Quaver textures which are flipped with regards to what our widgets
            // expect.
            gdk::Texture::for_pixbuf(
                &gdk_pixbuf::Pixbuf::from_resource(path)
                    .unwrap()
                    .flip(false)
                    .unwrap(),
            )
        };

        let mut store = plitki_gtk::skin::Store::new();

        let mut element = Vec::new();
        for lane in 0..4 {
            let lane_skin = LaneSkin {
                object: load_texture(&format!("{}/4k/note-hitobject-{}.png", path, lane + 1)),
                ln_head: load_texture(&format!("{}/4k/note-holdhitobject-{}.png", path, lane + 1)),
                ln_body: load_texture(&format!("{}/4k/note-holdbody-{}.png", path, lane + 1)),
                ln_tail: load_texture(&format!("{}/4k/note-holdend-{}.png", path, lane + 1)),
            };

            element.push(lane_skin);
        }
        store.insert(4, element);

        let mut element = Vec::new();
        for lane in 0..7 {
            let lane_skin = LaneSkin {
                object: load_texture(&format!("{}/7k/note-hitobject-{}.png", path, lane + 1)),
                ln_head: load_texture(&format!("{}/7k/note-holdhitobject-{}.png", path, lane + 1)),
                ln_body: load_texture(&format!("{}/7k/note-holdbody-{}.png", path, lane + 1)),
                ln_tail: load_texture(&format!("{}/7k/note-holdend-{}.png", path, lane + 1)),
            };

            element.push(lane_skin);
        }
        store.insert(7, element);

        Skin::new(store)
    }
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap;
}

#[gtk::template_callbacks]
impl Window {
    pub fn new(app: &impl IsA<gtk::Application>) -> Self {
        glib::Object::new(&[("application", app)]).unwrap()
    }

    #[template_callback]
    fn on_open_clicked(&self) {
        let file_chooser = gtk::FileChooserNative::builder()
            .transient_for(self)
            .modal(true)
            .action(gtk::FileChooserAction::Open)
            .select_multiple(true)
            .build();

        file_chooser.connect_response({
            let obj = self.downgrade();
            let file_chooser = RefCell::new(Some(file_chooser.clone()));
            move |_, response| {
                if let Some(obj) = obj.upgrade() {
                    if let Some(file_chooser) = file_chooser.take() {
                        if response == gtk::ResponseType::Accept {
                            for file in file_chooser.files().snapshot().into_iter() {
                                let file: gio::File = file
                                    .downcast()
                                    .expect("unexpected type returned from file chooser");
                                obj.imp().open_file(&file);
                            }
                        }
                    } else {
                        warn!("got file chooser response more than once");
                    }
                } else {
                    warn!("got file chooser response after window was freed");
                }
            }
        });

        file_chooser.show();
    }
}
