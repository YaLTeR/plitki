use gtk::glib;
use gtk::subclass::prelude::*;
use plitki_core::scroll::{Position, ScrollSpeed};

pub mod long_note;
pub mod regular_note;
pub mod timing_line;
pub mod widget;

use widget::ConveyorWidget;

mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;

    use gtk::prelude::*;
    use gtk::{graphene, gsk};
    use once_cell::sync::Lazy;
    use plitki_core::visibility_cache::VisibilityCache;
    use widget::ConveyorWidgetExt;

    use super::*;
    use crate::utils::to_pixels;

    #[derive(Debug)]
    struct Data {
        widgets: Vec<ConveyorWidget>,
        is_visible: Vec<bool>,
        was_visible: HashSet<usize>,
        cache: Option<(i32, VisibilityCache<i32>)>,
    }

    #[derive(Debug)]
    pub struct Conveyor {
        data: RefCell<Option<Data>>,
        scroll_speed: Cell<ScrollSpeed>,
        map_position: Cell<Position>,
        downscroll: Cell<bool>,
        hit_position: Cell<i32>,
    }

    impl Default for Conveyor {
        fn default() -> Self {
            Self {
                data: Default::default(),
                scroll_speed: Cell::new(ScrollSpeed(30)),
                map_position: Cell::new(Position::zero()),
                downscroll: Default::default(),
                hit_position: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Conveyor {
        const NAME: &'static str = "PlitkiConveyor";
        type Type = super::Conveyor;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Conveyor {
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
                    glib::ParamSpecUChar::builder("scroll-speed")
                        .default_value(30)
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecInt64::builder("map-position")
                        .minimum(Position::MIN.into())
                        .maximum(Position::MAX.into())
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
                "scroll-speed" => self.scroll_speed().0.to_value(),
                "map-position" => {
                    let value: i64 = self.map_position().into();
                    value.to_value()
                }
                "downscroll" => self.downscroll().to_value(),
                "hit-position" => self.hit_position().to_value(),
                _ => unreachable!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "scroll-speed" => self.set_scroll_speed(ScrollSpeed(value.get().unwrap())),
                "map-position" => {
                    let value: i64 = value.get().unwrap();
                    self.set_map_position(value.try_into().unwrap())
                }
                "downscroll" => self.set_downscroll(value.get().unwrap()),
                "hit-position" => self.set_hit_position(value.get().unwrap()),
                _ => unreachable!(),
            }
        }
    }

    impl WidgetImpl for Conveyor {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        #[instrument("Conveyor::measure", skip_all)]
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => {
                    let Some(data) = &mut *self.data.borrow_mut() else { return (0, 0, -1, -1) };

                    // Min and nat width for a lane is the maximum across objects.
                    let (min, nat) = data
                        .widgets
                        .iter()
                        .map(|widget| widget.measure(gtk::Orientation::Horizontal, -1))
                        .map(|(min_w, nat_w, _, _)| (min_w, nat_w))
                        .reduce(|(min, nat), (min_w, nat_w)| (min.max(min_w), nat.max(nat_w)))
                        // TODO: figure out better handling for empty lanes.
                        .unwrap_or((0, 0));

                    // TODO: update incrementally when only a single object updates.
                    data.cache = None;

                    (min, nat, -1, -1)
                }
                gtk::Orientation::Vertical => {
                    // Our height can always go down to 0.
                    (0, 0, -1, -1)
                }
                _ => unreachable!(),
            }
        }

        #[instrument("Conveyor::size_allocate", skip_all)]
        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let Some(data) = &mut *self.data.borrow_mut() else { return };

            let scroll_speed = self.scroll_speed.get();
            let downscroll = self.downscroll.get();
            let hit_position = self.hit_position.get();
            let map_position = self.map_position.get();

            // Invalidate the cache if our width changed.
            if matches!(data.cache, Some((cache_width, _)) if cache_width != width) {
                data.cache = None;
            }

            let cache = data.cache.get_or_insert_with(|| {
                let objects = data.widgets.iter().map(|widget| {
                    // We're using the width here, so we need to make sure to invalidate the cache
                    // on width changes.
                    let widget_height = widget.measure(gtk::Orientation::Vertical, width).1;
                    let position = widget.position();
                    let difference = position - Position::zero();
                    let y = to_pixels(difference * scroll_speed);

                    (y, y + widget_height)
                });
                (width, VisibilityCache::new(objects))
            });
            let cache = &cache.1;

            let mut visible = HashSet::new();
            let first_y =
                to_pixels((map_position - Position::zero()) * scroll_speed) - hit_position;
            for idx in cache.visible_objects(first_y..first_y + height) {
                let widget = &data.widgets[idx];

                if widget.is_hidden() {
                    continue;
                }

                visible.insert(idx);
                if !data.is_visible[idx] {
                    widget.set_child_visible(true);
                    data.is_visible[idx] = true;
                }

                let start_pos = cache.start_position(idx);
                let mut y = start_pos - first_y;
                let widget_height = cache.end_position(idx) - start_pos;

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

            // Hide widgets that were visible last time but are no longer visible.
            for &idx in data.was_visible.difference(&visible) {
                data.widgets[idx].set_child_visible(false);
                data.is_visible[idx] = false;
            }
            data.was_visible = visible;
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

    impl Conveyor {
        pub fn set_widgets(&self, widgets: Vec<ConveyorWidget>) {
            let obj = self.obj();

            // Set parent in reverse to get the right draw order.
            for widget in widgets.iter().rev() {
                // Default to invisible.
                widget.set_child_visible(false);
                widget.set_parent(&*obj);
            }

            let is_visible = vec![false; widgets.len()];

            let prev_data = self.data.replace(Some(Data {
                widgets,
                is_visible,
                was_visible: HashSet::new(),
                cache: None,
            }));
            if let Some(prev_data) = prev_data {
                for widget in prev_data.widgets {
                    widget.unparent();
                }
            }
        }

        pub fn scroll_speed(&self) -> ScrollSpeed {
            self.scroll_speed.get()
        }

        pub fn set_scroll_speed(&self, value: ScrollSpeed) {
            if self.scroll_speed.get() == value {
                return;
            }

            self.scroll_speed.set(value);
            if let Some(data) = &mut *self.data.borrow_mut() {
                // Scroll speed changes cause object positions to shift, invalidating the cahce.
                data.cache = None;
            }
            self.obj().queue_allocate();
            self.obj().notify("scroll-speed");
        }

        pub fn map_position(&self) -> Position {
            self.map_position.get()
        }

        pub fn set_map_position(&self, value: Position) {
            if self.map_position.get() == value {
                return;
            }

            assert!(value >= Position::MIN);
            assert!(value <= Position::MAX);

            self.map_position.set(value);
            self.obj().queue_allocate();
            self.obj().notify("map-position");
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
    pub struct Conveyor(ObjectSubclass<imp::Conveyor>)
        @extends gtk::Widget;
}

impl Conveyor {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn set_widgets(&self, widgets: Vec<ConveyorWidget>) {
        self.imp().set_widgets(widgets);
    }

    pub fn scroll_speed(&self) -> ScrollSpeed {
        self.imp().scroll_speed()
    }

    pub fn set_scroll_speed(&self, value: ScrollSpeed) {
        self.imp().set_scroll_speed(value);
    }

    pub fn map_position(&self) -> Position {
        self.imp().map_position()
    }

    pub fn set_map_position(&self, value: Position) {
        self.imp().set_map_position(value);
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

impl Default for Conveyor {
    fn default() -> Self {
        Self::new()
    }
}
