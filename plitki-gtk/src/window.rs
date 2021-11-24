use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

mod imp {
    use std::cell::RefCell;

    use anyhow::Context;
    use gtk::{CompositeTemplate, ResponseType};
    use log::info;
    use once_cell::unsync::OnceCell;
    use plitki_core::map::Map;
    use plitki_core::scroll::ScreenPositionDifference;

    use super::*;
    use crate::long_note::LongNote;
    use crate::skin::{load_texture, Skin, SKIN};
    use crate::view::View;

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

        view: OnceCell<RefCell<View>>,
        scroll_speed_binding: OnceCell<RefCell<glib::Binding>>,
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

            view.set_halign(gtk::Align::Center);
            view.set_valign(gtk::Align::Center);
            view.set_vexpand(true);

            let binding = view
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

            self.scrolled_window_playfield.set_child(Some(&view));

            self.view.set(RefCell::new(view)).unwrap();

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
                            .view
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
                            .view
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
                        *SKIN.write().unwrap() = Skin::Arrows;
                        self_.rebuild();
                    }
                }
            });

            self.button_bars.connect_toggled({
                let obj = obj.downgrade();
                move |button| {
                    let obj = obj.upgrade().unwrap();
                    let self_ = Self::from_instance(&obj);

                    if button.is_active() {
                        *SKIN.write().unwrap() = Skin::Bars;
                        self_.rebuild();
                    }
                }
            });

            self.button_circles.connect_toggled({
                let obj = obj.downgrade();
                move |button| {
                    let obj = obj.upgrade().unwrap();
                    let self_ = Self::from_instance(&obj);

                    if button.is_active() {
                        *SKIN.write().unwrap() = Skin::Circles;
                        self_.rebuild();
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
            let view = View::new(map);

            view.set_halign(gtk::Align::Center);
            view.set_valign(gtk::Align::Center);
            view.set_vexpand(true);

            if self.button_downscroll.is_active() {
                view.set_property("downscroll", true).unwrap();
            }

            self.scrolled_window_playfield.set_child(Some(&view));

            self.scroll_speed_binding.get().unwrap().borrow().unbind();

            let binding = view
                .bind_property(
                    "scroll-speed",
                    &self.scale_scroll_speed.adjustment(),
                    "value",
                )
                .flags(glib::BindingFlags::BIDIRECTIONAL | glib::BindingFlags::SYNC_CREATE)
                .build()
                .unwrap();

            *self.view.get().unwrap().borrow_mut() = view;
            *self.scroll_speed_binding.get().unwrap().borrow_mut() = binding;

            Ok(())
        }

        fn rebuild(&self) {
            let mut long_note_field = self.long_note.borrow_mut();
            let length = if let Some(long_note) = &*long_note_field {
                self.box_long_note.remove(long_note);
                self.view.get().unwrap().borrow().rebuild();
                long_note.property("length").unwrap().get::<i64>().unwrap()
            } else {
                0
            };

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
