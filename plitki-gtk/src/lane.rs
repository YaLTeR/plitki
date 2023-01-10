use glib::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;
use plitki_core::scroll::{Position, ScrollSpeed};

use crate::conveyor::widget::ConveyorWidget;

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::gdk;
    use gtk::prelude::*;
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;

    use super::*;
    use crate::conveyor::Conveyor;

    #[derive(Debug)]
    pub struct Lane {
        conveyor: OnceCell<Conveyor>,
        below_hit_pos_widget: RefCell<Option<(gtk::Widget, glib::Binding)>>,
        above_hit_pos_widget: RefCell<Option<(gtk::Widget, glib::Binding)>>,
        scroll_speed: Cell<ScrollSpeed>,
        map_position: Cell<Position>,
        downscroll: Cell<bool>,
        hit_position: Cell<i32>,
    }

    impl Default for Lane {
        fn default() -> Self {
            Self {
                conveyor: Default::default(),
                below_hit_pos_widget: Default::default(),
                above_hit_pos_widget: Default::default(),
                scroll_speed: Cell::new(ScrollSpeed(30)),
                map_position: Cell::new(Position::zero()),
                downscroll: Default::default(),
                hit_position: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Lane {
        const NAME: &'static str = "PlitkiLane";
        type Type = super::Lane;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Lane {
        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();

            obj.set_overflow(gtk::Overflow::Hidden);

            let conveyor = Conveyor::new();
            conveyor.set_parent(&*obj);
            for name in ["scroll-speed", "map-position", "downscroll", "hit-position"] {
                obj.bind_property(name, &conveyor, name)
                    .sync_create()
                    .build();
            }
            self.conveyor.set(conveyor).unwrap();
        }

        fn dispose(&self) {
            self.conveyor.get().unwrap().unparent();

            if let Some((widget, binding)) = self.below_hit_pos_widget.take() {
                binding.unbind();
                widget.unparent();
            }

            if let Some((widget, binding)) = self.above_hit_pos_widget.take() {
                binding.unbind();
                widget.unparent();
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
                    glib::ParamSpecObject::builder::<gtk::Widget>("below-hit-pos-widget")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecObject::builder::<gtk::Widget>("above-hit-pos-widget")
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
                "below-hit-pos-widget" => self.below_hit_pos_widget().to_value(),
                "above-hit-pos-widget" => self.above_hit_pos_widget().to_value(),
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
                "below-hit-pos-widget" => self.set_below_hit_pos_widget(value.get().unwrap()),
                "above-hit-pos-widget" => self.set_above_hit_pos_widget(value.get().unwrap()),
                _ => unreachable!(),
            }
        }
    }

    impl WidgetImpl for Lane {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => {
                    let conveyor = &self.conveyor.get().unwrap();
                    let (mut min, mut nat, _, _) =
                        conveyor.measure(gtk::Orientation::Horizontal, -1);

                    if let Some((widget, _)) = &*self.below_hit_pos_widget.borrow() {
                        let (min_w, nat_w, _, _) = widget.measure(gtk::Orientation::Horizontal, -1);
                        min = min.max(min_w);
                        nat = nat.max(nat_w);
                    }

                    if let Some((widget, _)) = &*self.above_hit_pos_widget.borrow() {
                        let (min_w, nat_w, _, _) = widget.measure(gtk::Orientation::Horizontal, -1);
                        min = min.max(min_w);
                        nat = nat.max(nat_w);
                    }

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
            let downscroll = self.downscroll.get();
            let hit_position = self.hit_position.get();

            let conveyor = self.conveyor.get().unwrap();
            let conveyor_height = conveyor.measure(gtk::Orientation::Vertical, width).0;
            conveyor.size_allocate(
                &gdk::Rectangle::new(0, 0, width, height.max(conveyor_height)),
                -1,
            );

            if let Some((widget, _)) = &*self.below_hit_pos_widget.borrow() {
                let widget_height = widget.measure(gtk::Orientation::Vertical, width).1;

                let mut y = hit_position - widget_height;
                if downscroll {
                    y = height - y - widget_height;
                }

                widget.size_allocate(&gdk::Rectangle::new(0, y, width, widget_height), -1);
            }

            if let Some((widget, _)) = &*self.above_hit_pos_widget.borrow() {
                let widget_height = widget.measure(gtk::Orientation::Vertical, width).1;

                let mut y = hit_position;
                if downscroll {
                    y = height - y - widget_height;
                }

                widget.size_allocate(&gdk::Rectangle::new(0, y, width, widget_height), -1);
            }
        }
    }

    impl Lane {
        pub fn scroll_speed(&self) -> ScrollSpeed {
            self.scroll_speed.get()
        }

        pub fn set_scroll_speed(&self, value: ScrollSpeed) {
            if self.scroll_speed.get() == value {
                return;
            }

            self.scroll_speed.set(value);
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

            self.hit_position.set(value);
            self.obj().notify("hit-position");
            self.obj().queue_allocate();
        }

        pub fn below_hit_pos_widget(&self) -> Option<gtk::Widget> {
            self.below_hit_pos_widget
                .borrow()
                .as_ref()
                .map(|(widget, _)| widget.clone())
        }

        pub fn set_below_hit_pos_widget(&self, value: Option<gtk::Widget>) {
            if self
                .below_hit_pos_widget
                .borrow()
                .as_ref()
                .map(|(widget, _)| widget)
                == value.as_ref()
            {
                return;
            }

            let obj = self.obj();

            let value = value.map(|widget| {
                widget.set_parent(&*obj);
                let binding = obj
                    .bind_property("downscroll", &widget, "downscroll")
                    .sync_create()
                    .build();
                (widget, binding)
            });

            if let Some((old_widget, old_binding)) = self.below_hit_pos_widget.replace(value) {
                old_binding.unbind();
                old_widget.unparent();
            }

            obj.queue_resize();
            obj.notify("below-hit-pos-widget");
        }

        pub fn above_hit_pos_widget(&self) -> Option<gtk::Widget> {
            self.above_hit_pos_widget
                .borrow()
                .as_ref()
                .map(|(widget, _)| widget.clone())
        }

        pub fn set_above_hit_pos_widget(&self, value: Option<gtk::Widget>) {
            if self
                .above_hit_pos_widget
                .borrow()
                .as_ref()
                .map(|(widget, _)| widget)
                == value.as_ref()
            {
                return;
            }

            let obj = self.obj();

            let value = value.map(|widget| {
                widget.set_parent(&*obj);
                let binding = obj
                    .bind_property("downscroll", &widget, "downscroll")
                    .sync_create()
                    .build();
                (widget, binding)
            });

            if let Some((old_widget, old_binding)) = self.above_hit_pos_widget.replace(value) {
                old_binding.unbind();
                old_widget.unparent();
            }

            obj.queue_resize();
            obj.notify("above-hit-pos-widget");
        }

        pub fn set_notes(&self, notes: Vec<ConveyorWidget>) {
            self.conveyor.get().unwrap().set_widgets(notes);
        }
    }
}

glib::wrapper! {
    pub struct Lane(ObjectSubclass<imp::Lane>)
        @extends gtk::Widget;
}

impl Lane {
    pub fn new() -> Self {
        glib::Object::builder().build()
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

    pub fn below_hit_pos_widget(&self) -> Option<gtk::Widget> {
        self.imp().below_hit_pos_widget()
    }

    pub fn set_below_hit_pos_widget(&self, value: Option<&impl IsA<gtk::Widget>>) {
        self.imp()
            .set_below_hit_pos_widget(value.map(|w| w.clone().upcast()));
    }

    pub fn above_hit_pos_widget(&self) -> Option<gtk::Widget> {
        self.imp().above_hit_pos_widget()
    }

    pub fn set_above_hit_pos_widget(&self, value: Option<&impl IsA<gtk::Widget>>) {
        self.imp()
            .set_above_hit_pos_widget(value.map(|w| w.clone().upcast()));
    }

    pub fn set_notes(&self, notes: Vec<ConveyorWidget>) {
        self.imp().set_notes(notes);
    }
}

impl Default for Lane {
    fn default() -> Self {
        Self::new()
    }
}
