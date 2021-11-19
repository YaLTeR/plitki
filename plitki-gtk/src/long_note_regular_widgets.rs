//! This is a LongNote that can (sort of) work with regular widgets. It's limited in that it always
//! tries to allocate only the minimum size. It's also probably broken in a few different ways.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use std::cell::Cell;
    use std::cmp::{max, min};

    use gtk::{gdk, graphene, gsk};
    use log::trace;
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;
    use plitki_core::scroll::ScreenPositionDifference;

    use super::*;

    #[derive(Debug)]
    pub struct LongNoteRegularWidgets {
        head: OnceCell<gtk::Widget>,
        tail: OnceCell<gtk::Widget>,
        body: OnceCell<gtk::Widget>,
        length: Cell<ScreenPositionDifference>,
        lane_count: Cell<u8>,
    }

    impl Default for LongNoteRegularWidgets {
        fn default() -> Self {
            Self {
                head: Default::default(),
                tail: Default::default(),
                body: Default::default(),
                length: Default::default(),
                lane_count: Cell::new(1),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LongNoteRegularWidgets {
        const NAME: &'static str = "LongNoteRegularWidgets";
        type Type = super::LongNoteRegularWidgets;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("long-note");
        }
    }

    impl ObjectImpl for LongNoteRegularWidgets {
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
                    glib::ParamSpec::new_object(
                        "head",
                        "head",
                        "head",
                        gtk::Widget::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpec::new_object(
                        "tail",
                        "tail",
                        "tail",
                        gtk::Widget::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpec::new_object(
                        "body",
                        "body",
                        "body",
                        gtk::Widget::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpec::new_int64(
                        "length",
                        "length",
                        "length",
                        i64::MIN,
                        i64::MAX,
                        0,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_uchar(
                        "lane-count",
                        "lane-count",
                        "lane-count",
                        1,
                        u8::MAX,
                        1,
                        glib::ParamFlags::READWRITE,
                    ),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "length" => self.length.get().0.to_value(),
                "lane-count" => self.lane_count.get().to_value(),
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
                    let widget = value.get::<gtk::Widget>().expect("wrong property type");
                    self.head.set(widget).expect("property set more than once");
                }
                "tail" => {
                    let widget = value.get::<gtk::Widget>().expect("wrong property type");
                    self.tail.set(widget).expect("property set more than once");
                }
                "body" => {
                    let widget = value.get::<gtk::Widget>().expect("wrong property type");
                    self.body.set(widget).expect("property set more than once");
                }
                "length" => {
                    let length = value.get::<i64>().expect("wrong property type");
                    if self.length.get().0 != length {
                        self.length.set(ScreenPositionDifference(length));
                        obj.queue_resize();
                    }
                }
                "lane-count" => {
                    let lane_count = value.get::<u8>().expect("wrong property type");
                    assert!(lane_count >= 1);
                    if self.lane_count.get() != lane_count {
                        self.lane_count.set(lane_count);
                        obj.queue_resize();
                    }
                }
                _ => unimplemented!(),
            }
        }
    }

    fn to_pixels(length: ScreenPositionDifference, lane_width: i32, lane_count: u8) -> i32 {
        let lane_count: i32 = lane_count.into();
        let playfield_width = lane_width * lane_count;
        (length.0 as f64 / 2_000_000_000. * playfield_width as f64).round() as i32
    }

    impl WidgetImpl for LongNoteRegularWidgets {
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
            log::debug!("LongNoteRegularWidgets::measure({}, {})", orientation, for_size);

            match orientation {
                gtk::Orientation::Horizontal => {
                    if for_size == -1 {
                        // To get the absolute minimum width, just take a maximum of minimum widths
                        // of all components.
                        let (min_head, nat_head, _, _) = self.head().measure(gtk::Orientation::Horizontal, -1);
                        let (min_tail, nat_tail, _, _) = self.tail().measure(gtk::Orientation::Horizontal, -1);
                        let (min_body, nat_body, _, _) = self.body().measure(gtk::Orientation::Horizontal, -1);

                        let min = min_head.max(min_tail).max(min_body);

                        // let nat = if for_size == -1 {
                        //     nat_head.max(nat_tail).max(nat_body)
                        // } else {
                        //     nat_head.max(nat_tail).max(nat_body)

                        // };
                        let nat = min;
                        log::debug!("returning for height = {}: min width = {}, nat = {}", for_size, min, nat);
                        (min, nat, -1, -1)
                    } else {
                        let height_to_fit = for_size;

                        // Sanity check for the input so we don't infinite loop.
                        let min_height = self.measure(widget, gtk::Orientation::Vertical, -1).0;
                        if height_to_fit < min_height {
                            panic!("trying to measure for height = {} < minimum height = {}", height_to_fit, min_height);
                        }

                        // Try all widths until we find one that fits this height.
                        let min_width = self.measure(widget, gtk::Orientation::Horizontal, -1).0;
                        for width in min_width.. {
                            let height = self.measure(widget, gtk::Orientation::Vertical, width).0;
                            if height <= height_to_fit {
                                let min = width;
                                let nat = min;
                                log::debug!("returning for height = {}: min width = {}, nat = {}", for_size, min, nat);
                                return (min, nat, -1, -1);
                            }
                        }
                        unreachable!()
                    }
                }
                gtk::Orientation::Vertical => {
                    if for_size == -1 {
                        // We can't actually know what width will get us minimum height because as
                        // width increases height can decrease, then start increasing again...
                        //
                        // HE        HEAD         HEAD
                        // AD   ->   body   ->   -body-
                        // TA        TAIL        -body-
                        // IL                     TAIL
                        //
                        // What we can do is compute our min-height for min-width for min-height of
                        // child widgets...
                        let (min_height_head, _, _, _) =
                            self.head().measure(gtk::Orientation::Vertical, -1);
                        let (min_height_tail, _, _, _) =
                            self.tail().measure(gtk::Orientation::Vertical, -1);
                        let (min_height_body, _, _, _) =
                            self.body().measure(gtk::Orientation::Vertical, -1);

                        let (min_width_for_min_height_head, _, _, _) = self
                            .head()
                            .measure(gtk::Orientation::Horizontal, min_height_head);
                        let (min_width_for_min_height_tail, _, _, _) = self
                            .tail()
                            .measure(gtk::Orientation::Horizontal, min_height_tail);
                        let (min_width_for_min_height_body, _, _, _) = self
                            .body()
                            .measure(gtk::Orientation::Horizontal, min_height_body);

                        let width_for_min_height = min_width_for_min_height_head
                            .max(min_width_for_min_height_tail)
                            .max(min_width_for_min_height_body);
                        self.measure(widget, gtk::Orientation::Vertical, width_for_min_height)
                        // TODO nat
                    } else {
                        // Our height-for-width might be non-monotonic: the minimum height might be
                        // using this width, or using some smaller width.
                        let min_width = self.measure(widget, gtk::Orientation::Horizontal, -1).0;
                        let mut global_min = i32::MAX;
                        let mut global_min_width = 0;
                        for width in min_width..=for_size {
                            let (min_head, _, _, _) =
                                self.head().measure(gtk::Orientation::Vertical, width);
                            let (min_tail, _, _, _) =
                                self.tail().measure(gtk::Orientation::Vertical, width);
                            let (min_body, _, _, _) =
                                self.body().measure(gtk::Orientation::Vertical, width);

                            let length = to_pixels(self.length.get(), width, self.lane_count.get());
                            assert!(length >= 0, "negative lengths are TODO");

                            let min_body_end = max(length + min_tail / 2, min_head / 2 + min_body);
                            let min_height = max(min_head, min_body_end - min_tail / 2 + min_tail);
                            if min_height <= global_min {
                                global_min_width = width;
                            }
                            global_min = min(global_min, min_height);
                        }
                        log::debug!("measured min height = {} at width = {}", global_min, global_min_width);

                        // TODO nat
                        let min = global_min;
                        let nat = global_min;

                        log::debug!("returning for width = {}: min height = {}, nat = {}", for_size, min, nat);
                        (min, nat, -1, -1)
                    }
                }
                _ => unimplemented!(),
            }
        }

        fn size_allocate(&self, widget: &Self::Type, width: i32, height: i32, _baseline: i32) {
            log::debug!("LongNoteRegularWidgets::size_allocate({}, {})", width, height);

            let min_width = self.measure(widget, gtk::Orientation::Horizontal, -1).0;

            // log::debug!("Running measure test from width = {} to {}...", min_width, min_width + 100);
            // let mut h = self.measure(widget, gtk::Orientation::Vertical, min_width).0;
            // for w in min_width + 1..=min_width + 100 {
            //     let new_h = self.measure(widget, gtk::Orientation::Vertical, w).0;
            //     assert!(new_h <= h, "height for width {} = {} which is > previous height for width = {}", w, new_h, h);
            //     h = new_h;
            // }

            // let min_height = self.measure(widget, gtk::Orientation::Vertical, -1).0;
            // log::debug!("Running measure test from height = {} to {}...", min_height, min_height + 100);
            // let mut w = self.measure(widget, gtk::Orientation::Horizontal, min_height).0;
            // for h in min_height + 1..=min_height + 100 {
            //     let new_w = self.measure(widget, gtk::Orientation::Horizontal, h).0;
            //     assert!(new_w <= w, "height for width {} = {} which is > previous height for width = {}", h, new_w, w);
            //     w = new_w;
            // }

            // We know that there's some width <= given where the widget fits. Let's find it.
            // If we don't find it then we got underallocated, just use min_width in that case.
            let mut fitting_width = min_width;
            for width in (min_width..=width).rev() {
                let (min_head, _, _, _) = self.head().measure(gtk::Orientation::Vertical, width);
                let (min_tail, _, _, _) = self.tail().measure(gtk::Orientation::Vertical, width);
                let (min_body, _, _, _) = self.body().measure(gtk::Orientation::Vertical, width);

                let length = to_pixels(self.length.get(), width, self.lane_count.get());
                assert!(length >= 0, "negative lengths are TODO");

                let min_body_end = max(length + min_tail / 2, min_head / 2 + min_body);
                let min_height = max(min_head, min_body_end - min_tail / 2 + min_tail);
                if min_height <= height {
                    fitting_width = width;
                    break;
                }
            }

            let width = fitting_width;
            let (min_head, nat_head, _, _) = self.head().measure(gtk::Orientation::Vertical, width);
            let (min_tail, _, _, _) = self.tail().measure(gtk::Orientation::Vertical, width);
            let (min_body, _, _, _) = self.body().measure(gtk::Orientation::Vertical, width);

            // Number of pixels between start of head and start of tail.
            let length = to_pixels(self.length.get(), width, self.lane_count.get());
            assert!(length >= 0, "negative lengths are TODO");

            let min_body_start = min_head / 2;
            let min_body_end = max(length + min_tail / 2, min_body_start + min_body);

            let min_body_start_for_nat_head = nat_head / 2;
            let min_body_end_for_nat_head = max(length + min_tail / 2, min_body_start_for_nat_head + min_body);

            let (head_height, body_start) = if min_body_end == min_body_end_for_nat_head {
                (nat_head, min_body_start_for_nat_head)
            } else {
                (min_head, min_body_start)
            };

            let body_end = min_body_end;
            let body_height = body_end - body_start;
            let min_tail_start = body_end - min_tail / 2;

            // let nat_body_end = max(length + nat_tail / 2, nat_head / 2 + min_body);
            // let nat = max(nat_head, nat_body_end - nat_tail / 2 + nat_tail);

            self.head().size_allocate(
                &gdk::Rectangle {
                    x: 0,
                    y: 0,
                    width: fitting_width,
                    height: head_height,
                },
                -1,
            );

            self.body().size_allocate(
                &gdk::Rectangle {
                    x: 0,
                    y: body_start,
                    width: fitting_width,
                    height: body_height,
                },
                -1,
            );

            self.tail().size_allocate(
                &gdk::Rectangle {
                    x: 0,
                    y: min_tail_start,
                    width: fitting_width,
                    height: min_tail,
                },
                -1,
            );
        }
    }

    impl LongNoteRegularWidgets {
        fn head(&self) -> &gtk::Widget {
            self.head
                .get()
                .expect("property not set during construction")
        }

        fn tail(&self) -> &gtk::Widget {
            self.tail
                .get()
                .expect("property not set during construction")
        }

        fn body(&self) -> &gtk::Widget {
            self.body
                .get()
                .expect("property not set during construction")
        }
    }
}

glib::wrapper! {
    pub struct LongNoteRegularWidgets(ObjectSubclass<imp::LongNoteRegularWidgets>)
        @extends gtk::Widget;
}

impl LongNoteRegularWidgets {
    pub(crate) fn new(head: &gtk::Widget, tail: &gtk::Widget, body: &gtk::Widget) -> Self {
        glib::Object::new(&[("head", head), ("tail", tail), ("body", body)]).unwrap()
    }
}
