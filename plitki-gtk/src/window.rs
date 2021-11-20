use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

mod imp {
    use std::cell::RefCell;

    use gtk::{CompositeTemplate, ResponseType};
    use once_cell::unsync::OnceCell;
    use plitki_core::map::Map;
    use plitki_core::scroll::ScreenPositionDifference;

    use super::*;
    use crate::long_note::LongNote;
    use crate::skin::load_texture;
    use crate::view::View;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(file = "window.ui")]
    pub struct ApplicationWindow {
        #[template_child]
        button_open: TemplateChild<gtk::Button>,

        #[template_child]
        viewport_playfield: TemplateChild<gtk::Viewport>,
        #[template_child]
        scale_scroll_speed: TemplateChild<gtk::Scale>,

        #[template_child]
        box_long_note: TemplateChild<gtk::Box>,
        #[template_child]
        scale_length: TemplateChild<gtk::Scale>,

        view: OnceCell<View>,
        long_note: RefCell<Option<LongNote>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ApplicationWindow {
        const NAME: &'static str = "PlitkiGtkWindow";
        type Type = super::ApplicationWindow;
        type ParentType = gtk::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ApplicationWindow {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let qua = plitki_map_qua::from_reader(
                &include_bytes!("../../plitki-map-qua/tests/data/actual_map.qua")[..],
            )
            .unwrap();
            let map: Map = qua.try_into().unwrap();
            let view = View::new(map);

            view.add_css_class("upside-down");

            view.bind_property(
                "scroll-speed",
                &self.scale_scroll_speed.adjustment(),
                "value",
            )
            .flags(glib::BindingFlags::BIDIRECTIONAL | glib::BindingFlags::SYNC_CREATE)
            .build()
            .unwrap();

            self.viewport_playfield.set_child(Some(&view));

            self.view.set(view).unwrap();

            self.rebuild();

            self.button_open.connect_clicked({
                let obj = obj.downgrade();
                move |_| {
                    let obj = obj.upgrade().unwrap();

                    let file_chooser = gtk::FileChooserNativeBuilder::new()
                        .transient_for(&obj)
                        .action(gtk::FileChooserAction::Open)
                        .title("Open a .qua map")
                        .transient_for(&obj)
                        .modal(true)
                        .build();

                    glib::MainContext::default().spawn_local(async move {
                        if file_chooser.run_future().await != ResponseType::Accept {
                            return;
                        }

                        let file = file_chooser.file().unwrap();
                        obj.open(file);
                    });
                }
            });
        }
    }

    impl WidgetImpl for ApplicationWindow {}
    impl WindowImpl for ApplicationWindow {}

    impl ApplicationWindowImpl for ApplicationWindow {}

    impl ApplicationWindow {
        pub fn open(&self, file: gio::File) {
            // TODO
        }

        fn rebuild(&self) {
            let mut long_note_field = self.long_note.borrow_mut();
            if let Some(long_note) = &*long_note_field {
                self.box_long_note.remove(long_note);
                self.view.get().unwrap().rebuild();
            }

            let long_note = LongNote::new(
                &gtk::Picture::builder()
                    .paintable(&load_texture("note-holdhitobject-1.png"))
                    .css_classes(vec!["upside-down".to_string()])
                    .build(),
                &gtk::Picture::builder()
                    .paintable(&load_texture("note-holdend-1.png"))
                    .css_classes(vec!["upside-down".to_string()])
                    .build(),
                &gtk::Picture::builder()
                    .paintable(&load_texture("note-holdbody-1.png"))
                    .keep_aspect_ratio(false)
                    .css_classes(vec!["upside-down".to_string()])
                    .build(),
                1,
                ScreenPositionDifference::default(),
            );

            long_note.set_halign(gtk::Align::Center);
            long_note.set_valign(gtk::Align::Center);
            long_note.set_vexpand(true);
            long_note.add_css_class("upside-down");

            long_note
                .bind_property("length", &self.scale_length.adjustment(), "value")
                .flags(glib::BindingFlags::BIDIRECTIONAL | glib::BindingFlags::SYNC_CREATE)
                .build()
                .unwrap();

            self.box_long_note.prepend(&long_note);

            *long_note_field = Some(long_note);
        }
    }
}

glib::wrapper! {
    pub struct ApplicationWindow(ObjectSubclass<imp::ApplicationWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl ApplicationWindow {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::new(&[("application", app)]).unwrap()
    }

    fn open(&self, file: gio::File) {
        imp::ApplicationWindow::from_instance(self).open(file)
    }
}
