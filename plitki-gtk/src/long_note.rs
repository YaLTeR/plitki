use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};
use plitki_core::scroll::ScreenPositionDifference;

mod imp {
    use std::cell::Cell;

    use gtk::{gdk, graphene};
    use log::trace;
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;
    use plitki_core::scroll::ScreenPositionDifference;

    use crate::utils::to_pixels;

    use super::*;

    #[derive(Debug, Default)]
    pub struct LongNote {
        head: OnceCell<gtk::Picture>,
        tail: OnceCell<gtk::Picture>,
        body: OnceCell<gtk::Picture>,
        length: Cell<ScreenPositionDifference>,
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

            self.body().set_parent(obj);
            self.tail().set_parent(obj);
            self.head().set_parent(obj);

            self.body().add_css_class("body");
            self.tail().add_css_class("tail");
            self.head().add_css_class("head");
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
                        "head",
                        "head",
                        gtk::Picture::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecObject::new(
                        "tail",
                        "tail",
                        "tail",
                        gtk::Picture::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecObject::new(
                        "body",
                        "body",
                        "body",
                        gtk::Picture::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecInt64::new(
                        "length",
                        "length",
                        "length",
                        0,
                        i64::MAX,
                        0,
                        glib::ParamFlags::READWRITE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "length" => self.length.get().0.to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "head" => {
                    let widget = value.get::<gtk::Picture>().expect("wrong property type");
                    self.head.set(widget).expect("property set more than once");
                }
                "tail" => {
                    let widget = value.get::<gtk::Picture>().expect("wrong property type");
                    self.tail.set(widget).expect("property set more than once");
                }
                "body" => {
                    let widget = value.get::<gtk::Picture>().expect("wrong property type");
                    self.body.set(widget).expect("property set more than once");
                }
                "length" => {
                    let length = value.get::<i64>().expect("wrong property type");
                    assert!(length >= 0);
                    if self.length.get().0 != length {
                        self.length.set(ScreenPositionDifference(length));
                        obj.queue_resize();
                    }
                }
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
            widget: &Self::Type,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            trace!("LongNote::measure({}, {})", orientation, for_size);

            // We only support can-shrink paintables which can always go down to zero, so our min
            // size is always zero.
            match orientation {
                gtk::Orientation::Horizontal => {
                    if for_size == -1 {
                        // We're basing our natural size on the head size.
                        let nat = self.head().measure(gtk::Orientation::Horizontal, -1).1;

                        trace!("returning for height = {}: nat width = {}", for_size, nat);
                        (0, nat, -1, -1)
                    } else if for_size == 0 {
                        // GtkPicture natural size for 0 isn't 0, so special-case it.
                        (0, 0, -1, -1)
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
                        // We're basing our natural size on the head size.
                        let width = self.head().measure(gtk::Orientation::Horizontal, -1).1;
                        // Compute our heights at that width.
                        self.measure(widget, gtk::Orientation::Vertical, width)
                    } else if for_size == 0 {
                        // GtkPicture natural size for 0 isn't 0, so special-case it.
                        (0, 0, -1, -1)
                    } else {
                        let width = for_size;

                        let nat_head = self.head().measure(gtk::Orientation::Vertical, width).1;

                        let length = to_pixels(self.length.get());
                        let tail_start = length;

                        let nat_tail = self.tail().measure(gtk::Orientation::Vertical, width).1;
                        // A very small tail can end up smaller than the rest of the head.
                        let nat = (tail_start + nat_tail).max(nat_head);

                        trace!("returning for width = {}: nat height = {}", for_size, nat);
                        (0, nat, -1, -1)
                    }
                }
                _ => unimplemented!(),
            }
        }

        fn size_allocate(&self, widget: &Self::Type, width: i32, height: i32, _baseline: i32) {
            trace!("LongNote::size_allocate({}, {})", width, height);

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

            let nat_head = self.head().measure(gtk::Orientation::Vertical, width).1;
            self.head()
                .size_allocate(&gdk::Rectangle::new(0, 0, width, nat_head), -1);

            let length = to_pixels(self.length.get());
            let tail_start = length;

            let nat_tail = self.tail().measure(gtk::Orientation::Vertical, width).1;
            self.tail()
                .size_allocate(&gdk::Rectangle::new(0, tail_start, width, nat_tail), -1);

            // Silence warning from GTK.
            let _ = self.body().measure(gtk::Orientation::Vertical, width);

            let body_start = nat_head / 2;
            let body_end = (length + nat_tail / 2).max(body_start);
            let body_height = body_end - body_start;

            self.body()
                .size_allocate(&gdk::Rectangle::new(0, body_start, width, body_height), -1);
        }

        fn snapshot(&self, widget: &Self::Type, snapshot: &gtk::Snapshot) {
            widget.snapshot_child(self.body(), snapshot);

            let bounds = self.tail().compute_bounds(widget).unwrap();
            snapshot.push_clip(&graphene::Rect::new(
                bounds.x(),
                (self.head().allocated_height() / 2) as f32,
                bounds.width(),
                bounds.y() + bounds.height(),
            ));
            widget.snapshot_child(self.tail(), snapshot);
            snapshot.pop();

            widget.snapshot_child(self.head(), snapshot);
        }
    }

    impl LongNote {
        fn head(&self) -> &gtk::Picture {
            self.head
                .get()
                .expect("property not set during construction")
        }

        fn tail(&self) -> &gtk::Picture {
            self.tail
                .get()
                .expect("property not set during construction")
        }

        fn body(&self) -> &gtk::Picture {
            self.body
                .get()
                .expect("property not set during construction")
        }
    }
}

glib::wrapper! {
    pub struct LongNote(ObjectSubclass<imp::LongNote>)
        @extends gtk::Widget;
}

impl LongNote {
    pub(crate) fn new(
        head: &gdk::Texture,
        tail: &gdk::Texture,
        body: &gdk::Texture,
        length: ScreenPositionDifference,
    ) -> Self {
        glib::Object::new(&[
            ("head", &gtk::Picture::for_paintable(head)),
            ("tail", &gtk::Picture::for_paintable(tail)),
            ("body", &{
                let picture = gtk::Picture::for_paintable(body);
                picture.set_keep_aspect_ratio(false);
                picture
            }),
            ("length", &length.0),
        ])
        .unwrap()
    }
}
