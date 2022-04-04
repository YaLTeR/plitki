use crate::audio::AudioEngine;
use std::cell::RefCell;
use std::rc::Rc;

use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use log::warn;

#[derive(Debug, Clone, glib::SharedBoxed)]
#[shared_boxed_type(name = "BoxedAudioEngine")]
pub(crate) struct BoxedAudioEngine(Rc<AudioEngine>);

mod imp {
    use std::io::Cursor;

    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gdk, gdk_pixbuf, CompositeTemplate};
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;
    use plitki_core::map::Map;
    use plitki_core::scroll::ScrollSpeed;
    use plitki_core::timing::{GameTimestamp, Timestamp};
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

        playfield: RefCell<Option<Playfield>>,

        audio: OnceCell<Rc<AudioEngine>>,
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

            // Set up the drop target.
            let drop_target = gtk::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
            drop_target.connect_drop(
                clone!(@weak obj => @default-return false, move |_, data, _, _| {
                    if let Ok(file) = data.get::<gio::File>() {
                        obj.open_file(file);
                        return true;
                    }

                    false
                }),
            );
            self.stack.add_controller(&drop_target);

            // Set up playfield scrolling.
            obj.add_tick_callback(move |obj, _clock| {
                obj.imp().on_tick_callback();
                glib::Continue(true)
            });
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecBoxed::new(
                    "audio-engine",
                    "",
                    "",
                    BoxedAudioEngine::static_type(),
                    glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                )]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "audio-engine" => {
                    let value = value.get::<BoxedAudioEngine>().unwrap().0;
                    self.audio.set(value).unwrap();
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}

    impl Window {
        pub async fn open_file(&self, file: &gio::File) {
            // Load the .qua.
            let (contents, _) = match file.load_contents_future().await {
                Ok(x) => x,
                Err(err) => {
                    warn!("error reading map file: {err:?}");
                    return;
                }
            };

            let qua = match plitki_map_qua::from_reader(&contents[..]) {
                Ok(x) => x,
                Err(err) => {
                    warn!("could not open file as .qua: {err:?}");
                    return;
                }
            };

            let map: Map = qua.try_into().unwrap();

            // Load the audio file.
            let track = if let Some(name) = &map.audio_file {
                if let Some(dir) = file.parent() {
                    let file = dir.child(name);
                    match file.load_contents_future().await {
                        Ok((contents, _)) => {
                            let contents = Cursor::new(contents);
                            match rodio::Decoder::new(contents) {
                                Ok(x) => Some(x),
                                Err(err) => {
                                    warn!("error decoding audio file: {err:?}");
                                    None
                                }
                            }
                        }
                        Err(err) => {
                            warn!("error reading audio file: {err:?}");
                            None
                        }
                    }
                } else {
                    warn!(".qua file has no parent dir");
                    None
                }
            } else {
                warn!("map has no audio file set");
                None
            };

            // Create the playfield.
            let playfield = Playfield::new(map, &create_skin("/plitki-gnome/skin/arrows"));

            playfield.set_halign(gtk::Align::Center);
            playfield.set_valign(gtk::Align::End);
            playfield.set_downscroll(true);
            playfield.set_scroll_speed(ScrollSpeed(60));

            self.scrolled_window.set_child(Some(&playfield));

            self.playfield.replace(Some(playfield));

            self.stack.set_visible_child_name("content");

            // Start the audio.
            let engine = self.audio.get().unwrap();
            if let Some(track) = track {
                engine.play_track(track);
            } else {
                engine.play_track(rodio::source::Zero::<f32>::new(2, 44100));
            }
        }

        fn on_tick_callback(&self) {
            let audio_time_passed = self.audio.get().unwrap().track_time();
            let game_timestamp = GameTimestamp(Timestamp::try_from(audio_time_passed).unwrap());

            let playfield = self.playfield.borrow();
            if let Some(playfield) = &*playfield {
                playfield.set_game_timestamp(game_timestamp);
            }
        }
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
    pub fn new(app: &impl IsA<gtk::Application>, audio: Rc<AudioEngine>) -> Self {
        glib::Object::new(&[
            ("application", app),
            ("audio-engine", &BoxedAudioEngine(audio)),
        ])
        .unwrap()
    }

    pub fn open_file(&self, file: gio::File) {
        glib::MainContext::default().spawn_local(
            clone!(@strong self as obj => async move { obj.imp().open_file(&file).await; }),
        );
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
                                obj.open_file(file);
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
