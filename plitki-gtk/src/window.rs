use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

mod imp {
    use std::cell::RefCell;

    use anyhow::Context;
    use gtk::{gdk, gdk_pixbuf, CompositeTemplate, ResponseType};
    use log::info;
    use once_cell::unsync::OnceCell;
    use plitki_core::map::Map;
    use plitki_core::scroll::ScreenPositionDifference;

    use super::*;
    use crate::long_note::LongNote;
    use crate::playfield::Playfield;
    use crate::skin::{LaneSkin, Skin, Store};

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

        let mut store = Store::new();

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

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/plitki-gtk/window.ui")]
    pub struct ApplicationWindow {
        #[template_child]
        button_open: TemplateChild<gtk::Button>,

        #[template_child]
        button_upscroll: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        button_downscroll: TemplateChild<gtk::ToggleButton>,

        #[template_child]
        button_arrows: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        button_bars: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        button_circles: TemplateChild<gtk::ToggleButton>,

        #[template_child]
        scrolled_window_playfield: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        scale_scroll_speed: TemplateChild<gtk::Scale>,

        #[template_child]
        box_long_note: TemplateChild<gtk::Box>,
        #[template_child]
        scale_length: TemplateChild<gtk::Scale>,

        playfield: OnceCell<RefCell<Playfield>>,
        scroll_speed_binding: OnceCell<RefCell<glib::Binding>>,
        long_note: RefCell<Option<LongNote>>,

        skin_arrows: OnceCell<Skin>,
        skin_bars: OnceCell<Skin>,
        skin_circles: OnceCell<Skin>,
        skin_current: OnceCell<RefCell<Skin>>,
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

            self.skin_arrows
                .set(create_skin("/plitki-gtk/skin/arrows"))
                .unwrap();
            self.skin_bars
                .set(create_skin("/plitki-gtk/skin/bars"))
                .unwrap();
            self.skin_circles
                .set(create_skin("/plitki-gtk/skin/circles"))
                .unwrap();
            self.skin_current
                .set(RefCell::new(self.skin_arrows.get().unwrap().clone()))
                .unwrap();

            let qua = plitki_map_qua::from_reader(
                &include_bytes!("../../plitki-map-qua/tests/data/actual_map.qua")[..],
            )
            .unwrap();
            let map: Map = qua.try_into().unwrap();
            let playfield = Playfield::new(map, &*self.skin_current.get().unwrap().borrow());

            playfield.set_halign(gtk::Align::Center);
            playfield.set_valign(gtk::Align::Center);
            playfield.set_vexpand(true);

            let binding = playfield
                .bind_property(
                    "scroll-speed",
                    &self.scale_scroll_speed.adjustment(),
                    "value",
                )
                .flags(glib::BindingFlags::BIDIRECTIONAL | glib::BindingFlags::SYNC_CREATE)
                .build()
                .unwrap();
            self.scroll_speed_binding
                .set(RefCell::new(binding))
                .unwrap();

            self.scrolled_window_playfield.set_child(Some(&playfield));

            self.playfield.set(RefCell::new(playfield)).unwrap();

            self.set_skin(self.skin_arrows.get().unwrap());

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
                        if let Err(err) = obj.open(file).with_context(|| "couldn't load the map") {
                            info!("{:?}", err);
                        }
                    });
                }
            });

            self.button_upscroll.connect_toggled({
                let obj = obj.downgrade();
                move |button| {
                    let obj = obj.upgrade().unwrap();
                    let self_ = Self::from_instance(&obj);

                    if button.is_active() {
                        self_
                            .playfield
                            .get()
                            .unwrap()
                            .borrow()
                            .set_property("downscroll", false)
                            .unwrap();
                        self_
                            .long_note
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .remove_css_class("upside-down");
                    }
                }
            });

            self.button_downscroll.connect_toggled({
                let obj = obj.downgrade();
                move |button| {
                    let obj = obj.upgrade().unwrap();
                    let self_ = Self::from_instance(&obj);

                    if button.is_active() {
                        self_
                            .playfield
                            .get()
                            .unwrap()
                            .borrow()
                            .set_property("downscroll", true)
                            .unwrap();
                        self_
                            .long_note
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .add_css_class("upside-down");
                    }
                }
            });

            self.button_arrows.connect_toggled({
                let obj = obj.downgrade();
                move |button| {
                    let obj = obj.upgrade().unwrap();
                    let self_ = Self::from_instance(&obj);

                    if button.is_active() {
                        self_.set_skin(self_.skin_arrows.get().unwrap());
                    }
                }
            });

            self.button_bars.connect_toggled({
                let obj = obj.downgrade();
                move |button| {
                    let obj = obj.upgrade().unwrap();
                    let self_ = Self::from_instance(&obj);

                    if button.is_active() {
                        self_.set_skin(self_.skin_bars.get().unwrap());
                    }
                }
            });

            self.button_circles.connect_toggled({
                let obj = obj.downgrade();
                move |button| {
                    let obj = obj.upgrade().unwrap();
                    let self_ = Self::from_instance(&obj);

                    if button.is_active() {
                        self_.set_skin(self_.skin_circles.get().unwrap());
                    }
                }
            });
        }
    }

    impl WidgetImpl for ApplicationWindow {}
    impl WindowImpl for ApplicationWindow {}

    impl ApplicationWindowImpl for ApplicationWindow {}

    impl ApplicationWindow {
        pub fn open(&self, file: gio::File) -> anyhow::Result<()> {
            let bytes = file
                .load_contents(None::<&gio::Cancellable>)
                .with_context(|| "couldn't read the file")?
                .0;

            let qua = plitki_map_qua::from_reader(&bytes[..])
                .with_context(|| "couldn't parse the file as a .qua map")?;
            let map: Map = qua
                .try_into()
                .with_context(|| "couldn't convert the map to plitki's format")?;
            let playfield = Playfield::new(map, &*self.skin_current.get().unwrap().borrow());

            playfield.set_halign(gtk::Align::Center);
            playfield.set_valign(gtk::Align::Center);
            playfield.set_vexpand(true);

            if self.button_downscroll.is_active() {
                playfield.set_property("downscroll", true).unwrap();
            }

            self.scrolled_window_playfield.set_child(Some(&playfield));

            self.scroll_speed_binding.get().unwrap().borrow().unbind();

            let binding = playfield
                .bind_property(
                    "scroll-speed",
                    &self.scale_scroll_speed.adjustment(),
                    "value",
                )
                .flags(glib::BindingFlags::BIDIRECTIONAL | glib::BindingFlags::SYNC_CREATE)
                .build()
                .unwrap();

            *self.playfield.get().unwrap().borrow_mut() = playfield;
            *self.scroll_speed_binding.get().unwrap().borrow_mut() = binding;

            Ok(())
        }

        fn set_skin(&self, skin: &Skin) {
            *self.skin_current.get().unwrap().borrow_mut() = skin.clone();

            let mut long_note_field = self.long_note.borrow_mut();
            let length = if let Some(long_note) = &*long_note_field {
                self.box_long_note.remove(long_note);
                self.playfield
                    .get()
                    .unwrap()
                    .borrow()
                    .set_property("skin", skin)
                    .unwrap();
                long_note.property("length").unwrap().get::<i64>().unwrap()
            } else {
                0
            };

            let lane_skin = skin.store().get(4, 0);
            let long_note = LongNote::new(
                &lane_skin.ln_head,
                &lane_skin.ln_tail,
                &lane_skin.ln_body,
                1,
                ScreenPositionDifference(length),
            );

            long_note.set_halign(gtk::Align::Center);
            long_note.set_valign(gtk::Align::Center);
            long_note.set_vexpand(true);

            if self.button_downscroll.is_active() {
                long_note.add_css_class("upside-down");
            }

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

    fn open(&self, file: gio::File) -> anyhow::Result<()> {
        imp::ApplicationWindow::from_instance(self).open(file)
    }
}
