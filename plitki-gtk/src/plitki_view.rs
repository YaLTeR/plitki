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
    use plitki_core::scroll::{ScreenPositionDifference, ScrollSpeed};
    use plitki_core::state::GameState;

    use super::*;

    #[derive(Debug)]
    struct State {
        game: GameState,
        objects: Vec<Vec<gtk::Picture>>,
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
    pub struct PlitkiView {
        state: OnceCell<RefCell<State>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PlitkiView {
        const NAME: &'static str = "PlitkiView";
        type Type = super::PlitkiView;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for PlitkiView {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let mut state = self.state().borrow_mut();
            let state = &mut *state;
            let map = &state.game.immutable.map;

            debug!(
                "{} - {} [{}]",
                map.song_artist.as_ref().unwrap(),
                map.song_title.as_ref().unwrap(),
                map.difficulty_name.as_ref().unwrap()
            );

            let texture = gdk::MemoryTexture::new(
                3,
                1,
                gdk::MemoryFormat::R8g8b8,
                &glib::Bytes::from_static(&[50, 200, 50, 200, 200, 50, 200, 50, 50]),
                3,
            );

            for lane in &map.lanes {
                let mut widgets = Vec::new();

                for _object in &lane.objects {
                    let picture = gtk::Picture::for_paintable(Some(&texture));
                    picture.set_parent(obj);
                    widgets.push(picture);
                }

                state.objects.push(widgets);
            }
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_boxed(
                    "map",
                    "map",
                    "map",
                    BoxedMap::static_type(),
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
                "map" => {
                    let map = value.get::<BoxedMap>().expect("wrong property type").0;
                    let state = State::new(GameState::new(map).expect("invalid map"));
                    self.state
                        .set(RefCell::new(state))
                        .expect("property set more than once");
                }
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for PlitkiView {
        fn request_mode(&self, _widget: &Self::Type) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(
            &self,
            widget: &Self::Type,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            trace!("PlitkiView::measure({}, {})", orientation, for_size);

            match orientation {
                gtk::Orientation::Horizontal => {
                    if for_size == -1 {
                        let state = self.state().borrow();
                        let lane_count = state.objects.len();

                        // Each lane is 1 px wide at minimum.
                        let min_width = lane_count.try_into().unwrap();
                        trace!("returning width = {}", min_width);
                        (min_width, min_width, -1, -1)
                    } else {
                        // It's complicated. Return a dummy width for now.
                        let min_width = 1;
                        trace!("TODO returning width = {}", min_width);
                        (min_width, min_width, -1, -1)
                    }
                }
                gtk::Orientation::Vertical => {
                    let width = if for_size == -1 {
                        self.measure(widget, gtk::Orientation::Horizontal, -1).0
                    } else {
                        for_size
                    };

                    let state = self.state().borrow();

                    let lane_count: i32 = state.objects.len().try_into().unwrap();
                    let lane_width = width / lane_count;
                    let square_width = lane_width * lane_count;
                    let to_pixels = |difference: ScreenPositionDifference| {
                        (difference.0 as f64 / 2_000_000_000. * square_width as f64).round() as i32
                    };

                    let first_position = state.game.first_position().unwrap();

                    let (l, position) = state
                        .game
                        .immutable
                        .lane_caches
                        .iter()
                        .enumerate()
                        .filter_map(|(l, cache)| {
                            cache
                                .object_caches
                                .last()
                                .map(|object| (l, object.start_position()))
                        })
                        .max_by_key(|(_, position)| *position)
                        .unwrap();

                    let difference = position - first_position;
                    let y = to_pixels(difference * state.scroll_speed);

                    let widget = state.objects[l].last().unwrap();
                    let last_object_height = widget.measure(orientation, lane_width).1;

                    let min_height = y + last_object_height;
                    trace!("returning height = {}", min_height);
                    (min_height, min_height, -1, -1)
                }
                _ => unimplemented!(),
            }
        }

        fn size_allocate(&self, _widget: &Self::Type, width: i32, height: i32, _baseline: i32) {
            trace!("PlitkiView::size_allocate({}, {})", width, height);

            let state = self.state().borrow();

            let lane_count: i32 = state.objects.len().try_into().unwrap();
            let lane_width = width / lane_count;
            let square_width = lane_width * lane_count;
            let to_pixels = |difference: ScreenPositionDifference| {
                (difference.0 as f64 / 2_000_000_000. * square_width as f64).round() as i32
            };

            let first_position = state.game.first_position().unwrap();

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
                    let y = to_pixels(difference * state.scroll_speed);

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

    impl PlitkiView {
        fn state(&self) -> &RefCell<State> {
            self.state
                .get()
                .expect("map property was not set during construction")
        }
    }
}

glib::wrapper! {
    pub struct PlitkiView(ObjectSubclass<imp::PlitkiView>)
        @extends gtk::Widget;
}

impl PlitkiView {
    pub(crate) fn new(map: Map) -> Self {
        glib::Object::new(&[("map", &BoxedMap(map))]).unwrap()
    }
}