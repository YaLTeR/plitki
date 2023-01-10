use std::cell::RefCell;
use std::rc::Rc;

use glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::audio::AudioEngine;

#[derive(Debug, Clone, glib::SharedBoxed)]
#[shared_boxed_type(name = "BoxedAudioEngine")]
pub(crate) struct BoxedAudioEngine(Rc<AudioEngine>);

mod imp {
    use std::cell::Cell;
    use std::io::Cursor;

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::{gdk, gdk_pixbuf, CompositeTemplate};
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;
    use plitki_core::map::Map;
    use plitki_core::scroll::ScrollSpeed;
    use plitki_core::state::{Event, EventKind, GameState, Hit};
    use plitki_core::timing::{
        GameTimestamp, GameTimestampDifference, MapTimestampDifference, Timestamp,
    };
    use plitki_gtk::playfield::Playfield;
    use plitki_gtk::skin::{LaneSkin, Skin};
    use plitki_gtk::state::State;

    use super::*;
    use crate::accuracy::Accuracy;
    use crate::background::Background;
    use crate::combo::Combo;
    use crate::hit_error::HitError;
    use crate::hit_light::HitLight;
    use crate::judgement::Judgement;
    use crate::key_binding_indicator::KeyBindingIndicator;
    use crate::statistics::Statistics;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/plitki-gnome/window.ui")]
    pub struct Window {
        #[template_child]
        toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        stack: TemplateChild<gtk::Stack>,
        #[template_child]
        playfield: TemplateChild<Playfield>,
        #[template_child]
        accuracy: TemplateChild<Accuracy>,
        #[template_child]
        combo: TemplateChild<Combo>,
        #[template_child]
        hit_error: TemplateChild<HitError>,
        #[template_child]
        judgement: TemplateChild<Judgement>,
        #[template_child]
        pref_window: TemplateChild<adw::PreferencesWindow>,
        #[template_child]
        gameplay_header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        gameplay_window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        map_background: TemplateChild<Background>,
        #[template_child]
        global_offset_adjustment: TemplateChild<gtk::Adjustment>,
        #[template_child]
        skin_combo_row: TemplateChild<adw::ComboRow>,

        statistics: RefCell<Statistics>,

        audio: OnceCell<Rc<AudioEngine>>,

        offset_toast: RefCell<Option<adw::Toast>>,
        scroll_speed_toast: RefCell<Option<adw::Toast>>,

        // GTK key events have key repeat, so filter that out manually using this array.
        is_lane_pressed: RefCell<[bool; 7]>,

        // Timestamp, in milliseconds, of the last mouse movement.
        last_mouse_movement_timestamp: Cell<i64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "PlitkiWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();

            let skin_model = gio::ListStore::new(Skin::static_type());
            skin_model.extend_from_slice(&[
                create_skin("Bars", "/plitki-gnome/skin/bars"),
                create_skin("Arrows", "/plitki-gnome/skin/arrows"),
                create_skin("Circles", "/plitki-gnome/skin/circles"),
            ]);
            self.skin_combo_row
                .set_expression(Some(gtk::PropertyExpression::new(
                    Skin::static_type(),
                    None::<&gtk::Expression>,
                    "name",
                )));
            self.skin_combo_row.set_model(Some(&skin_model));

            self.pref_window.set_transient_for(Some(&*obj));

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

            // Set up key bindings.
            let controller = gtk::EventControllerKey::new();
            controller.connect_key_pressed(clone!(
                @weak obj => @default-return gtk::Inhibit(false), move |_, key, _, modifier| {
                    obj.imp().on_key_pressed(key, modifier)
                }
            ));
            controller.connect_key_released(clone!(@weak obj => move |_, key, _, _| {
                obj.imp().on_key_released(key);
            }));
            obj.add_controller(&controller);

            // Set up mouse movement tracking.
            let controller = gtk::EventControllerMotion::new();
            controller.connect_enter(clone!(@weak self as imp => move |controller, _, _| {
                let time = controller.current_event_time();
                if time != 0 {
                    imp.on_mouse_moved(time.into());
                }
            }));
            controller.connect_motion(clone!(@weak self as imp => move |controller, _, _| {
                let time = controller.current_event_time();
                if time != 0 {
                    imp.on_mouse_moved(time.into());
                }
            }));
            obj.add_controller(&controller);

            // Set up playfield scrolling.
            obj.add_tick_callback(move |obj, clock| {
                obj.imp().on_tick_callback(clock);
                glib::Continue(true)
            });
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoxed::builder::<BoxedAudioEngine>("audio-engine")
                        .write_only()
                        .construct_only()
                        .build(),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
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

    #[gtk::template_callbacks]
    impl Window {
        #[template_callback]
        fn open_preferences(&self) {
            self.pref_window.present();
        }

        #[template_callback]
        fn on_global_offset_changed(&self) {
            if let Some(state) = self.playfield.state() {
                let mut game_state = state.game_state_mut();

                game_state.timestamp_converter.global_offset = GameTimestampDifference::from_millis(
                    self.global_offset_adjustment.value() as i32,
                );
                self.playfield.queue_allocate();
            }
        }

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

            let map_dir = file.parent();

            // Load the audio file.
            let track = if let Some(name) = &map.audio_file {
                if let Some(dir) = &map_dir {
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

            let mut game_state =
                match GameState::new(map, GameTimestampDifference::from_millis(164)) {
                    Ok(x) => x,
                    Err(err) => {
                        warn!("map is invalid: {err:?}");
                        return;
                    }
                };

            let map = &game_state.immutable.map;
            let title = match (&map.song_artist, &map.song_title) {
                (None, None) => "Plitki".to_owned(),
                (None, Some(title)) => title.clone(),
                (Some(artist), None) => artist.clone(),
                (Some(artist), Some(title)) => format!("{} - {}", artist, title),
            };
            self.gameplay_window_title.set_title(&title);

            self.map_background.set_paintable(
                map.background_file
                    .as_deref()
                    .zip(map_dir)
                    .map(|(name, dir)| dir.child(name))
                    .and_then(|file| gdk::Texture::from_file(&file).ok()),
            );

            let lane_count = map.lane_count();

            self.gameplay_window_title
                .set_subtitle(map.difficulty_name.as_deref().unwrap_or(""));

            game_state.timestamp_converter.global_offset =
                GameTimestampDifference::from_millis(self.global_offset_adjustment.value() as i32);

            let state = State::new(game_state);
            self.playfield.set_state(Some(state));

            for (i, lane) in self.playfield.lanes().unwrap().iter().enumerate() {
                lane.set_below_hit_pos_widget(Some(HitLight::new().upcast()));

                let accelerator = match lane_count {
                    4 => ["s", "d", "l", "semicolon"][i],
                    7 => ["a", "s", "d", "space", "l", "semicolon", "apostrophe"][i],
                    _ => unimplemented!(),
                };
                let indicator = KeyBindingIndicator::new(Some(accelerator.to_string()));
                lane.set_above_hit_pos_widget(Some(indicator.upcast()));
            }

            // Fire indicators in an idle when they are mapped so the animation isn't skipped.
            glib::idle_add_local_once(clone!(@weak self as imp => move || {
                if let Some(lanes) = imp.playfield.lanes() {
                    for lane in &*lanes {
                        let indicator: KeyBindingIndicator =
                            lane.above_hit_pos_widget().unwrap().downcast().unwrap();
                        indicator.fire();
                    }
                };
            }));

            self.stack.set_visible_child_name("gameplay");

            let mut is_lane_pressed = self.is_lane_pressed.borrow_mut();
            *is_lane_pressed = [false; 7];

            self.statistics.replace(Statistics::new());
            self.accuracy
                .set_accuracy(self.statistics.borrow().accuracy());
            self.combo.set_combo(0);

            // Start the audio.
            let engine = self.audio.get().unwrap();
            if let Some(track) = track {
                engine.play_track(track);
            } else {
                engine.play_track(rodio::source::Zero::<f32>::new(2, 44100));
            }
        }

        fn process_event(&self, lane: usize, event: Event) {
            match event.kind {
                EventKind::Miss => {
                    self.combo.set_combo(0);
                }
                EventKind::Hit(Hit { difference, .. }) => {
                    if difference.into_milli_hundredths().abs() / 100 <= 127 {
                        self.combo.set_combo(self.combo.combo() + 1);
                    } else {
                        self.combo.set_combo(0);
                    }
                }
            }

            self.playfield.update_object_state(lane, event.object_index);

            let mut statistics = self.statistics.borrow_mut();
            statistics.process_event(event.kind);
            self.accuracy.set_accuracy(statistics.accuracy());
        }

        fn hit_light_for_lane(&self, lane: usize) -> HitLight {
            self.playfield.lanes().unwrap()[lane]
                .below_hit_pos_widget()
                .unwrap()
                .downcast()
                .unwrap()
        }

        #[instrument(skip_all)]
        fn update_state(&self, timestamp: GameTimestamp) {
            let Some(lane_count) = self.playfield.state().map(|s| s.lane_count()) else { return };

            for lane in 0..lane_count {
                let hit_light = self.hit_light_for_lane(lane);

                let Some(state) = self.playfield.state() else { return };
                while let Some(event) = {
                    let mut game_state = state.game_state_mut();
                    game_state.update_lane(lane, timestamp)
                } {
                    self.process_event(lane, event);

                    let css_class = hit_light_css_class(event.kind);
                    hit_light.set_css_classes(&[css_class]);
                    hit_light.fire();
                }

                // If we're holding a long note, update its length.
                let first_active_object = state.game_state().first_active_object(lane);
                if let Some(index) = first_active_object {
                    self.playfield.update_object_state(lane, index);
                }
            }
        }

        fn update_mouse_inactivity(&self, clock: &gdk::FrameClock) {
            let obj = self.obj();

            let current_time = clock.frame_time() / 1000;
            let last_mouse_movement_time: i64 = self.last_mouse_movement_timestamp.get();
            let time_since_last_mouse_movement =
                current_time.saturating_sub(last_mouse_movement_time);

            if self.stack.visible_child_name().as_deref() != Some("gameplay")
                || self.pref_window.is_visible()
                || last_mouse_movement_time == 0
            {
                self.gameplay_header_bar.set_opacity(1.);
                obj.set_cursor_from_name(None);

                // Reset the timestamp so that we don't get instant fade-out if, for example, the
                // user did not move the cursor to close the preferences window, letting the idle
                // time build up.
                self.last_mouse_movement_timestamp.set(current_time);
                return;
            }

            let opacity = (3000 - time_since_last_mouse_movement.clamp(1000, 3000)) as f64 / 2000.;
            self.gameplay_header_bar.set_opacity(opacity);

            if opacity == 0. {
                // If the cursor is already hidden, avoid a costly creation within.
                if obj.cursor().is_none() {
                    obj.set_cursor_from_name(Some("none"));
                }
            } else {
                obj.set_cursor_from_name(None);
            }
        }

        #[instrument(skip_all)]
        fn on_tick_callback(&self, clock: &gdk::FrameClock) {
            self.update_mouse_inactivity(clock);

            let game_timestamp = self.game_timestamp();

            self.playfield.set_game_timestamp(game_timestamp);
            self.update_state(game_timestamp);

            if let Some(state) = self.playfield.state() {
                let game_state = state.game_state();

                self.hit_error.update(
                    game_timestamp,
                    game_state.last_hits.iter().copied().collect(),
                );

                self.judgement
                    .update(game_timestamp, game_state.last_hits.iter().next().copied());
            }
        }

        fn game_timestamp(&self) -> GameTimestamp {
            let audio_time_passed = self.audio.get().unwrap().track_time();
            GameTimestamp(Timestamp::try_from(audio_time_passed).unwrap())
        }

        fn show_local_offset_toast(&self) {
            let Some(state) = self.playfield.state() else { return };
            let offset = state
                .game_state()
                .timestamp_converter
                .local_offset
                .as_millis();

            let title =
                format!("Map offset set to <span font_features='tnum=1'>{offset}</span> ms");

            let mut toast = self.offset_toast.borrow_mut();
            if let Some(toast) = &*toast {
                toast.set_title(&title);
            } else {
                let obj = self.obj();
                let new_toast = adw::Toast::new(&title);
                new_toast.connect_dismissed(clone!(@weak obj => move |_| {
                    obj.imp().offset_toast.replace(None);
                }));
                self.toast_overlay.add_toast(&new_toast);
                *toast = Some(new_toast);
            }
            drop(toast);

            let scroll_speed_toast = self.scroll_speed_toast.borrow();
            if let Some(toast) = scroll_speed_toast.clone() {
                drop(scroll_speed_toast);
                toast.dismiss();
            }
        }

        fn show_scroll_speed_toast(&self) {
            let title = format!(
                "Scroll speed set to <span font_features='tnum=1'>{}</span>",
                self.playfield.scroll_speed().0
            );

            let mut toast = self.scroll_speed_toast.borrow_mut();
            if let Some(toast) = &*toast {
                toast.set_title(&title);
            } else {
                let obj = self.obj();
                let new_toast = adw::Toast::new(&title);
                new_toast.connect_dismissed(clone!(@weak obj => move |_| {
                    obj.imp().scroll_speed_toast.replace(None);
                }));
                self.toast_overlay.add_toast(&new_toast);
                *toast = Some(new_toast);
            }
            drop(toast);

            let offset_toast = self.offset_toast.borrow();
            if let Some(toast) = offset_toast.clone() {
                drop(offset_toast);
                toast.dismiss();
            }
        }

        fn maybe_adjust_local_offset(&self, key: gdk::Key, modifier: gdk::ModifierType) -> bool {
            let Some(state) = self.playfield.state() else { return false };
            let mut game_state = state.game_state_mut();

            let diff = MapTimestampDifference::from_millis(
                if modifier.contains(gdk::ModifierType::CONTROL_MASK) {
                    1
                } else {
                    5
                },
            );

            match key {
                gdk::Key::plus | gdk::Key::equal => {
                    game_state.timestamp_converter.local_offset = game_state
                        .timestamp_converter
                        .local_offset
                        .saturating_add(diff);
                    true
                }
                gdk::Key::minus => {
                    game_state.timestamp_converter.local_offset = game_state
                        .timestamp_converter
                        .local_offset
                        .saturating_sub(diff);
                    true
                }
                _ => false,
            }
        }

        fn maybe_adjust_scroll_speed(&self, key: gdk::Key, modifier: gdk::ModifierType) -> bool {
            let diff = if modifier.contains(gdk::ModifierType::CONTROL_MASK) {
                1
            } else {
                5
            };

            let scroll_speed = self.playfield.scroll_speed();
            match key {
                gdk::Key::F4 => {
                    self.playfield
                        .set_scroll_speed(ScrollSpeed(scroll_speed.0.saturating_add(diff)));
                    true
                }
                gdk::Key::F3 => {
                    self.playfield
                        .set_scroll_speed(ScrollSpeed(scroll_speed.0.saturating_sub(diff).max(1)));
                    true
                }
                _ => false,
            }
        }

        fn lane_for_key(&self, key: gdk::Key) -> Option<usize> {
            let lane = match self
                .playfield
                .state()?
                .game_state()
                .immutable
                .lane_caches
                .len()
            {
                4 => match key {
                    gdk::Key::s => 0,
                    gdk::Key::d => 1,
                    gdk::Key::l => 2,
                    gdk::Key::semicolon => 3,
                    _ => return None,
                },
                7 => match key {
                    gdk::Key::a => 0,
                    gdk::Key::s => 1,
                    gdk::Key::d => 2,
                    gdk::Key::space => 3,
                    gdk::Key::l => 4,
                    gdk::Key::semicolon => 5,
                    gdk::Key::apostrophe => 6,
                    _ => return None,
                },
                _ => return None,
            };
            Some(lane)
        }

        fn on_key_pressed(&self, key: gdk::Key, modifier: gdk::ModifierType) -> gtk::Inhibit {
            // Handle local offset keys.
            if self.maybe_adjust_local_offset(key, modifier) {
                self.show_local_offset_toast();
                return gtk::Inhibit(true);
            }

            // Handle scroll speed keys.
            if self.maybe_adjust_scroll_speed(key, modifier) {
                self.show_scroll_speed_toast();
                return gtk::Inhibit(true);
            }

            // Handle gameplay keys.
            let lane = match self.lane_for_key(key) {
                Some(x) => x,
                None => return gtk::Inhibit(false),
            };

            let mut is_lane_pressed = self.is_lane_pressed.borrow_mut();
            if is_lane_pressed[lane] {
                return gtk::Inhibit(false);
            }
            is_lane_pressed[lane] = true;

            let timestamp = self.game_timestamp();
            self.update_state(timestamp);

            let hit_light = self.hit_light_for_lane(lane);

            let Some(state) = self.playfield.state() else { return gtk::Inhibit(false) };

            if let Some(event) = {
                let mut game_state = state.game_state_mut();
                game_state.key_press(lane, timestamp)
            } {
                self.process_event(lane, event);

                let css_class = hit_light_css_class(event.kind);
                hit_light.set_css_classes(&[css_class]);
                hit_light.fire();
            };

            gtk::Inhibit(true)
        }

        fn on_key_released(&self, key: gdk::Key) {
            let Some(lane) = self.lane_for_key(key) else { return };

            let mut is_lane_pressed = self.is_lane_pressed.borrow_mut();
            if !is_lane_pressed[lane] {
                return;
            }
            is_lane_pressed[lane] = false;

            let timestamp = self.game_timestamp();
            self.update_state(timestamp);

            let hit_light = self.hit_light_for_lane(lane);

            let Some(state) = self.playfield.state() else { return };

            if let Some(event) = {
                let mut game_state = state.game_state_mut();
                game_state.key_release(lane, timestamp)
            } {
                self.process_event(lane, event);

                let css_class = hit_light_css_class(event.kind);
                hit_light.set_css_classes(&[css_class]);
                hit_light.fire();
            };
        }

        fn on_mouse_moved(&self, timestamp: i64) {
            self.last_mouse_movement_timestamp.set(timestamp);
        }
    }

    fn create_skin(name: &str, path: &str) -> Skin {
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

        let skin = Skin::new(Some(name.to_owned()));
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

    fn hit_light_css_class(event_kind: EventKind) -> &'static str {
        match event_kind {
            EventKind::Miss => "judge-miss",
            EventKind::Hit(Hit { difference, .. }) => {
                match difference.into_milli_hundredths().abs() / 100 {
                    0..=18 => "judge-marv",
                    19..=43 => "judge-perf",
                    44..=76 => "judge-great",
                    77..=106 => "judge-good",
                    107..=127 => "judge-okay",
                    128..=164 => "judge-miss",
                    _ => "",
                }
            }
        }
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
        glib::Object::builder()
            .property("application", app)
            .property("audio-engine", &BoxedAudioEngine(audio))
            .build()
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
