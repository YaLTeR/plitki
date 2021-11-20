use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use plitki_core::map::Map;

#[derive(Debug, Clone, glib::GBoxed)]
#[gboxed(type_name = "BoxedMap")]
pub(crate) struct BoxedMap(Map);

mod imp {
    use std::cell::RefCell;

    use gtk::gdk;
    use log::{debug, trace};
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;
    use plitki_core::scroll::ScrollSpeed;
    use plitki_core::state::{GameState, ObjectCache};

    use super::*;
    use crate::long_note::LongNote;
    use crate::skin::load_texture;
    use crate::utils::to_pixels;

    #[derive(Debug)]
    struct State {
        game: GameState,
        objects: Vec<Vec<gtk::Widget>>,
        scroll_speed: ScrollSpeed,
    }

    impl State {
        fn new(game: GameState) -> Self {
            Self {
                game,
                objects: vec![],
                scroll_speed: ScrollSpeed(32),
            }
        }
    }

    #[derive(Debug, Default)]
    pub struct View {
        state: OnceCell<RefCell<State>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for View {
        const NAME: &'static str = "PlitkiView";
        type Type = super::View;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("plitki-view");
        }
    }

    impl ObjectImpl for View {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

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
                    glib::ParamSpec::new_boxed(
                        "map",
                        "map",
                        "map",
                        BoxedMap::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpec::new_uint(
                        "scroll-speed",
                        "scroll-speed",
                        "scroll-speed",
                        0,
                        255,
                        32,
                        glib::ParamFlags::READABLE | glib::ParamFlags::WRITABLE,
                    ),
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
                    let mut state = self.state.get().expect("map needs to be set").borrow_mut();

                    if state.scroll_speed.0 != speed {
                        state.scroll_speed = ScrollSpeed(speed);

                        for (widget, cache) in state
                            .objects
                            .iter()
                            .zip(&state.game.immutable.lane_caches)
                            .flat_map(|(widget_lane, lane)| {
                                widget_lane.iter().zip(&lane.object_caches)
                            })
                        {
                            if let ObjectCache::LongNote(cache) = cache {
                                let length = (cache.end_position - cache.start_position)
                                    * state.scroll_speed;
                                widget.set_property("length", length.0).unwrap();
                            }
                        }

                        obj.queue_resize();
                    }
                }
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
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for View {
        fn request_mode(&self, _widget: &Self::Type) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(
            &self,
            widget: &Self::Type,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            trace!("View::measure({}, {})", orientation, for_size);

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
                        let lane_count: u8 = lane_count.try_into().unwrap();

                        let min_position = state.game.min_position().unwrap();
                        let max_regular = state.game.max_regular().unwrap();
                        let max_long_note = state.game.max_long_note().unwrap();

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

                        let nat = (regular_y + nat_regular).max(long_note_y + nat_long_note);
                        trace!("returning for height = {}: nat width = {}", for_size, nat);
                        (0, nat, -1, -1)
                    }
                }
                _ => unimplemented!(),
            }
        }

        fn size_allocate(&self, widget: &Self::Type, width: i32, height: i32, _baseline: i32) {
            trace!("View::size_allocate({}, {})", width, height);

            // Check that the given width would fit into the given height.
            let nat_height = self.measure(widget, gtk::Orientation::Vertical, width).1;
            let width = if nat_height <= height {
                width
            } else {
                // If it wouldn't, compute a smaller width that would fit and use that.
                let nat_width = self.measure(widget, gtk::Orientation::Horizontal, height).1;
                assert!(nat_width < width);
                nat_width
            };

            let state = self.state().borrow();
            let lane_count: i32 = state.objects.len().try_into().unwrap();
            let lane_width = width / lane_count;
            let lane_count: u8 = lane_count.try_into().unwrap();

            let first_position = state.game.min_position().unwrap();

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
                    let y = to_pixels(difference * state.scroll_speed, lane_width, lane_count);
                    let height = widget.measure(gtk::Orientation::Vertical, lane_width).1;
                    widget.size_allocate(
                        &gdk::Rectangle {
                            x,
                            y,
                            width: lane_width,
                            height,
                        },
                        -1,
                    );
                }
            }
        }
    }

    impl View {
        fn state(&self) -> &RefCell<State> {
            self.state
                .get()
                .expect("map property was not set during construction")
        }

        pub fn rebuild(&self, obj: &super::View) {
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

            let textures = [
                "note-hitobject-1.png",
                "note-hitobject-2.png",
                "note-hitobject-3.png",
                "note-hitobject-4.png",
            ]
            .map(load_texture);

            let heads = [
                "note-holdhitobject-1.png",
                "note-holdhitobject-2.png",
                "note-holdhitobject-3.png",
                "note-holdhitobject-4.png",
            ]
            .map(load_texture);
            let tails = [
                "note-holdend-1.png",
                "note-holdend-2.png",
                "note-holdend-3.png",
                "note-holdend-4.png",
            ]
            .map(load_texture);
            let bodies = [
                "note-holdbody-1.png",
                "note-holdbody-2.png",
                "note-holdbody-3.png",
                "note-holdbody-4.png",
            ]
            .map(load_texture);

            for ((((lane, texture), head), tail), body) in state
                .game
                .immutable
                .lane_caches
                .iter()
                .zip(textures)
                .zip(heads)
                .zip(tails)
                .zip(bodies)
            {
                let mut widgets = Vec::new();

                for object in &lane.object_caches {
                    let widget: gtk::Widget = match object {
                        ObjectCache::Regular { .. } => gtk::Picture::builder()
                            .paintable(&texture)
                            .css_classes(vec!["upside-down".to_string()])
                            .build()
                            .upcast(),
                        ObjectCache::LongNote { .. } => LongNote::new(
                            &gtk::Picture::builder()
                                .paintable(&head)
                                .css_classes(vec!["upside-down".to_string()])
                                .build(),
                            &gtk::Picture::builder()
                                .paintable(&tail)
                                .css_classes(vec!["upside-down".to_string()])
                                .build(),
                            &gtk::Picture::builder()
                                .paintable(&body)
                                .keep_aspect_ratio(false)
                                .css_classes(vec!["upside-down".to_string()])
                                .build(),
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
    }
}

glib::wrapper! {
    pub struct View(ObjectSubclass<imp::View>)
        @extends gtk::Widget;
}

impl View {
    pub(crate) fn new(map: Map) -> Self {
        glib::Object::new(&[("map", &BoxedMap(map))]).unwrap()
    }

    pub(crate) fn rebuild(&self) {
        imp::View::from_instance(self).rebuild(self);
    }
}
