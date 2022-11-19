use std::cell::{Ref, RefMut};

use crate::skin::Skin;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use plitki_core::scroll::ScrollSpeed;
use plitki_core::state::GameState;
use plitki_core::timing::GameTimestamp;

#[derive(Debug, Clone, glib::Boxed)]
#[boxed_type(nullable, name = "BoxedGameState")]
pub(crate) struct BoxedGameState(GameState);

mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    use gtk::{graphene, gsk};
    use log::trace;
    use once_cell::sync::Lazy;
    use plitki_core::scroll::{Position, ScrollSpeed};
    use plitki_core::state::{ObjectCache, ObjectState};

    use super::*;
    use crate::long_note::LongNote;
    use crate::utils::to_pixels;

    #[derive(Debug)]
    struct State {
        game: GameState,
        objects: Vec<Vec<gtk::Widget>>,
        timing_lines: Vec<gtk::Separator>,
        map_position: Position,

        /// Cached min and nat widths for each lane.
        ///
        /// Refreshed in measure() and valid only in size_allocate().
        lane_sizes: Vec<(i32, i32)>,
    }

    #[derive(Debug)]
    pub struct Playfield {
        state: RefCell<Option<State>>,
        skin: RefCell<Option<Skin>>,
        scroll_speed: Cell<ScrollSpeed>,
        game_timestamp: Cell<GameTimestamp>,
        downscroll: Cell<bool>,
        lane_width: Cell<i32>,
        hit_position: Cell<i32>,
    }

    impl Default for Playfield {
        fn default() -> Self {
            Self {
                state: Default::default(),
                skin: Default::default(),
                scroll_speed: Cell::new(ScrollSpeed(30)),
                game_timestamp: Cell::new(GameTimestamp::zero()),
                downscroll: Default::default(),
                lane_width: Cell::new(0),
                hit_position: Cell::new(0),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Playfield {
        const NAME: &'static str = "PlitkiPlayfield";
        type Type = super::Playfield;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("plitki-playfield");
        }
    }

    impl ObjectImpl for Playfield {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().set_overflow(gtk::Overflow::Hidden);
        }

        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoxed::new(
                        "game-state",
                        "",
                        "",
                        BoxedGameState::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecBoxed::new(
                        "skin",
                        "",
                        "",
                        Skin::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecUInt::new(
                        "scroll-speed",
                        "",
                        "",
                        0,
                        255,
                        30,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecInt::new(
                        "game-timestamp",
                        "",
                        "",
                        -(2i32.pow(30)),
                        2i32.pow(30) - 1,
                        0,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecBoolean::new(
                        "downscroll",
                        "",
                        "",
                        false,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecInt::new(
                        "lane-width",
                        "",
                        "",
                        0,
                        10_000,
                        0,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecInt::new(
                        "hit-position",
                        "",
                        "",
                        -10_000,
                        10_000,
                        0,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "game-state" => {
                    let value = value.get::<Option<BoxedGameState>>().unwrap();
                    self.set_game_state(value.map(|x| x.0));
                }
                "skin" => self.set_skin(value.get().unwrap()),
                "scroll-speed" => {
                    let speed = value.get::<u32>().expect("wrong property type");
                    let speed: u8 = speed.try_into().expect("value outside u8 range");
                    self.set_scroll_speed(ScrollSpeed(speed));
                }
                "game-timestamp" => {
                    let timestamp = value.get::<i32>().expect("wrong property type");
                    let timestamp = GameTimestamp::from_milli_hundredths(timestamp);
                    self.set_game_timestamp(timestamp);
                }
                "downscroll" => self.set_downscroll(value.get().unwrap()),
                "lane-width" => self.set_lane_width(value.get().unwrap()),
                "hit-position" => self.set_hit_position(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "scroll-speed" => {
                    let speed: u32 = self.scroll_speed().0.into();
                    speed.to_value()
                }
                "game-timestamp" => self.game_timestamp.get().into_milli_hundredths().to_value(),
                "downscroll" => self.downscroll.get().to_value(),
                "skin" => self.skin.borrow().to_value(),
                "lane-width" => self.lane_width.get().to_value(),
                "hit-position" => self.hit_position.get().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Playfield {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            trace!("Playfield::measure({}, {})", orientation, for_size);

            match orientation {
                gtk::Orientation::Horizontal => {
                    let mut state = self.state.borrow_mut();
                    let state = match &mut *state {
                        Some(x) => x,
                        None => return (0, 0, -1, -1),
                    };

                    self.refresh_lane_sizes(state);

                    // Min and nat widths are the sum of lanes' widths.
                    let (min, nat) = state
                        .lane_sizes
                        .iter()
                        .fold((0, 0), |(min, nat), (min_lane, nat_lane)| {
                            (min + min_lane, nat + nat_lane)
                        });

                    // Also take the timing lines into account.
                    let min_tl = state
                        .timing_lines
                        .iter()
                        .map(|widget| widget.measure(gtk::Orientation::Horizontal, -1).0)
                        .max()
                        .unwrap_or(0);

                    let min = min.max(min_tl);
                    let nat = nat.max(min_tl);

                    trace!("returning min width = {min}, nat width = {nat}");
                    (min, nat, -1, -1)
                }
                gtk::Orientation::Vertical => {
                    // Our height can always go down to 0.
                    (0, 0, -1, -1)
                }
                _ => unimplemented!(),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            trace!("Playfield::size_allocate({}, {})", width, height);

            let state = self.state.borrow();
            let state = match &*state {
                Some(x) => x,
                None => return,
            };

            let scroll_speed = self.scroll_speed.get();
            let downscroll = self.downscroll.get();
            let hit_position = self.hit_position.get();

            for (line, widget) in state
                .game
                .immutable
                .timing_lines
                .iter()
                .zip(&state.timing_lines)
            {
                // Our width is guaranteed to fit the timing lines because we considered them in
                // measure().

                let difference = line.position - state.map_position;
                let mut y = to_pixels(difference * scroll_speed) + hit_position;
                if y >= height {
                    widget.set_child_visible(false);
                    continue;
                }

                let widget_height = widget.measure(gtk::Orientation::Vertical, width).1;
                if y + widget_height <= 0 {
                    widget.set_child_visible(false);
                    continue;
                }
                widget.set_child_visible(true);

                if downscroll {
                    y = height - y - widget_height;
                }

                let mut transform =
                    gsk::Transform::new().translate(&graphene::Point::new(0., y as f32));
                if downscroll {
                    transform = transform
                        .translate(&graphene::Point::new(0., widget_height as f32))
                        .scale(1., -1.)
                }

                widget.allocate(width, widget_height, -1, Some(&transform));
            }

            let mut x = 0;
            let lane_widths = compute_lane_widths(state, width);

            for (((cache, lane_state), widgets), lane_width) in state
                .game
                .immutable
                .lane_caches
                .iter()
                .zip(&state.game.lane_states)
                .zip(&state.objects)
                .zip(lane_widths)
            {
                for ((cache, obj_state), widget) in cache
                    .object_caches
                    .iter()
                    .zip(&lane_state.object_states)
                    .zip(widgets)
                {
                    if obj_state.is_hidden() {
                        widget.set_child_visible(false);
                        continue;
                    }

                    let position =
                        state
                            .game
                            .object_start_position(*obj_state, *cache, state.map_position);
                    let difference = position - state.map_position;
                    let mut y = to_pixels(difference * scroll_speed) + hit_position;
                    if y >= height {
                        widget.set_child_visible(false);
                        continue;
                    }

                    let widget_height = widget.measure(gtk::Orientation::Vertical, lane_width).1;
                    if y + widget_height <= 0 {
                        widget.set_child_visible(false);
                        continue;
                    }
                    widget.set_child_visible(true);

                    if downscroll {
                        y = height - y - widget_height;
                    }

                    let mut transform =
                        gsk::Transform::new().translate(&graphene::Point::new(x as f32, y as f32));
                    if downscroll {
                        transform = transform
                            .translate(&graphene::Point::new(0., widget_height as f32))
                            .scale(1., -1.)
                    }

                    widget.allocate(lane_width, widget_height, -1, Some(&transform));
                }

                x += lane_width;
            }
        }
    }

    impl ScrollableImpl for Playfield {}

    impl Playfield {
        pub fn set_downscroll(&self, value: bool) {
            if self.downscroll.get() != value {
                self.downscroll.set(value);

                let obj = self.obj();
                obj.notify("downscroll");
                obj.queue_allocate();
            }
        }

        pub fn scroll_speed(&self) -> ScrollSpeed {
            self.scroll_speed.get()
        }

        pub fn set_scroll_speed(&self, value: ScrollSpeed) {
            if self.scroll_speed.get() != value {
                self.scroll_speed.set(value);

                self.update_ln_lengths();

                let obj = self.obj();
                obj.notify("scroll-speed");
                obj.queue_allocate();
            }
        }

        pub fn set_lane_width(&self, value: i32) {
            if self.lane_width.get() != value {
                self.lane_width.set(value);

                let obj = self.obj();
                obj.notify("lane-width");
                obj.queue_resize();
            }
        }

        pub fn set_hit_position(&self, value: i32) {
            if self.hit_position.get() != value {
                self.hit_position.set(value);

                let obj = self.obj();
                obj.notify("hit-position");
                obj.queue_allocate();
            }
        }

        pub fn set_game_timestamp(&self, value: GameTimestamp) {
            if self.game_timestamp.get() != value {
                self.game_timestamp.set(value);

                let obj = self.obj();
                obj.notify("game-timestamp");

                let mut state = self.state.borrow_mut();
                if let Some(state) = &mut *state {
                    let map_timestamp = value.to_map(&state.game.timestamp_converter);
                    let position = state.game.position_at_time(map_timestamp);
                    if state.map_position != position {
                        state.map_position = position;
                        obj.queue_allocate();
                    }
                }
            }
        }

        pub fn set_game_state(&self, value: Option<GameState>) {
            let obj = self.obj();
            obj.queue_resize();

            while let Some(child) = obj.first_child() {
                child.unparent();
            }

            let game = match value {
                Some(x) => x,
                None => {
                    if self.state.replace(None).is_some() {
                        obj.notify("game-state");
                    }
                    return;
                }
            };

            let timing_lines = game
                .immutable
                .timing_lines
                .iter()
                .map(|_| {
                    let widget = gtk::Separator::new(gtk::Orientation::Horizontal);
                    widget.set_parent(&*obj);
                    widget
                })
                .collect();

            let objects = game
                .immutable
                .lane_caches
                .iter()
                .map(|lane| {
                    let widgets: Vec<gtk::Widget> = lane
                        .object_caches
                        .iter()
                        .map(|object| match object {
                            ObjectCache::Regular { .. } => gtk::Picture::new().upcast(),
                            ObjectCache::LongNote { .. } => LongNote::new().upcast(),
                        })
                        .collect();

                    // Set parent in reverse to get the right draw order.
                    for widget in widgets.iter().rev() {
                        widget.set_parent(&*obj);
                    }

                    widgets
                })
                .collect();

            let map_position =
                game.position_at_time(self.game_timestamp.get().to_map(&game.timestamp_converter));

            let state = State {
                lane_sizes: vec![(0, 0); game.lane_count()],
                game,
                objects,
                timing_lines,
                map_position,
            };

            self.state.replace(Some(state));

            self.update_ln_lengths();
            self.update_skin();

            obj.notify("game-state");
        }

        pub fn set_skin(&self, value: Option<Skin>) {
            let value_is_some = value.is_some();
            if self.skin.replace(value).is_some() || value_is_some {
                self.update_skin();
                self.obj().notify("skin");
            }
        }

        pub fn game_state(&self) -> Option<Ref<GameState>> {
            let state = self.state.borrow();
            if state.is_some() {
                Some(Ref::map(state, |state| {
                    if let Some(state) = state {
                        &state.game
                    } else {
                        unreachable!()
                    }
                }))
            } else {
                None
            }
        }

        pub fn game_state_mut(&self) -> Option<RefMut<GameState>> {
            let state = self.state.borrow_mut();
            if state.is_some() {
                Some(RefMut::map(state, |state| {
                    if let Some(state) = state {
                        &mut state.game
                    } else {
                        unreachable!()
                    }
                }))
            } else {
                None
            }
        }

        fn update_skin(&self) {
            let state = self.state.borrow();
            let state = match &*state {
                Some(x) => x,
                None => return,
            };

            let skin = self.skin.borrow();
            let store = skin.as_ref().map(|s| s.store());

            let lane_count = state.game.lane_count();

            for ((lane, lane_state), widgets) in state
                .game
                .lane_states
                .iter()
                .enumerate()
                .zip(&state.objects)
            {
                let lane_skin = store.map(|s| s.get(lane_count, lane));

                for (obj_state, widget) in lane_state.object_states.iter().zip(widgets) {
                    match obj_state {
                        ObjectState::Regular(_) => {
                            let picture = widget.downcast_ref::<gtk::Picture>().unwrap();
                            picture.set_paintable(lane_skin.map(|s| &s.object));
                        }
                        ObjectState::LongNote(_) => {
                            let long_note = widget.downcast_ref::<LongNote>().unwrap();
                            long_note.set_head_paintable(lane_skin.map(|s| &s.ln_head));
                            long_note.set_tail_paintable(lane_skin.map(|s| &s.ln_tail));
                            long_note.set_body_paintable(lane_skin.map(|s| &s.ln_body));
                        }
                    }
                }
            }
        }

        pub fn update_ln_lengths(&self) {
            let state = self.state.borrow();
            let state = match &*state {
                Some(x) => x,
                None => return,
            };

            for ((cache, lane_state), widgets) in state
                .game
                .immutable
                .lane_caches
                .iter()
                .zip(&state.game.lane_states)
                .zip(&state.objects)
            {
                for ((cache, obj_state), widget) in cache
                    .object_caches
                    .iter()
                    .zip(&lane_state.object_states)
                    .zip(widgets)
                {
                    if let ObjectCache::LongNote(_) = cache {
                        let long_note: &LongNote = widget.downcast_ref().unwrap();
                        let start_position = state.game.object_start_position(
                            *obj_state,
                            *cache,
                            state.map_position,
                        );
                        long_note.set_length(
                            (cache.end_position() - start_position) * self.scroll_speed.get(),
                        );
                    }
                }
            }
        }

        fn refresh_lane_sizes(&self, state: &mut State) {
            let lane_sizes = state.objects.iter().map(|lane| {
                // Min and nat width for a lane is the maximum across objects.
                // TODO: handle empty lanes better.
                lane.iter()
                    .map(|widget| widget.measure(gtk::Orientation::Horizontal, -1))
                    .fold((0, 0), |(min, nat), (min_w, nat_w, _, _)| {
                        (min.max(min_w), nat.max(nat_w))
                    })
            });

            for (place, value) in state.lane_sizes.iter_mut().zip(lane_sizes) {
                *place = value;
            }

            self.scale_lane_nat_sizes(state);
        }

        fn scale_lane_nat_sizes(&self, state: &mut State) {
            let lane_width = self.lane_width.get();
            if lane_width == 0 {
                // Lane width not set.
                return;
            }

            // Count the number of lanes sized the same.
            let mut nat_sizes = HashMap::with_capacity(state.lane_sizes.len());
            for &(_, nat) in &state.lane_sizes {
                *nat_sizes.entry(nat).or_insert(0) += 1;
            }

            // Find the most common non-zero lane size. This is the one we'll use for scaling.
            let mut nat_sizes: Vec<_> = nat_sizes.into_iter().collect();
            nat_sizes.sort_by_key(|&(_, count)| count);

            match nat_sizes.into_iter().rev().find(|&(nat, _)| nat > 0) {
                Some((most_common, _)) => {
                    // Compute scale based on the most common nat lane size.
                    let scale = lane_width as f64 / most_common as f64;
                    for (min, nat) in &mut state.lane_sizes {
                        *nat = ((*nat as f64 * scale).round() as i32).max(*min);
                    }
                }
                None => {
                    // All nat sizes were zero.
                    for (min, nat) in &mut state.lane_sizes {
                        *nat = lane_width.max(*min);
                    }
                }
            }
        }
    }

    fn compute_lane_widths(state: &State, width: i32) -> impl Iterator<Item = i32> + '_ {
        // When the playfield is smaller or bigger than its natural size, we want all lanes to be
        // smaller or bigger in the same proportion. However, when making the playfield smaller, the
        // desired width for some lanes might end up below their min width. In this case these lanes
        // are given their min width, and to compensate for that, the other lanes are made even
        // smaller.
        //
        // This loop iteratively reduces the scale until all lanes would fit.
        let mut remaining_width = width;
        let mut remaining_nat = state.lane_sizes.iter().map(|(_, nat)| nat).sum::<i32>();
        let mut at_min_width = vec![false; state.game.lane_count()];

        loop {
            let scale = remaining_width as f64 / remaining_nat as f64;

            let mut nothing_changed = true;
            for (&(min, nat), at_min) in state.lane_sizes.iter().zip(&mut at_min_width) {
                if !*at_min && (nat as f64 * scale).floor() as i32 <= min {
                    nothing_changed = false;
                    *at_min = true;

                    // Remove this lane from the scale computation. It will be allocated minimum
                    // size. Counter-intuitively, this *may* sometimes increase the scale!
                    remaining_width -= min;
                    remaining_nat -= nat;
                }
            }

            if nothing_changed || remaining_nat == 0 {
                break;
            }
        }

        let scale = if remaining_nat == 0 {
            0.
        } else {
            remaining_width as f64 / remaining_nat as f64
        };

        let widths = state
            .lane_sizes
            .iter()
            .zip(at_min_width)
            .map(move |(&(min, nat), at_min)| {
                if at_min {
                    min
                } else {
                    (nat as f64 * scale).floor() as i32
                }
            });

        // Make sure we got a valid result.
        assert!(widths.clone().sum::<i32>() <= width);

        widths
    }
}

glib::wrapper! {
    pub struct Playfield(ObjectSubclass<imp::Playfield>)
        @extends gtk::Widget;
}

impl Playfield {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn set_game_state(&self, value: Option<GameState>) {
        self.imp().set_game_state(value);
    }

    pub fn set_skin(&self, value: Option<Skin>) {
        self.imp().set_skin(value);
    }

    pub fn set_downscroll(&self, value: bool) {
        self.imp().set_downscroll(value);
    }

    pub fn scroll_speed(&self) -> ScrollSpeed {
        self.imp().scroll_speed()
    }

    pub fn set_scroll_speed(&self, value: ScrollSpeed) {
        self.imp().set_scroll_speed(value);
    }

    pub fn set_game_timestamp(&self, timestamp: GameTimestamp) {
        self.imp().set_game_timestamp(timestamp);
    }

    pub fn set_lane_width(&self, value: i32) {
        self.imp().set_lane_width(value);
    }

    pub fn state(&self) -> Option<Ref<GameState>> {
        self.imp().game_state()
    }

    pub fn state_mut(&self) -> Option<RefMut<GameState>> {
        self.imp().game_state_mut()
    }

    pub fn update_ln_lengths(&self) {
        self.imp().update_ln_lengths();
    }
}

impl Default for Playfield {
    fn default() -> Self {
        Self::new()
    }
}
