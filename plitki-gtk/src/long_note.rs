use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use gtk::{graphene, gsk};
    use log::trace;
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct LongNote {
        head: OnceCell<gtk::Widget>,
        tail: OnceCell<gtk::Widget>,
        body: OnceCell<gtk::Widget>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LongNote {
        const NAME: &'static str = "LongNote";
        type Type = super::LongNote;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for LongNote {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.body().set_parent(obj);
            self.tail().set_parent(obj);
            self.head().set_parent(obj);
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
                ]
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
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for LongNote {
        fn request_mode(&self, _widget: &Self::Type) -> gtk::SizeRequestMode {
            unimplemented!()
        }

        fn measure(
            &self,
            _widget: &Self::Type,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            trace!("LongNote::measure({}, {})", orientation, for_size);
            unimplemented!()
        }

        fn size_allocate(&self, _widget: &Self::Type, width: i32, height: i32, _baseline: i32) {
            trace!("LongNote::size_allocate({}, {})", width, height);

            let head_height = self.head().measure(gtk::Orientation::Vertical, width).1;
            let tail_height = self.tail().measure(gtk::Orientation::Vertical, width).1;
            let body_height = height - head_height / 2 + tail_height / 2;

            // Overallocate so the tail ends up above.
            if body_height > 0 {
                self.body().set_child_visible(true);
                self.body().allocate(
                    width,
                    body_height,
                    -1,
                    Some(
                        &gsk::Transform::new()
                            .translate(&graphene::Point::new(
                                width as f32,
                                (height + tail_height / 2) as f32,
                            ))
                            .unwrap()
                            .rotate(180.)
                            .unwrap(),
                    ),
                );
            } else {
                self.body().set_child_visible(false);
            }

            self.tail().allocate(
                width,
                tail_height,
                -1,
                Some(
                    &gsk::Transform::new()
                        .translate(&graphene::Point::new(
                            width as f32,
                            (height + tail_height) as f32,
                        ))
                        .unwrap()
                        .rotate(180.)
                        .unwrap(),
                ),
            );

            self.head().allocate(
                width,
                head_height,
                -1,
                Some(
                    &gsk::Transform::new()
                        .translate(&graphene::Point::new(width as f32, head_height as f32))
                        .unwrap()
                        .rotate(180.)
                        .unwrap(),
                ),
            );
        }
    }

    impl LongNote {
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
    pub struct LongNote(ObjectSubclass<imp::LongNote>)
        @extends gtk::Widget;
}

impl LongNote {
    pub(crate) fn new(head: &gtk::Widget, tail: &gtk::Widget, body: &gtk::Widget) -> Self {
        glib::Object::new(&[("head", head), ("tail", tail), ("body", body)]).unwrap()
    }
}
