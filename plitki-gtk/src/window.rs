use anyhow::Context;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use log::info;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::time::Duration;

    use anyhow::{anyhow, Context};
    use gtk::{gdk, gdk_pixbuf, CompositeTemplate, TickCallbackId};
    use once_cell::unsync::OnceCell;
    use plitki_core::map::Map;
    use plitki_core::state::GameState;
    use plitki_core::timing::{GameTimestampDifference, Timestamp};

    use super::*;
    use crate::long_note::LongNote;
    use crate::playfield::Playfield;
    use crate::skin::{LaneSkin, Skin};

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

        let skin = Skin::new(None);
        let mut store = skin.store_mut();

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

        drop(store);
        skin
    }

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/plitki-gtk/window.ui")]
    pub struct ApplicationWindow {
        #[template_child]
        playfield: TemplateChild<Playfield>,
        #[template_child]
        adjustment_timestamp: TemplateChild<gtk::Adjustment>,
        #[template_child]
        long_note: TemplateChild<LongNote>,

        tick_callback: RefCell<Option<TickCallbackId>>,

        skin_arrows: OnceCell<Skin>,
        skin_bars: OnceCell<Skin>,
        skin_circles: OnceCell<Skin>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ApplicationWindow {
        const NAME: &'static str = "PlitkiGtkWindow";
        type Type = super::ApplicationWindow;
        type ParentType = gtk::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ApplicationWindow {
        fn constructed(&self) {
            self.parent_constructed();

            self.skin_arrows
                .set(create_skin("/plitki-gtk/skin/arrows"))
                .unwrap();
            self.skin_bars
                .set(create_skin("/plitki-gtk/skin/bars"))
                .unwrap();
            self.skin_circles
                .set(create_skin("/plitki-gtk/skin/circles"))
                .unwrap();

            self.set_skin(self.skin_arrows.get().unwrap().clone());

            self.open_reader(&include_bytes!("../../plitki-map-qua/tests/data/actual_map.qua")[..])
                .unwrap();
        }
    }

    impl WidgetImpl for ApplicationWindow {}
    impl WindowImpl for ApplicationWindow {}

    impl ApplicationWindowImpl for ApplicationWindow {}

    #[gtk::template_callbacks]
    impl ApplicationWindow {
        pub fn open(&self, file: gio::File) -> anyhow::Result<()> {
            let bytes = file
                .load_contents(None::<&gio::Cancellable>)
                .with_context(|| "couldn't read the file")?
                .0;

            self.open_reader(&bytes[..])
        }

        fn open_reader(&self, reader: impl std::io::Read) -> anyhow::Result<()> {
            let qua = plitki_map_qua::from_reader(reader)
                .with_context(|| "couldn't parse the file as a .qua map")?;
            let map: Map = qua
                .try_into()
                .with_context(|| "couldn't convert the map to plitki's format")?;
            let state = GameState::new(map, GameTimestampDifference::from_millis(0))
                .map_err(|_| anyhow!("map has invalid objects"))?;

            self.adjustment_timestamp.configure(
                state
                    .first_timestamp()
                    .unwrap()
                    .into_milli_hundredths()
                    .into(),
                state
                    .first_timestamp()
                    .unwrap()
                    .into_milli_hundredths()
                    .into(),
                state
                    .last_timestamp()
                    .unwrap()
                    .into_milli_hundredths()
                    .into(),
                1.,
                10.,
                10.,
            );

            self.playfield.set_game_state(Some(state));

            Ok(())
        }

        fn set_skin(&self, skin: Skin) {
            let store = skin.store();
            let lane_skin = store.get(4, 0);
            self.long_note.set_head_paintable(Some(&lane_skin.ln_head));
            self.long_note.set_tail_paintable(Some(&lane_skin.ln_tail));
            self.long_note.set_body_paintable(Some(&lane_skin.ln_body));
            drop(store);

            self.playfield.set_skin(Some(skin));

            set_up_picture_drop_target(self.long_note.head().downcast().unwrap());
            set_up_picture_drop_target(self.long_note.tail().downcast().unwrap());
            set_up_picture_drop_target(self.long_note.body().downcast().unwrap());
        }

        #[template_callback]
        fn on_upscroll_toggled(&self, button: gtk::ToggleButton) {
            if button.is_active() {
                self.playfield.set_downscroll(false);
                self.long_note.remove_css_class("upside-down");
            }
        }

        #[template_callback]
        fn on_downscroll_toggled(&self, button: gtk::ToggleButton) {
            if button.is_active() {
                self.playfield.set_downscroll(true);
                self.long_note.add_css_class("upside-down");
            }
        }

        #[template_callback]
        fn on_arrows_toggled(&self, button: gtk::ToggleButton) {
            if button.is_active() {
                self.set_skin(self.skin_arrows.get().unwrap().clone());
            }
        }

        #[template_callback]
        fn on_bars_toggled(&self, button: gtk::ToggleButton) {
            if button.is_active() {
                self.set_skin(self.skin_bars.get().unwrap().clone());
            }
        }

        #[template_callback]
        fn on_circles_toggled(&self, button: gtk::ToggleButton) {
            if button.is_active() {
                self.set_skin(self.skin_circles.get().unwrap().clone());
            }
        }

        #[template_callback]
        fn on_play_pause_clicked(&self, button: gtk::Button) {
            let obj = self.obj();

            let mut callback = self.tick_callback.borrow_mut();
            match callback.take() {
                Some(callback) => {
                    callback.remove();
                    button.set_icon_name("media-playback-start-symbolic");
                }
                None => {
                    button.set_icon_name("media-playback-pause-symbolic");

                    let last_frame_time = Cell::new(obj.frame_clock().unwrap().frame_time());
                    *callback = Some(obj.add_tick_callback(move |obj, clock| {
                        let self_ = Self::from_instance(obj);

                        let frame_time = clock.frame_time();
                        let time_passed = Duration::from_micros(
                            (frame_time - last_frame_time.get()).try_into().unwrap(),
                        );
                        let time_passed = Timestamp::try_from(time_passed)
                            .unwrap()
                            .into_milli_hundredths();
                        last_frame_time.set(frame_time);

                        let adjustment = &*self_.adjustment_timestamp;
                        let new_time = adjustment.value() + f64::from(time_passed);
                        if new_time >= adjustment.upper() {
                            adjustment.set_value(adjustment.upper());
                            self_.tick_callback.borrow_mut().take();
                            button.set_icon_name("media-playback-start-symbolic");
                            glib::Continue(false)
                        } else {
                            adjustment.set_value(new_time);
                            glib::Continue(true)
                        }
                    }));
                }
            }
        }
    }

    fn set_up_picture_drop_target(picture: gtk::Picture) {
        let drop_target = gtk::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
        picture.add_controller(&drop_target);
        drop_target.connect_drop(move |_, data, _, _| {
            if let Ok(file) = data.get::<gio::File>() {
                let old_paintable = picture.paintable();

                picture.set_file(Some(&file));

                if picture.paintable().is_none() {
                    picture.set_paintable(old_paintable.as_ref());
                    return false;
                }

                return true;
            }

            false
        });
    }
}

glib::wrapper! {
    pub struct ApplicationWindow(ObjectSubclass<imp::ApplicationWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

#[gtk::template_callbacks]
impl ApplicationWindow {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn open(&self, file: gio::File) -> anyhow::Result<()> {
        imp::ApplicationWindow::from_instance(self).open(file)
    }

    #[template_callback]
    fn on_open_clicked(self) {
        let file_chooser = gtk::FileChooserNative::builder()
            .transient_for(&self)
            .action(gtk::FileChooserAction::Open)
            .title("Open a .qua map")
            .transient_for(&self)
            .modal(true)
            .build();

        glib::MainContext::default().spawn_local(async move {
            if file_chooser.run_future().await != gtk::ResponseType::Accept {
                return;
            }

            let file = file_chooser.file().unwrap();
            if let Err(err) = self.open(file).with_context(|| "couldn't load the map") {
                info!("{:?}", err);
            }
        });
    }
}
