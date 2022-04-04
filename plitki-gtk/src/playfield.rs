use std::cell::Ref;

use crate::skin::Skin;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use plitki_core::map::Map;
use plitki_core::scroll::ScrollSpeed;
use plitki_core::state::GameState;

#[derive(Debug, Clone, glib::Boxed)]
#[boxed_type(name = "BoxedMap")]
pub(crate) struct BoxedMap(Map);

mod imp {
    use std::cell::RefCell;

    use gtk::{graphene, gsk};
    use log::{debug, trace};
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;
    use plitki_core::scroll::{Position, ScrollSpeed};
    use plitki_core::state::ObjectCache;
    use plitki_core::timing::MapTimestamp;

    use super::*;
    use crate::long_note::LongNote;
    use crate::utils::{from_pixels_f64, to_pixels, to_pixels_f64};

    #[derive(Debug)]
    struct State {
        game: GameState,
        objects: Vec<Vec<gtk::Widget>>,
        timing_lines: Vec<gtk::Separator>,
        scroll_speed: ScrollSpeed,
        map_timestamp: MapTimestamp,
        map_position: Position,
        downscroll: bool,
    }

    impl State {
        fn new(game: GameState) -> Self {
            Self {
                game,
                objects: vec![],
                timing_lines: vec![],
                scroll_speed: ScrollSpeed(32),
                map_timestamp: MapTimestamp::zero(),
                map_position: Position::zero(),
                downscroll: false,
            }
        }
    }

    #[derive(Debug, Default)]
    pub struct Playfield {
        state: OnceCell<RefCell<State>>,
        skin: OnceCell<RefCell<Skin>>,
        hadjustment: RefCell<Option<gtk::Adjustment>>,
        vadjustment: RefCell<Option<gtk::Adjustment>>,
        hadjustment_signal_handler: RefCell<Option<glib::SignalHandlerId>>,
        vadjustment_signal_handler: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Playfield {
        const NAME: &'static str = "PlitkiPlayfield";
        type Type = super::Playfield;
        type ParentType = gtk::Widget;
        type Interfaces = (gtk::Scrollable,);

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("plitki-playfield");
        }
    }

    impl ObjectImpl for Playfield {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.set_overflow(gtk::Overflow::Hidden);

            self.rebuild(obj);
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoxed::new(
                        "map",
                        "map",
                        "map",
                        BoxedMap::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecUInt::new(
                        "scroll-speed",
                        "scroll-speed",
                        "scroll-speed",
                        0,
                        255,
                        32,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpecInt::new(
                        "map-timestamp",
                        "map-timestamp",
                        "map-timestamp",
                        -(2i32.pow(30)),
                        2i32.pow(30) - 1,
                        0,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpecInt64::new(
                        "map-position",
                        "map-position",
                        "map-position",
                        -(2i64.pow(32 + 24)),
                        2i64.pow(32 + 24) - 1,
                        0,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpecBoolean::new(
                        "downscroll",
                        "downscroll",
                        "downscroll",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpecBoxed::new(
                        "skin",
                        "skin",
                        "skin",
                        Skin::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT,
                    ),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("hadjustment"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("vadjustment"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("hscroll-policy"),
                    glib::ParamSpecOverride::for_interface::<gtk::Scrollable>("vscroll-policy"),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "map" => {
                    let map = value.get::<BoxedMap>().expect("wrong property type").0;
                    let state = State::new(GameState::new(map).expect("invalid map"));
                    self.state
                        .set(RefCell::new(state))
                        .expect("property set more than once");
                }
                "scroll-speed" => {
                    let speed = value.get::<u32>().expect("wrong property type");
                    let speed: u8 = speed.try_into().expect("value outside u8 range");
                    self.set_scroll_speed(ScrollSpeed(speed));
                }
                "map-timestamp" => {
                    let timestamp = value.get::<i32>().expect("wrong property type");
                    let timestamp = MapTimestamp::from_milli_hundredths(timestamp);
                    let mut state = self.state.get().expect("map needs to be set").borrow_mut();
                    state.map_timestamp = timestamp;

                    let position = state.game.position_at_time(timestamp);
                    if state.map_position != position {
                        state.map_position = position;
                        drop(state);
                        obj.notify("map-position");
                        obj.queue_allocate();
                    }
                }
                "map-position" => {
                    let position = value.get::<i64>().expect("wrong property type");
                    let position = Position::new(position);
                    let mut state = self.state.get().expect("map needs to be set").borrow_mut();

                    if state.map_position != position {
                        state.map_position = position;
                        obj.queue_allocate();
                    }
                }
                "hadjustment" => {
                    let value = value.get::<Option<gtk::Adjustment>>().unwrap();
                    self.set_hadjustment(obj, value);
                }
                "vadjustment" => {
                    let value = value.get::<Option<gtk::Adjustment>>().unwrap();
                    self.set_vadjustment(obj, value);
                }
                "downscroll" => {
                    let value = value.get::<bool>().unwrap();
                    self.set_downscroll(value);
                }
                "skin" => {
                    let value = value.get::<Skin>().unwrap();
                    match self.skin.get() {
                        Some(skin) => {
                            *skin.borrow_mut() = value;
                            self.rebuild(obj);
                        }
                        None => self.skin.set(RefCell::new(value)).unwrap(),
                    }
                }
                "hscroll-policy" => {}
                "vscroll-policy" => {}
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "scroll-speed" => {
                    let state = self.state.get().expect("map needs to be set").borrow();
                    let speed: u32 = state.scroll_speed.0.into();
                    speed.to_value()
                }
                "map-timestamp" => {
                    let state = self.state.get().expect("map needs to be set").borrow();
                    state.map_timestamp.into_milli_hundredths().to_value()
                }
                "map-position" => {
                    let state = self.state.get().expect("map needs to be set").borrow();
                    let position: i64 = state.map_position.into();
                    position.to_value()
                }
                "downscroll" => {
                    let state = self.state.get().expect("map needs to be set").borrow();
                    state.downscroll.to_value()
                }
                "skin" => self.skin.get().unwrap().borrow().to_value(),
                "hadjustment" => self.hadjustment.borrow().to_value(),
                "vadjustment" => self.vadjustment.borrow().to_value(),
                "hscroll-policy" => gtk::ScrollablePolicy::Natural.to_value(),
                "vscroll-policy" => gtk::ScrollablePolicy::Natural.to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Playfield {
        fn request_mode(&self, _widget: &Self::Type) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(
            &self,
            widget: &Self::Type,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            trace!("Playfield::measure({}, {})", orientation, for_size);

            // We only support can-shrink paintables which can always go down to zero, so our min
            // size is always zero.
            match orientation {
                gtk::Orientation::Horizontal => {
                    if for_size == -1 {
                        let state = self.state().borrow();
                        let lane_count = state.objects.len() as i32;

                        // All lanes must have the same width, so let's base the natural size on the
                        // first object width we can find.
                        let object = state
                            .objects
                            .iter()
                            .flat_map(|lane| lane.iter())
                            .next()
                            .unwrap();
                        let object_nat = object.measure(gtk::Orientation::Horizontal, -1).1;

                        let nat = object_nat * lane_count;
                        trace!("returning for height = {}: nat width = {}", for_size, nat);
                        (0, nat, -1, -1)
                    } else {
                        let height_to_fit = for_size;

                        // Natural width is the biggest width that fits the given height.

                        // Compute the aspect ratio of the long note, then estimate the starting
                        // width from there.
                        let nat_width = self.measure(widget, gtk::Orientation::Horizontal, -1).1;
                        let nat_height = self
                            .measure(widget, gtk::Orientation::Vertical, nat_width)
                            .1;
                        let starting_width =
                            (nat_width as f32 / nat_height as f32 * height_to_fit as f32) as i32;

                        // The real width should be somewhere close.
                        let height = self
                            .measure(widget, gtk::Orientation::Vertical, starting_width)
                            .1;
                        if height <= height_to_fit {
                            // We're under, search up from here.
                            for width in starting_width + 1.. {
                                let height =
                                    self.measure(widget, gtk::Orientation::Vertical, width).1;
                                if height > height_to_fit {
                                    // We went over, so take the previous width.
                                    let nat = width - 1;
                                    trace!(
                                        "returning for height = {}: nat width = {}",
                                        for_size,
                                        nat
                                    );
                                    return (0, nat, -1, -1);
                                }
                            }
                        } else {
                            // We're over, search down from here.
                            for width in (0..starting_width).rev() {
                                let height =
                                    self.measure(widget, gtk::Orientation::Vertical, width).1;
                                if height <= height_to_fit {
                                    let nat = width;
                                    trace!(
                                        "returning for height = {}: nat width = {}",
                                        for_size,
                                        nat
                                    );
                                    return (0, nat, -1, -1);
                                }
                            }
                        }

                        unreachable!()
                    }
                }
                gtk::Orientation::Vertical => {
                    if for_size == -1 {
                        let width = self.measure(widget, gtk::Orientation::Horizontal, -1).1;
                        self.measure(widget, gtk::Orientation::Vertical, width)
                    } else {
                        let state = self.state().borrow();
                        let lane_count = state.objects.len() as i32;
                        let lane_width = for_size / lane_count;
                        let width = lane_width * lane_count;

                        let min_position = state.game.min_position().unwrap();

                        let (regular_y, nat_regular) =
                            if let Some(max_regular) = state.game.max_regular() {
                                // All regular notes are the same so just take the first one.
                                let regular_widget = state
                                    .objects
                                    .iter()
                                    .flat_map(|lane| lane.iter())
                                    .find(|widget| widget.is::<gtk::Picture>())
                                    .unwrap();
                                let nat_regular = regular_widget
                                    .measure(gtk::Orientation::Vertical, lane_width)
                                    .1;
                                let regular_y = to_pixels(
                                    (max_regular.position - min_position) * state.scroll_speed,
                                    lane_width,
                                    lane_count,
                                );

                                (nat_regular, regular_y)
                            } else {
                                (0, 0)
                            };

                        let (long_note_y, nat_long_note) =
                            if let Some(max_long_note) = state.game.max_long_note() {
                                // We need the right long note though.
                                let max_long_note_widget = state
                                    .objects
                                    .iter()
                                    .zip(&state.game.immutable.lane_caches)
                                    .flat_map(|(widget_lane, lane)| {
                                        widget_lane.iter().zip(&lane.object_caches)
                                    })
                                    .find_map(|(widget, cache)| {
                                        if let ObjectCache::LongNote(cache) = cache {
                                            if *cache == max_long_note {
                                                Some(widget)
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap();
                                let nat_long_note = max_long_note_widget
                                    .measure(gtk::Orientation::Vertical, lane_width)
                                    .1;
                                let long_note_y = to_pixels(
                                    (max_long_note.start_position.min(max_long_note.end_position)
                                        - min_position)
                                        * state.scroll_speed,
                                    lane_width,
                                    lane_count,
                                );

                                (long_note_y, nat_long_note)
                            } else {
                                (0, 0)
                            };

                        let (timing_line_y, nat_timing_line) =
                            if let Some(line) = state.game.max_timing_line() {
                                if width == 0 {
                                    // Separators do have a minimum width.
                                    (0, 0)
                                } else {
                                    // All timing lines are the same so just take the first one.
                                    let widget = &state.timing_lines[0];
                                    let nat_timing_line =
                                        widget.measure(gtk::Orientation::Vertical, width).1;
                                    let timing_line_y = to_pixels(
                                        (line.position - min_position) * state.scroll_speed,
                                        lane_width,
                                        lane_count,
                                    );

                                    (nat_timing_line, timing_line_y)
                                }
                            } else {
                                (0, 0)
                            };

                        let nat = (regular_y + nat_regular)
                            .max(long_note_y + nat_long_note)
                            .max(timing_line_y + nat_timing_line);
                        trace!("returning for height = {}: nat width = {}", for_size, nat);
                        (0, nat, -1, -1)
                    }
                }
                _ => unimplemented!(),
            }
        }

        fn size_allocate(&self, widget: &Self::Type, width: i32, height: i32, _baseline: i32) {
            trace!("Playfield::size_allocate({}, {})", width, height);

            let _ = self.hadjustment.borrow().as_ref().unwrap().freeze_notify();
            let _ = self.vadjustment.borrow().as_ref().unwrap().freeze_notify();

            self.configure_adjustments(widget);

            let state = self.state().borrow();
            let lane_count: i32 = state.objects.len().try_into().unwrap();
            let lane_width = width / lane_count;
            let width = lane_width * lane_count;

            let full_height = widget.measure(gtk::Orientation::Vertical, width).1;

            let vadjustment = self.vadjustment.borrow();
            let vadjustment = vadjustment.as_ref().unwrap();
            let base_y = vadjustment.value() as i32;

            let first_position = state.game.min_position().unwrap();

            for (line, widget) in state
                .game
                .immutable
                .timing_lines
                .iter()
                .zip(&state.timing_lines)
            {
                if line.position < first_position {
                    widget.set_child_visible(false);
                    continue;
                }

                if width == 0 {
                    // Separators do have a minimum width.
                    widget.set_child_visible(false);
                    continue;
                }
                widget.set_child_visible(true);

                let difference = line.position - first_position;
                let mut y = to_pixels(difference * state.scroll_speed, lane_width, lane_count);
                let height = widget.measure(gtk::Orientation::Vertical, width).1;
                if state.downscroll {
                    y = full_height - y - height;
                }
                y -= base_y;

                let mut transform = gsk::Transform::new()
                    .translate(&graphene::Point::new(0., y as f32))
                    .unwrap();
                if state.downscroll {
                    transform = transform
                        .translate(&graphene::Point::new(0., height as f32))
                        .unwrap_or_default()
                        .scale(1., -1.)
                        .unwrap();
                }

                widget.allocate(width, height, -1, Some(&transform));
            }

            for (l, (cache, widgets)) in state
                .game
                .immutable
                .lane_caches
                .iter()
                .zip(&state.objects)
                .enumerate()
            {
                let l: i32 = l.try_into().unwrap();
                let x = l * lane_width;

                for (cache, widget) in cache.object_caches.iter().zip(widgets) {
                    let position = cache.start_position();
                    let difference = position - first_position;
                    let mut y = to_pixels(difference * state.scroll_speed, lane_width, lane_count);
                    let height = widget.measure(gtk::Orientation::Vertical, lane_width).1;
                    if state.downscroll {
                        y = full_height - y - height;
                    }
                    y -= base_y;

                    let mut transform = gsk::Transform::new()
                        .translate(&graphene::Point::new(x as f32, y as f32))
                        .unwrap();
                    if state.downscroll {
                        transform = transform
                            .translate(&graphene::Point::new(0., height as f32))
                            .unwrap_or_default()
                            .scale(1., -1.)
                            .unwrap();
                    }

                    widget.allocate(lane_width, height, -1, Some(&transform));
                }
            }
        }
    }

    impl ScrollableImpl for Playfield {}

    impl Playfield {
        pub fn set_downscroll(&self, value: bool) {
            let mut state = self.state.get().expect("map needs to be set").borrow_mut();

            if state.downscroll != value {
                state.downscroll = value;
                self.instance().queue_allocate();
            }
        }

        pub fn set_scroll_speed(&self, value: ScrollSpeed) {
            let mut state = self.state.get().expect("map needs to be set").borrow_mut();

            if state.scroll_speed != value {
                state.scroll_speed = value;

                for (widget, cache) in state
                    .objects
                    .iter()
                    .zip(&state.game.immutable.lane_caches)
                    .flat_map(|(widget_lane, lane)| widget_lane.iter().zip(&lane.object_caches))
                {
                    if let ObjectCache::LongNote(cache) = cache {
                        let length =
                            (cache.end_position - cache.start_position) * state.scroll_speed;
                        widget.set_property("length", length.0);
                    }
                }

                self.instance().queue_resize();
            }
        }

        fn state(&self) -> &RefCell<State> {
            self.state
                .get()
                .expect("map property was not set during construction")
        }

        pub fn game_state(&self) -> Ref<GameState> {
            Ref::map(self.state().borrow(), |state| &state.game)
        }

        pub fn rebuild(&self, obj: &super::Playfield) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }

            let mut state = self.state().borrow_mut();
            let state = &mut *state;
            let map = &state.game.immutable.map;

            debug!(
                "{} - {} [{}]",
                map.song_artist.as_ref().unwrap(),
                map.song_title.as_ref().unwrap(),
                map.difficulty_name.as_ref().unwrap()
            );

            state.objects.clear();
            state.timing_lines.clear();

            for _ in &state.game.immutable.timing_lines {
                let widget = gtk::Separator::new(gtk::Orientation::Horizontal);
                widget.set_parent(obj);
                state.timing_lines.push(widget);
            }

            let lane_count = state.game.immutable.map.lanes.len();

            for (l, lane) in state.game.immutable.lane_caches.iter().enumerate() {
                let skin = self.skin.get().unwrap().borrow();
                let lane_skin = skin.store().get(lane_count, l);

                let mut widgets = Vec::new();

                for object in &lane.object_caches {
                    let widget: gtk::Widget = match object {
                        ObjectCache::Regular { .. } => {
                            gtk::Picture::for_paintable(&lane_skin.object).upcast()
                        }
                        ObjectCache::LongNote { .. } => LongNote::new(
                            &lane_skin.ln_head,
                            &lane_skin.ln_tail,
                            &lane_skin.ln_body,
                            map.lanes.len().try_into().unwrap(),
                            (object.end_position() - object.start_position()) * state.scroll_speed,
                        )
                        .upcast(),
                    };
                    widgets.push(widget);
                }

                // Set parent in reverse to get the right draw order.
                for widget in widgets.iter().rev() {
                    widget.set_parent(obj);
                }

                state.objects.push(widgets);
            }
        }

        fn configure_adjustments(&self, widget: &super::Playfield) {
            if let Some(hadjustment) = self.hadjustment.borrow().as_ref() {
                // We never actually scroll horizontally.
                let view_width: f64 = widget.width().into();
                hadjustment.configure(
                    hadjustment.value(),
                    0.,
                    view_width,
                    view_width * 0.1,
                    view_width * 0.9,
                    view_width,
                );
            }

            if let Some(vadjustment) = self.vadjustment.borrow().as_ref() {
                let state = self.state.get().unwrap().borrow();

                let view_width = widget.width();
                let lane_count: i32 = state.objects.len().try_into().unwrap();
                let lane_width = view_width / lane_count;

                let first_position = state.game.min_position().unwrap();

                let mut position = to_pixels_f64(
                    (state.map_position - first_position) * state.scroll_speed,
                    lane_width,
                    lane_count,
                );

                let nat_height = widget.measure(gtk::Orientation::Vertical, view_width).1;
                let view_height: f64 = widget.height().into();
                if state.downscroll {
                    position = nat_height as f64 - view_height - position;
                }

                // vadjustment.configure() can emit value-changed which needs mutable access to
                // state.
                drop(state);

                vadjustment.configure(
                    position,
                    0.,
                    nat_height.into(),
                    view_height * 0.1,
                    view_height * 0.9,
                    view_height,
                );
            };
        }

        fn set_hadjustment(&self, obj: &super::Playfield, adjustment: Option<gtk::Adjustment>) {
            if let Some(current) = self.hadjustment.take() {
                let handler = self.hadjustment_signal_handler.take().unwrap();
                current.disconnect(handler);
            }

            self.hadjustment.replace(adjustment.clone());

            if let Some(adjustment) = adjustment {
                self.configure_adjustments(obj);

                let handler = adjustment.connect_value_changed({
                    let obj = obj.downgrade();
                    move |_| {
                        let obj = obj.upgrade().unwrap();
                        obj.queue_allocate();
                    }
                });
                self.hadjustment_signal_handler.replace(Some(handler));
            }
        }

        fn set_vadjustment(&self, obj: &super::Playfield, adjustment: Option<gtk::Adjustment>) {
            if let Some(current) = self.vadjustment.take() {
                let handler = self.vadjustment_signal_handler.take().unwrap();
                current.disconnect(handler);
            }

            self.vadjustment.replace(adjustment.clone());

            if let Some(adjustment) = adjustment {
                self.configure_adjustments(obj);

                let handler = adjustment.connect_value_changed({
                    let obj = obj.downgrade();
                    move |adjustment| {
                        let obj = obj.upgrade().unwrap();
                        let self_ = Self::from_instance(&obj);
                        let mut state = self_.state.get().unwrap().borrow_mut();

                        // Convert the new value into map-position.
                        let view_width = obj.width();
                        let lane_count: i32 = state.objects.len().try_into().unwrap();
                        let lane_width = view_width / lane_count;
                        let mut pixels = adjustment.value();
                        if state.downscroll {
                            pixels = adjustment.upper() - adjustment.page_size() - pixels;
                        }
                        let length = from_pixels_f64(pixels, lane_width, lane_count);
                        let first_position = state.game.min_position().unwrap();
                        let position = if state.scroll_speed.0 > 0 {
                            let difference = length / state.scroll_speed;
                            first_position + difference
                        } else {
                            first_position
                        };
                        state.map_position = position;
                        drop(state);

                        obj.notify("map-position");
                        obj.queue_allocate();
                    }
                });
                self.vadjustment_signal_handler.replace(Some(handler));
            }
        }
    }
}

glib::wrapper! {
    pub struct Playfield(ObjectSubclass<imp::Playfield>)
        @extends gtk::Widget,
        @implements gtk::Scrollable;
}

impl Playfield {
    pub fn new(map: Map, skin: &Skin) -> Self {
        glib::Object::new(&[("map", &BoxedMap(map)), ("skin", skin)]).unwrap()
    }

    pub fn set_downscroll(&self, value: bool) {
        self.imp().set_downscroll(value);
    }

    pub fn set_scroll_speed(&self, value: ScrollSpeed) {
        self.imp().set_scroll_speed(value);
    }

    pub fn state(&self) -> Ref<GameState> {
        imp::Playfield::from_instance(self).game_state()
    }
}
