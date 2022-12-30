use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use plitki_core::scroll::ScrollSpeed;
use plitki_core::timing::GameTimestamp;

use crate::skin::Skin;
use crate::state::State;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    use gtk::{gdk, graphene, gsk};
    use log::trace;
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;
    use plitki_core::scroll::{Position, ScrollSpeed};
    use plitki_core::state::{LongNoteCache, ObjectCache, RegularObjectCache};

    use super::*;
    use crate::conveyor::Conveyor;
    use crate::conveyor_widget::{ConveyorWidget, ConveyorWidgetExt};
    use crate::long_note::LongNote;
    use crate::regular_note::RegularNote;
    use crate::skin::LaneSkin;
    use crate::timing_line::TimingLine;

    #[derive(Debug)]
    enum NoteWidget {
        Regular(RegularNote),
        Long(LongNote),
    }

    impl NoteWidget {
        fn as_conveyor_widget(&self) -> &ConveyorWidget {
            match self {
                NoteWidget::Regular(regular) => regular.upcast_ref(),
                NoteWidget::Long(long) => long.upcast_ref(),
            }
        }

        fn set_skin(&self, skin: Option<&LaneSkin>) {
            match self {
                NoteWidget::Regular(regular) => regular.set_skin(skin),
                NoteWidget::Long(long) => long.set_skin(skin),
            }
        }

        fn as_long(&self) -> Option<&LongNote> {
            if let Self::Long(v) = self {
                Some(v)
            } else {
                None
            }
        }
    }

    #[derive(Debug)]
    struct Data {
        state: State,
        notes: Vec<Vec<NoteWidget>>,
        conveyors: Vec<Conveyor>,
        hit_lights: Vec<gtk::Widget>,
        map_position: Position,

        /// Cached min and nat widths for each lane.
        ///
        /// Refreshed in measure() and valid only in size_allocate().
        lane_sizes: Vec<(i32, i32)>,
    }

    #[derive(Debug)]
    pub struct Playfield {
        data: RefCell<Option<Data>>,
        timing_line_conveyor: OnceCell<Conveyor>,
        skin: RefCell<Option<Skin>>,
        scroll_speed: Cell<ScrollSpeed>,
        game_timestamp: Cell<GameTimestamp>,
        downscroll: Cell<bool>,
        lane_width: Cell<i32>,
        hit_position: Cell<i32>,
        hit_light_widget_type: Cell<glib::Type>,
    }

    impl Default for Playfield {
        fn default() -> Self {
            Self {
                data: Default::default(),
                timing_line_conveyor: Default::default(),
                skin: Default::default(),
                scroll_speed: Cell::new(ScrollSpeed(30)),
                game_timestamp: Cell::new(GameTimestamp::zero()),
                downscroll: Default::default(),
                lane_width: Cell::new(0),
                hit_position: Cell::new(0),
                hit_light_widget_type: Cell::new(gtk::Picture::static_type()),
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
            let obj = self.obj();
            self.parent_constructed();

            obj.set_overflow(gtk::Overflow::Hidden);

            let timing_line_conveyor = Conveyor::new();
            timing_line_conveyor.set_parent(&*obj);
            for name in ["scroll-speed", "downscroll", "hit-position"] {
                obj.bind_property(name, &timing_line_conveyor, name)
                    .sync_create()
                    .build();
            }
            self.timing_line_conveyor.set(timing_line_conveyor).unwrap();
        }

        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::builder::<State>("state")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecObject::builder::<Skin>("skin")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecUInt::builder("scroll-speed")
                        .maximum(255)
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
                    glib::ParamSpecInt::builder("lane-width")
                        .minimum(0)
                        .maximum(10_000)
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecInt::builder("hit-position")
                        .minimum(-10_000)
                        .maximum(10_000)
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecGType::builder("hit-light-widget-type")
                        .is_a_type(gtk::Widget::static_type())
                        .explicit_notify()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "state" => self.set_state(value.get().unwrap()),
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
                "hit-light-widget-type" => self.set_hit_light_widget_type(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "state" => self.state().to_value(),
                "scroll-speed" => {
                    let speed: u32 = self.scroll_speed().0.into();
                    speed.to_value()
                }
                "game-timestamp" => self.game_timestamp.get().into_milli_hundredths().to_value(),
                "downscroll" => self.downscroll.get().to_value(),
                "skin" => self.skin.borrow().to_value(),
                "lane-width" => self.lane_width.get().to_value(),
                "hit-position" => self.hit_position.get().to_value(),
                "hit-light-widget-type" => self.hit_light_widget_type.get().to_value(),
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
                    let mut data = self.data.borrow_mut();
                    let Some(data) = &mut *data else { return (0, 0, -1, -1) };

                    self.refresh_lane_sizes(data);

                    // Min and nat widths are the sum of lanes' widths.
                    let (min, nat) = data
                        .lane_sizes
                        .iter()
                        .fold((0, 0), |(min, nat), (min_lane, nat_lane)| {
                            (min + min_lane, nat + nat_lane)
                        });

                    // Also take the timing lines into account.
                    let min_tl = self
                        .timing_line_conveyor
                        .get()
                        .unwrap()
                        .measure(gtk::Orientation::Horizontal, -1)
                        .0;

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

            let data = self.data.borrow();
            let Some(data) = &*data else { return };

            let downscroll = self.downscroll.get();
            let hit_position = self.hit_position.get();

            // Our width is guaranteed to fit the timing lines because we considered them in
            // measure().
            let timing_line_conveyor = self.timing_line_conveyor.get().unwrap();
            let conveyor_height = timing_line_conveyor
                .measure(gtk::Orientation::Vertical, width)
                .0;
            timing_line_conveyor.size_allocate(
                &gdk::Rectangle::new(0, 0, width, height.max(conveyor_height)),
                -1,
            );

            let mut x = 0;
            let lane_widths = compute_lane_widths(data, width);

            for ((conveyor, hit_light), lane_width) in
                data.conveyors.iter().zip(&data.hit_lights).zip(lane_widths)
            {
                let conveyor_height = conveyor.measure(gtk::Orientation::Vertical, lane_width).0;
                conveyor.size_allocate(
                    &gdk::Rectangle::new(x, 0, lane_width, height.max(conveyor_height)),
                    -1,
                );

                // Allocate the hit light.
                {
                    // Our width is guaranteed to fit the hit lights because we considered them in
                    // measure().
                    let hit_light_height =
                        hit_light.measure(gtk::Orientation::Vertical, lane_width).1;

                    let mut y = hit_position - hit_light_height;
                    if downscroll {
                        y = height - y - hit_light_height;
                    }

                    let mut transform =
                        gsk::Transform::new().translate(&graphene::Point::new(x as f32, y as f32));
                    if downscroll {
                        transform = transform
                            .translate(&graphene::Point::new(0., hit_light_height as f32))
                            .scale(1., -1.)
                    }

                    hit_light.allocate(lane_width, hit_light_height, -1, Some(&transform));
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

                self.update_object_states();

                let obj = self.obj();
                obj.notify("scroll-speed");
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

                let mut data = self.data.borrow_mut();
                if let Some(data) = &mut *data {
                    let game_state = data.state.game_state();
                    let map_timestamp = value.to_map(&game_state.timestamp_converter);
                    let position = game_state.position_at_time(map_timestamp);
                    if data.map_position != position {
                        data.map_position = position;
                        self.timing_line_conveyor
                            .get()
                            .unwrap()
                            .set_map_position(position);
                        for conveyor in &data.conveyors {
                            conveyor.set_map_position(position);
                        }
                    }
                }
            }
        }

        pub fn set_state(&self, value: Option<State>) {
            if self.data.borrow().as_ref().map(|d| &d.state) == value.as_ref() {
                return;
            }

            let obj = self.obj();
            obj.queue_resize();

            for child in self
                .data
                .borrow()
                .as_ref()
                .iter()
                .flat_map(|d| d.conveyors.iter())
            {
                child.unparent();
            }
            for child in self
                .data
                .borrow()
                .as_ref()
                .iter()
                .flat_map(|d| d.hit_lights.iter())
            {
                child.unparent();
            }

            let Some(state) = value else {
                if self.data.replace(None).is_some() {
                    obj.notify("state");
                }
                return;
            };
            let game_state = state.game_state();

            let timing_lines = game_state
                .immutable
                .timing_lines
                .iter()
                .map(|timing_line| TimingLine::new(timing_line.position).upcast())
                .collect();
            self.timing_line_conveyor
                .get()
                .unwrap()
                .set_widgets(timing_lines);

            let mut notes = Vec::new();

            let conveyors: Vec<Conveyor> = (0..game_state.lane_count())
                .map(|lane| {
                    let conveyor = Conveyor::new();
                    conveyor.set_parent(&*obj);

                    for name in ["scroll-speed", "downscroll", "hit-position"] {
                        obj.bind_property(name, &conveyor, name)
                            .sync_create()
                            .build();
                    }

                    let lane_notes: Vec<NoteWidget> = game_state.immutable.lane_caches[lane]
                        .object_caches
                        .iter()
                        .map(|&object| match object {
                            ObjectCache::Regular(RegularObjectCache { position }) => {
                                NoteWidget::Regular(RegularNote::new(position))
                            }
                            ObjectCache::LongNote(LongNoteCache { start_position, .. }) => {
                                NoteWidget::Long(LongNote::new(start_position))
                            }
                        })
                        .collect();

                    let widgets = lane_notes
                        .iter()
                        .map(NoteWidget::as_conveyor_widget)
                        .cloned()
                        .collect();
                    conveyor.set_widgets(widgets);

                    notes.push(lane_notes);

                    conveyor
                })
                .collect();

            let hit_lights = game_state
                .immutable
                .lane_caches
                .iter()
                .map(|_| {
                    let hit_light_type = self.hit_light_widget_type.get();
                    let widget: gtk::Widget = glib::Object::builder_with_type(hit_light_type)
                        .build()
                        .downcast()
                        .expect(
                            "hit-light-widget-type must prevent non-Widget types from being set",
                        );
                    widget.set_parent(&*obj);
                    widget
                })
                .collect();

            let map_position = game_state.position_at_time(
                self.game_timestamp
                    .get()
                    .to_map(&game_state.timestamp_converter),
            );
            self.timing_line_conveyor
                .get()
                .unwrap()
                .set_map_position(map_position);
            for conveyor in &conveyors {
                conveyor.set_map_position(map_position);
            }

            let lane_sizes = vec![(0, 0); game_state.lane_count()];
            drop(game_state);

            let data = Data {
                lane_sizes,
                state,
                notes,
                conveyors,
                hit_lights,
                map_position,
            };

            self.data.replace(Some(data));

            self.update_object_states();
            self.update_skin();

            obj.notify("state");
        }

        pub fn set_skin(&self, value: Option<Skin>) {
            let value_is_some = value.is_some();
            if self.skin.replace(value).is_some() || value_is_some {
                self.update_skin();
                self.obj().notify("skin");
            }
        }

        pub fn set_hit_light_widget_type(&self, value: glib::Type) {
            if self.hit_light_widget_type.get() != value {
                self.hit_light_widget_type.set(value);
                self.obj().notify("hit-light-widget-type");
                // TODO: rebuild hit light widgets.
            }
        }

        pub fn state(&self) -> Option<State> {
            self.data.borrow().as_ref().map(|d| &d.state).cloned()
        }

        pub fn hit_light_for_lane(&self, column: usize) -> gtk::Widget {
            self.data.borrow().as_ref().unwrap().hit_lights[column].clone()
        }

        fn update_skin(&self) {
            let data = self.data.borrow();
            let Some(data) = &*data else {
                return
            };
            let game_state = data.state.game_state();

            let skin = self.skin.borrow();
            let store = skin.as_ref().map(|s| s.store());
            let store = store.as_ref();

            let lane_count = game_state.lane_count();

            for (lane, lane_notes) in data.notes.iter().enumerate() {
                let lane_skin = store.map(|s| s.get(lane_count, lane));
                for widget in lane_notes {
                    widget.set_skin(lane_skin);
                }
            }
        }

        pub fn update_object_state(&self, lane: usize, index: usize) {
            let Some(data) = &*self.data.borrow() else { return };
            let game_state = data.state.game_state();

            let widget = &data.notes[lane][index];
            let obj_cache = &game_state.immutable.lane_caches[lane].object_caches[index];
            let obj_state = &game_state.lane_states[lane].object_states[index];

            if let ObjectCache::LongNote(_) = obj_cache {
                let long_note = widget.as_long().unwrap();
                let start_position =
                    game_state.object_start_position(*obj_state, *obj_cache, data.map_position);
                long_note.set_position(start_position);
                long_note.set_length(
                    (obj_cache.end_position() - start_position) * self.scroll_speed.get(),
                );
            }

            let conveyor_widget = widget.as_conveyor_widget();
            conveyor_widget.set_hit(obj_state.is_hit());
            conveyor_widget.set_missed(obj_state.is_missed());
        }

        pub fn update_object_states(&self) {
            let Some(data) = &*self.data.borrow() else { return };
            let game_state = data.state.game_state();

            for (lane, lane_notes) in data.notes.iter().enumerate() {
                for ((widget, obj_cache), obj_state) in lane_notes
                    .iter()
                    .zip(&game_state.immutable.lane_caches[lane].object_caches)
                    .zip(&game_state.lane_states[lane].object_states)
                {
                    if let ObjectCache::LongNote(_) = obj_cache {
                        let long_note = widget.as_long().unwrap();
                        let start_position = game_state.object_start_position(
                            *obj_state,
                            *obj_cache,
                            data.map_position,
                        );
                        long_note.set_position(start_position);
                        long_note.set_length(
                            (obj_cache.end_position() - start_position) * self.scroll_speed.get(),
                        );
                    }

                    let conveyor_widget = widget.as_conveyor_widget();
                    conveyor_widget.set_hit(obj_state.is_hit());
                    conveyor_widget.set_missed(obj_state.is_missed());
                }
            }
        }

        fn refresh_lane_sizes(&self, data: &mut Data) {
            let lane_sizes = data.conveyors.iter().map(|lane| {
                let (min, nat, _, _) = lane.measure(gtk::Orientation::Horizontal, -1);
                (min, nat)
            });

            let hit_light_sizes = data
                .hit_lights
                .iter()
                .map(|widget| widget.measure(gtk::Orientation::Horizontal, -1).0);

            let sizes = lane_sizes
                .zip(hit_light_sizes)
                .map(|((min_lane, nat_lane), min_light)| {
                    (min_lane.max(min_light), nat_lane.max(min_light))
                });

            for (place, value) in data.lane_sizes.iter_mut().zip(sizes) {
                *place = value;
            }

            self.scale_lane_nat_sizes(data);
        }

        fn scale_lane_nat_sizes(&self, data: &mut Data) {
            let lane_width = self.lane_width.get();
            if lane_width == 0 {
                // Lane width not set.
                return;
            }

            // Count the number of lanes sized the same.
            let mut nat_sizes = HashMap::with_capacity(data.lane_sizes.len());
            for &(_, nat) in &data.lane_sizes {
                *nat_sizes.entry(nat).or_insert(0) += 1;
            }

            // Find the most common non-zero lane size. This is the one we'll use for scaling.
            let mut nat_sizes: Vec<_> = nat_sizes.into_iter().collect();
            nat_sizes.sort_by_key(|&(_, count)| count);

            match nat_sizes.into_iter().rev().find(|&(nat, _)| nat > 0) {
                Some((most_common, _)) => {
                    // Compute scale based on the most common nat lane size.
                    let scale = lane_width as f64 / most_common as f64;
                    for (min, nat) in &mut data.lane_sizes {
                        *nat = ((*nat as f64 * scale).round() as i32).max(*min);
                    }
                }
                None => {
                    // All nat sizes were zero.
                    for (min, nat) in &mut data.lane_sizes {
                        *nat = lane_width.max(*min);
                    }
                }
            }
        }
    }

    fn compute_lane_widths(data: &Data, width: i32) -> impl Iterator<Item = i32> + '_ {
        // When the playfield is smaller or bigger than its natural size, we want all lanes to be
        // smaller or bigger in the same proportion. However, when making the playfield smaller, the
        // desired width for some lanes might end up below their min width. In this case these lanes
        // are given their min width, and to compensate for that, the other lanes are made even
        // smaller.
        //
        // This loop iteratively reduces the scale until all lanes would fit.
        let mut remaining_width = width;
        let mut remaining_nat = data.lane_sizes.iter().map(|(_, nat)| nat).sum::<i32>();
        let mut at_min_width = vec![false; data.state.game_state().lane_count()];

        loop {
            let scale = remaining_width as f64 / remaining_nat as f64;

            let mut nothing_changed = true;
            for (&(min, nat), at_min) in data.lane_sizes.iter().zip(&mut at_min_width) {
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

        let widths = data
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

    pub fn state(&self) -> Option<State> {
        self.imp().state()
    }

    pub fn set_state(&self, value: Option<State>) {
        self.imp().set_state(value);
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

    pub fn set_hit_light_widget_type(&self, value: glib::Type) {
        self.imp().set_hit_light_widget_type(value);
    }

    pub fn hit_light_for_lane(&self, lane: usize) -> gtk::Widget {
        self.imp().hit_light_for_lane(lane)
    }

    pub fn update_object_state(&self, lane: usize, index: usize) {
        self.imp().update_object_state(lane, index);
    }

    pub fn update_object_states(&self) {
        self.imp().update_object_states();
    }
}

impl Default for Playfield {
    fn default() -> Self {
        Self::new()
    }
}
