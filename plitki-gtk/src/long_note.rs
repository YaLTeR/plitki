use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};
use plitki_core::scroll::ScreenPositionDifference;

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::{gdk, graphene};
    use log::trace;
    use once_cell::sync::Lazy;
    use plitki_core::scroll::ScreenPositionDifference;

    use crate::utils::to_pixels;

    use super::*;

    #[derive(Debug)]
    pub struct LongNote {
        head: RefCell<gtk::Widget>,
        tail: RefCell<gtk::Widget>,
        body: RefCell<gtk::Widget>,
        length: Cell<ScreenPositionDifference>,
    }

    impl Default for LongNote {
        fn default() -> Self {
            Self {
                head: RefCell::new(gtk::Picture::new().upcast()),
                tail: RefCell::new(gtk::Picture::new().upcast()),
                body: RefCell::new(gtk::Picture::new().upcast()),
                length: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LongNote {
        const NAME: &'static str = "PlitkiLongNote";
        type Type = super::LongNote;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("plitki-long-note");
        }
    }

    impl ObjectImpl for LongNote {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            // Set parent in case the properties weren't set during construction.
            self.head.borrow().set_parent(obj);
            self.tail.borrow().set_parent(obj);
            self.body.borrow().set_parent(obj);
        }

        fn dispose(&self, obj: &Self::Type) {
            while let Some(child) = obj.first_child() {
                child.unparent();
            }
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::new(
                        "head",
                        "",
                        "",
                        gtk::Widget::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecObject::new(
                        "tail",
                        "",
                        "",
                        gtk::Widget::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecObject::new(
                        "body",
                        "",
                        "",
                        gtk::Widget::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecInt64::new(
                        "length",
                        "",
                        "",
                        0,
                        i64::MAX,
                        0,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "head" => self.head().to_value(),
                "tail" => self.tail().to_value(),
                "body" => self.body().to_value(),
                "length" => self.length().0.to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "head" => self.set_head(value.get().unwrap()),
                "tail" => self.set_tail(value.get().unwrap()),
                "body" => self.set_body(value.get().unwrap()),
                "length" => self.set_length(ScreenPositionDifference(value.get().unwrap())),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for LongNote {
        fn request_mode(&self, _widget: &Self::Type) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
            // gtk::SizeRequestMode::WidthForHeight
        }

        fn measure(
            &self,
            obj: &Self::Type,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            trace!("LongNote::measure({}, {})", orientation, for_size);

            let head = self.head.borrow();
            let head = &*head;
            let tail = self.tail.borrow();
            let tail = &*tail;

            match orientation {
                gtk::Orientation::Horizontal => {
                    if for_size == -1 {
                        // Global min and nat width are maximum min and nat width across components,
                        // except body, which is hidden when it doesn't fit.
                        let (min, nat) =
                            [head, tail].into_iter().fold((0, 0), |(min, nat), widget| {
                                let (min_w, nat_w, _, _) =
                                    widget.measure(gtk::Orientation::Horizontal, -1);
                                (min.max(min_w), nat.max(nat_w))
                            });

                        trace!("returning global min width = {min}, nat = {nat}");
                        (min, nat, -1, -1)
                    } else {
                        let height_to_fit = for_size;

                        // Use the measure property: when width increases, min height can stay or
                        // decrease. Loop from min_width up until we fit the given height.
                        let min_width = self.measure(obj, gtk::Orientation::Horizontal, -1).0;
                        let min = (min_width..)
                            .find(|&width| self.height_for_width(width).0 <= height_to_fit)
                            .unwrap();

                        // Natural width for this height = min width that can fit it.
                        let nat = min;

                        trace!("returning for height = {for_size}: min width = {min}, nat = {nat}");
                        (min, nat, -1, -1)
                    }
                }
                gtk::Orientation::Vertical => {
                    // Even though the length does not depend on the width, the height does, because
                    // the LN tail (and possibly the head) sticks out past the length.
                    if for_size == -1 {
                        // Global min height is the min height for min width that gives the min
                        // height for the head and tail.
                        let width = [head, tail]
                            .into_iter()
                            .map(|widget| {
                                let min_height = widget.measure(gtk::Orientation::Vertical, -1).0;
                                widget.measure(gtk::Orientation::Horizontal, min_height).0
                            })
                            .max()
                            .unwrap();

                        let min = self.height_for_width(width).0;

                        // Global nat height is the nat height for global nat width.
                        let nat_width = self.measure(obj, gtk::Orientation::Horizontal, -1).1;
                        let nat = self.height_for_width(nat_width).1;

                        trace!("returning global min height = {min}, nat = {nat}");
                        (min, nat, -1, -1)
                    } else {
                        let width_to_fit = for_size;

                        // Loop over all widths and pick the lowest min height we can get.
                        let min_width = self.measure(obj, gtk::Orientation::Horizontal, -1).0;
                        let min = (min_width..=width_to_fit)
                            .map(|width| self.height_for_width(width).0)
                            .min()
                            .unwrap();

                        let nat = self.height_for_width(width_to_fit).1;

                        trace!("returning for width = {for_size}: min height = {min}, nat = {nat}");
                        (min, nat, -1, -1)
                    }
                }
                _ => unimplemented!(),
            }
        }

        fn size_allocate(&self, obj: &Self::Type, width: i32, height: i32, _baseline: i32) {
            trace!("LongNote::size_allocate({}, {})", width, height);

            let head = self.head.borrow();
            let head = &*head;
            let tail = self.tail.borrow();
            let tail = &*tail;
            let body = self.body.borrow();
            let body = &*body;
            let length = to_pixels(self.length.get());

            // We really want to allocate natural heights. Find the largest width that we can use
            // within the given allocation that still lets us use natural heights. Otherwise bail
            // out.
            let min_width = self.measure(obj, gtk::Orientation::Horizontal, -1).0;
            let width = match (min_width..=width)
                .rev()
                .find(|&width| self.height_for_width(width).1 <= height)
            {
                Some(x) => x,
                None => {
                    head.set_child_visible(false);
                    tail.set_child_visible(false);
                    body.set_child_visible(false);
                    return;
                }
            };

            head.set_child_visible(true);
            tail.set_child_visible(true);

            // Allocate the head.
            let nat_head = head.measure(gtk::Orientation::Vertical, width).1;
            head.size_allocate(&gdk::Rectangle::new(0, 0, width, nat_head), -1);

            // Allocate the tail.
            let tail_start = length;
            let nat_tail = tail.measure(gtk::Orientation::Vertical, width).1;
            tail.size_allocate(&gdk::Rectangle::new(0, tail_start, width, nat_tail), -1);

            // Allocate the body, if it fits.
            let body_start = nat_head / 2;
            let body_end = (length + nat_tail / 2).max(body_start);
            let body_height = body_end - body_start;

            let body_min_height = body.measure(gtk::Orientation::Vertical, width).0;
            if body_height >= body_min_height {
                let body_min_width = body.measure(gtk::Orientation::Horizontal, body_height).0;
                if width >= body_min_width {
                    body.set_child_visible(true);
                    body.size_allocate(&gdk::Rectangle::new(0, body_start, width, body_height), -1);
                } else {
                    body.set_child_visible(false);
                }
            } else {
                body.set_child_visible(false);
            }
        }

        fn snapshot(&self, widget: &Self::Type, snapshot: &gtk::Snapshot) {
            let head = self.head.borrow();
            let head = &*head;
            let tail = self.tail.borrow();
            let tail = &*tail;
            let body = self.body.borrow();
            let body = &*body;

            if body.is_child_visible() {
                widget.snapshot_child(body, snapshot);
            }

            let bounds = tail.compute_bounds(widget).unwrap();
            snapshot.push_clip(&graphene::Rect::new(
                bounds.x(),
                (head.allocated_height() / 2) as f32,
                bounds.width(),
                bounds.y() + bounds.height(),
            ));
            widget.snapshot_child(tail, snapshot);
            snapshot.pop();

            widget.snapshot_child(head, snapshot);
        }
    }

    impl LongNote {
        pub fn head(&self) -> gtk::Widget {
            self.head.borrow().clone()
        }

        pub fn tail(&self) -> gtk::Widget {
            self.tail.borrow().clone()
        }

        pub fn body(&self) -> gtk::Widget {
            self.body.borrow().clone()
        }

        pub fn set_head(&self, widget: Option<gtk::Widget>) {
            let widget = widget.unwrap_or_else(|| gtk::Picture::new().upcast());

            if *self.head.borrow() == widget {
                return;
            }

            let obj = self.instance();

            widget.add_css_class("head");
            widget.set_parent(&obj);

            let old_widget = self.head.replace(widget);
            old_widget.unparent();
            old_widget.remove_css_class("head");

            obj.notify("head");
            obj.queue_resize();
        }

        pub fn set_tail(&self, widget: Option<gtk::Widget>) {
            let widget = widget.unwrap_or_else(|| gtk::Picture::new().upcast());

            if *self.tail.borrow() == widget {
                return;
            }

            let obj = self.instance();

            widget.add_css_class("tail");
            widget.set_parent(&obj);

            let old_widget = self.tail.replace(widget);
            old_widget.unparent();
            old_widget.remove_css_class("tail");

            obj.notify("tail");
            obj.queue_resize();
        }

        pub fn set_body(&self, widget: Option<gtk::Widget>) {
            let widget = widget.unwrap_or_else(|| gtk::Picture::new().upcast());

            if *self.body.borrow() == widget {
                return;
            }

            let obj = self.instance();

            widget.add_css_class("body");
            widget.set_parent(&obj);

            let old_widget = self.body.replace(widget);
            old_widget.unparent();
            old_widget.remove_css_class("body");

            obj.notify("body");
            obj.queue_resize();
        }

        pub fn length(&self) -> ScreenPositionDifference {
            self.length.get()
        }

        pub fn set_length(&self, length: ScreenPositionDifference) {
            if self.length.get().0 != length.0 {
                self.length.set(length);

                let obj = self.instance();
                obj.notify("length");
                obj.queue_resize();
            }
        }

        fn height_for_width(&self, width: i32) -> (i32, i32) {
            let head = self.head.borrow();
            let head = &*head;
            let tail = self.tail.borrow();
            let tail = &*tail;
            let length = to_pixels(self.length.get());

            let (min_head, nat_head, _, _) = head.measure(gtk::Orientation::Vertical, width);
            let (min_tail, nat_tail, _, _) = tail.measure(gtk::Orientation::Vertical, width);

            let tail_start = length;

            // Body will be hidden if it doesn't fit, so no need to consider it.
            (
                min_head.max(tail_start + min_tail),
                nat_head.max(tail_start + nat_tail),
            )
        }
    }
}

glib::wrapper! {
    pub struct LongNote(ObjectSubclass<imp::LongNote>)
        @extends gtk::Widget;
}

impl LongNote {
    pub fn new() -> Self {
        glib::Object::new(&[]).unwrap()
    }

    pub fn with_paintables(
        head: &impl IsA<gdk::Paintable>,
        tail: &impl IsA<gdk::Paintable>,
        body: &impl IsA<gdk::Paintable>,
    ) -> Self {
        glib::Object::new(&[
            ("head", &gtk::Picture::for_paintable(head)),
            ("tail", &gtk::Picture::for_paintable(tail)),
            ("body", &{
                let picture = gtk::Picture::for_paintable(body);
                picture.set_keep_aspect_ratio(false);
                picture
            }),
        ])
        .unwrap()
    }

    pub fn with_widgets(
        head: &impl IsA<gtk::Widget>,
        tail: &impl IsA<gtk::Widget>,
        body: &impl IsA<gtk::Widget>,
    ) -> Self {
        glib::Object::new(&[("head", head), ("tail", tail), ("body", body)]).unwrap()
    }

    pub fn head(&self) -> gtk::Widget {
        self.imp().head()
    }

    pub fn tail(&self) -> gtk::Widget {
        self.imp().tail()
    }

    pub fn body(&self) -> gtk::Widget {
        self.imp().body()
    }

    pub fn set_head(&self, value: Option<impl IsA<gtk::Widget>>) {
        self.imp().set_head(value.map(|w| w.upcast()));
    }

    pub fn set_tail(&self, value: Option<impl IsA<gtk::Widget>>) {
        self.imp().set_tail(value.map(|w| w.upcast()));
    }

    pub fn set_body(&self, value: Option<impl IsA<gtk::Widget>>) {
        self.imp().set_body(value.map(|w| w.upcast()));
    }

    pub fn set_head_paintable(&self, value: Option<&impl IsA<gdk::Paintable>>) {
        self.set_head(value.map(|p| gtk::Picture::for_paintable(p)));
    }

    pub fn set_tail_paintable(&self, value: Option<&impl IsA<gdk::Paintable>>) {
        self.set_tail(value.map(|p| gtk::Picture::for_paintable(p)));
    }

    pub fn set_body_paintable(&self, value: Option<&impl IsA<gdk::Paintable>>) {
        self.set_body(value.map(|p| {
            let picture = gtk::Picture::for_paintable(p);
            picture.set_keep_aspect_ratio(false);
            picture
        }));
    }

    pub fn length(&self) -> ScreenPositionDifference {
        self.imp().length()
    }

    pub fn set_length(&self, length: ScreenPositionDifference) {
        self.imp().set_length(length);
    }
}

impl Default for LongNote {
    fn default() -> Self {
        Self::new()
    }
}
