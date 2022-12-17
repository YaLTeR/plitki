use gtk::glib;
use gtk::subclass::prelude::*;
use plitki_core::scroll::ScrollSpeed;
use plitki_core::timing::GameTimestamp;

use crate::skin::LaneSkin;
use crate::state::State;

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::prelude::*;
    use gtk::{graphene, gsk};
    use log::trace;
    use once_cell::sync::Lazy;
    use plitki_core::scroll::Position;
    use plitki_core::state::{ObjectCache, ObjectState};

    use super::*;
    use crate::long_note::LongNote;
    use crate::skin::{BoxedLaneSkin, LaneSkin};
    use crate::utils::to_pixels;

    #[derive(Debug)]
    struct Data {
        state: State,
        widgets: Vec<gtk::Widget>,
        is_visible: Vec<bool>,
        map_position: Position,
    }

    #[derive(Debug)]
    pub struct LaneConveyor {
        lane: Cell<u32>,
        data: RefCell<Option<Data>>,
        skin: RefCell<Option<LaneSkin>>,
        scroll_speed: Cell<ScrollSpeed>,
        game_timestamp: Cell<GameTimestamp>,
        downscroll: Cell<bool>,
        hit_position: Cell<i32>,
    }

    impl Default for LaneConveyor {
        fn default() -> Self {
            Self {
                lane: Default::default(),
                data: Default::default(),
                skin: Default::default(),
                scroll_speed: Cell::new(ScrollSpeed(30)),
                game_timestamp: Cell::new(GameTimestamp::zero()),
                downscroll: Default::default(),
                hit_position: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LaneConveyor {
        const NAME: &'static str = "PlitkiLaneConveyor";
        type Type = super::LaneConveyor;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for LaneConveyor {
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
                    glib::ParamSpecUInt::builder("lane")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecObject::builder::<State>("state")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecBoxed::builder::<BoxedLaneSkin>("skin")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecUChar::builder("scroll-speed")
                        .default_value(30)
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecInt::builder("game-timestamp")
                        .minimum(-(2i32.pow(30)))
                        .maximum(2i32.pow(30) - 1)
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecBoolean::builder("downscroll")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecInt::builder("hit-position")
                        .minimum(-10_000)
                        .maximum(10_000)
                        .explicit_notify()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "lane" => self.lane().to_value(),
                "state" => self.state().to_value(),
                "skin" => BoxedLaneSkin(self.skin()).to_value(),
                "scroll-speed" => self.scroll_speed().0.to_value(),
                "game-timestamp" => self.game_timestamp().into_milli_hundredths().to_value(),
                "downscroll" => self.downscroll().to_value(),
                "hit-position" => self.hit_position().to_value(),
                _ => unreachable!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "lane" => self.set_lane(value.get().unwrap()),
                "state" => self.set_state(value.get().unwrap()),
                "skin" => self.set_skin(value.get::<BoxedLaneSkin>().unwrap().0),
                "scroll-speed" => self.set_scroll_speed(ScrollSpeed(value.get().unwrap())),
                "game-timestamp" => {
                    let timestamp = GameTimestamp::from_milli_hundredths(value.get().unwrap());
                    self.set_game_timestamp(timestamp)
                }
                "downscroll" => self.set_downscroll(value.get().unwrap()),
                "hit-position" => self.set_hit_position(value.get().unwrap()),
                _ => unreachable!(),
            }
        }
    }

    impl WidgetImpl for LaneConveyor {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            trace!("LaneConveyor::measure({}, {})", orientation, for_size);

            match orientation {
                gtk::Orientation::Horizontal => {
                    let Some(data) = &*self.data.borrow() else { return (0, 0, -1, -1) };

                    // Min and nat width for a lane is the maximum across objects.
                    let (min, nat) = data
                        .widgets
                        .iter()
                        .map(|widget| widget.measure(gtk::Orientation::Horizontal, -1))
                        .map(|(min_w, nat_w, _, _)| (min_w, nat_w))
                        .reduce(|(min, nat), (min_w, nat_w)| (min.max(min_w), nat.max(nat_w)))
                        // TODO: figure out better handling for empty lanes.
                        .unwrap_or((0, 0));

                    (min, nat, -1, -1)
                }
                gtk::Orientation::Vertical => {
                    // Our height can always go down to 0.
                    (0, 0, -1, -1)
                }
                _ => unreachable!(),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            trace!("LaneConveyor::size_allocate({}, {})", width, height);

            let Some(data) = &mut *self.data.borrow_mut() else { return };
            let game_state = data.state.game_state();

            let scroll_speed = self.scroll_speed.get();
            let downscroll = self.downscroll.get();
            let hit_position = self.hit_position.get();

            let lane: usize = self.lane.get().try_into().unwrap();
            for (((widget, is_visible), obj_cache), obj_state) in data
                .widgets
                .iter()
                .zip(&mut data.is_visible)
                .zip(&game_state.immutable.lane_caches[lane].object_caches)
                .zip(&game_state.lane_states[lane].object_states)
            {
                if obj_state.is_hit() {
                    if *is_visible {
                        widget.set_child_visible(false);
                        *is_visible = false;
                    }
                    continue;
                }

                let position =
                    game_state.object_start_position(*obj_state, *obj_cache, data.map_position);
                let difference = position - data.map_position;
                let mut y = to_pixels(difference * scroll_speed) + hit_position;
                if y >= height {
                    if *is_visible {
                        widget.set_child_visible(false);
                        *is_visible = false;
                    }
                    continue;
                }

                let widget_height = widget.measure(gtk::Orientation::Vertical, width).1;
                if y + widget_height <= 0 {
                    if *is_visible {
                        widget.set_child_visible(false);
                        *is_visible = false;
                    }
                    continue;
                }

                if !*is_visible {
                    widget.set_child_visible(true);
                    *is_visible = true;
                }

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
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let Some(data) = &*self.data.borrow_mut() else { return };
            let obj = self.obj();

            for (widget, is_visible) in data.widgets.iter().zip(&data.is_visible) {
                if *is_visible {
                    obj.snapshot_child(widget, snapshot);
                }
            }
        }
    }

    impl LaneConveyor {
        fn build_data(&self, state: State) -> Data {
            let obj = self.obj();
            let game_state = state.game_state();

            let lane: usize = self.lane.get().try_into().unwrap();
            let widgets: Vec<gtk::Widget> = game_state.immutable.lane_caches[lane]
                .object_caches
                .iter()
                .map(|object| match object {
                    ObjectCache::Regular { .. } => gtk::Picture::new().upcast(),
                    ObjectCache::LongNote { .. } => LongNote::new().upcast(),
                })
                .collect();

            // Set parent in reverse to get the right draw order.
            for widget in widgets.iter().rev() {
                // Default to invisible.
                widget.set_child_visible(false);
                widget.set_parent(&*obj);
            }

            let is_visible = vec![false; widgets.len()];

            let map_position = game_state.position_at_time(
                self.game_timestamp()
                    .to_map(&game_state.timestamp_converter),
            );

            drop(game_state);
            Data {
                state,
                widgets,
                is_visible,
                map_position,
            }
        }

        fn update_skin(&self) {
            let Some(data) = &*self.data.borrow() else { return };
            let game_state = data.state.game_state();

            let lane: usize = self.lane.get().try_into().unwrap();
            let skin = self.skin.borrow();
            let skin = skin.as_ref();
            let object_texture = skin.map(|s| &s.object);
            let ln_head = skin.map(|s| &s.ln_head);
            let ln_tail = skin.map(|s| &s.ln_tail);
            let ln_body = skin.map(|s| &s.ln_body);

            for (widget, obj_state) in data
                .widgets
                .iter()
                .zip(&game_state.lane_states[lane].object_states)
            {
                match obj_state {
                    ObjectState::Regular(_) => {
                        let picture = widget.downcast_ref::<gtk::Picture>().unwrap();
                        picture.set_paintable(object_texture);
                    }
                    ObjectState::LongNote(_) => {
                        let long_note = widget.downcast_ref::<LongNote>().unwrap();
                        long_note.set_head_paintable(ln_head);
                        long_note.set_tail_paintable(ln_tail);
                        long_note.set_body_paintable(ln_body);
                    }
                }
            }
        }

        pub fn update_ln_lengths(&self) {
            let Some(data) = &*self.data.borrow() else { return };
            let game_state = data.state.game_state();

            let lane: usize = self.lane.get().try_into().unwrap();
            for ((widget, obj_cache), obj_state) in data
                .widgets
                .iter()
                .zip(&game_state.immutable.lane_caches[lane].object_caches)
                .zip(&game_state.lane_states[lane].object_states)
            {
                if let ObjectCache::LongNote(_) = obj_cache {
                    let long_note: &LongNote = widget.downcast_ref().unwrap();
                    let start_position =
                        game_state.object_start_position(*obj_state, *obj_cache, data.map_position);
                    long_note.set_length(
                        (obj_cache.end_position() - start_position) * self.scroll_speed.get(),
                    );
                }
            }
        }

        fn replace_state(&self, state: Option<State>) {
            let mut data = self.data.borrow_mut();
            if let Some(data) = &*data {
                for widget in &data.widgets {
                    widget.unparent();
                }
            }
            *data = None;

            if let Some(state) = state {
                *data = Some(self.build_data(state));
                drop(data);
                self.update_ln_lengths();
                self.update_skin();
            }
        }

        pub fn lane(&self) -> u32 {
            self.lane.get()
        }

        pub fn set_lane(&self, value: u32) {
            if self.lane.get() == value {
                return;
            }

            self.lane.set(value);
            self.replace_state(self.state());
            self.obj().notify("lane");
        }

        pub fn state(&self) -> Option<State> {
            self.data.borrow().as_ref().map(|data| data.state.clone())
        }

        pub fn set_state(&self, value: Option<State>) {
            if self.data.borrow().as_ref().map(|data| &data.state) == value.as_ref() {
                return;
            }

            self.replace_state(value);
            self.obj().notify("state");
        }

        pub fn skin(&self) -> Option<LaneSkin> {
            self.skin.borrow().clone()
        }

        pub fn set_skin(&self, value: Option<LaneSkin>) {
            if *self.skin.borrow() == value {
                return;
            }

            self.skin.replace(value);
            self.update_skin();
            self.obj().notify("skin");
        }

        pub fn scroll_speed(&self) -> ScrollSpeed {
            self.scroll_speed.get()
        }

        pub fn set_scroll_speed(&self, value: ScrollSpeed) {
            if self.scroll_speed.get() == value {
                return;
            }

            self.scroll_speed.set(value);
            self.update_ln_lengths();
            self.obj().queue_allocate();
            self.obj().notify("scroll-speed");
        }

        pub fn game_timestamp(&self) -> GameTimestamp {
            self.game_timestamp.get()
        }

        pub fn set_game_timestamp(&self, value: GameTimestamp) {
            if self.game_timestamp.get() == value {
                return;
            }

            assert!(value.into_milli_hundredths() >= -(2i32.pow(30)));
            assert!(value.into_milli_hundredths() < 2i32.pow(30));

            self.game_timestamp.set(value);
            if let Some(data) = &mut *self.data.borrow_mut() {
                let game_state = data.state.game_state();
                let map_timestamp = value.to_map(&game_state.timestamp_converter);
                let position = game_state.position_at_time(map_timestamp);
                if data.map_position != position {
                    data.map_position = position;
                    self.obj().queue_allocate();
                }
            }
            self.obj().notify("game-timestamp");
        }

        pub fn downscroll(&self) -> bool {
            self.downscroll.get()
        }

        pub fn set_downscroll(&self, value: bool) {
            if self.downscroll.get() == value {
                return;
            }

            self.downscroll.set(value);
            self.obj().queue_allocate();
            self.obj().notify("downscroll");
        }

        pub fn hit_position(&self) -> i32 {
            self.hit_position.get()
        }

        pub fn set_hit_position(&self, value: i32) {
            if self.hit_position.get() == value {
                return;
            }

            assert!(value >= -10_000);
            assert!(value <= 10_000);

            self.hit_position.set(value);
            self.obj().queue_allocate();
            self.obj().notify("hit-position");
        }
    }
}

glib::wrapper! {
    pub struct LaneConveyor(ObjectSubclass<imp::LaneConveyor>)
        @extends gtk::Widget;
}

impl LaneConveyor {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn update_ln_lengths(&self) {
        self.imp().update_ln_lengths();
    }

    pub fn lane(&self) -> u32 {
        self.imp().lane()
    }

    pub fn set_lane(&self, value: u32) {
        self.imp().set_lane(value);
    }

    pub fn state(&self) -> Option<State> {
        self.imp().state()
    }

    pub fn set_state(&self, value: Option<State>) {
        self.imp().set_state(value);
    }

    pub fn skin(&self) -> Option<LaneSkin> {
        self.imp().skin()
    }

    pub fn set_skin(&self, value: Option<LaneSkin>) {
        self.imp().set_skin(value);
    }

    pub fn scroll_speed(&self) -> ScrollSpeed {
        self.imp().scroll_speed()
    }

    pub fn set_scroll_speed(&self, value: ScrollSpeed) {
        self.imp().set_scroll_speed(value);
    }

    pub fn game_timestamp(&self) -> GameTimestamp {
        self.imp().game_timestamp()
    }

    pub fn set_game_timestamp(&self, value: GameTimestamp) {
        self.imp().set_game_timestamp(value);
    }

    pub fn downscroll(&self) -> bool {
        self.imp().downscroll()
    }

    pub fn set_downscroll(&self, value: bool) {
        self.imp().set_downscroll(value);
    }

    pub fn hit_position(&self) -> i32 {
        self.imp().hit_position()
    }

    pub fn set_hit_position(&self, value: i32) {
        self.imp().set_hit_position(value);
    }
}

impl Default for LaneConveyor {
    fn default() -> Self {
        Self::new()
    }
}
