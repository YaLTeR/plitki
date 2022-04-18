use glib::IsA;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};

mod imp {
    use once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};

    use gtk::graphene;

    use super::*;

    #[derive(Debug)]
    pub struct Background {
        paintable: RefCell<gdk::Paintable>,
        dim: Cell<f32>,
    }

    impl Default for Background {
        fn default() -> Self {
            Self {
                paintable: RefCell::new(gdk::Paintable::new_empty(0, 0)),
                dim: Cell::new(0.),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Background {
        const NAME: &'static str = "PlitkiBackground";
        type Type = super::Background;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("plitki-background");
        }
    }

    impl ObjectImpl for Background {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.set_overflow(gtk::Overflow::Hidden);
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::new(
                        "paintable",
                        "",
                        "",
                        gdk::Paintable::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecFloat::new(
                        "dim",
                        "",
                        "",
                        0.,
                        1.,
                        0.,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
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
                "paintable" => self.set_paintable(value.get().unwrap()),
                "dim" => self.set_dim(value.get().unwrap()),
                _ => unreachable!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "paintable" => self.paintable.borrow().clone().to_value(),
                "dim" => self.dim.get().to_value(),
                _ => unreachable!(),
            }
        }
    }

    impl WidgetImpl for Background {
        fn request_mode(&self, _widget: &Self::Type) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(
            &self,
            _widget: &Self::Type,
            _orientation: gtk::Orientation,
            _for_size: i32,
        ) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        fn snapshot(&self, widget: &Self::Type, snapshot: &gtk::Snapshot) {
            let widget_width = widget.width() as f64;
            let widget_height = widget.height() as f64;

            if widget_width == 0. || widget_height == 0. {
                return;
            }

            let paintable = self.paintable.borrow();

            let ratio = paintable.intrinsic_aspect_ratio();
            let (width, height) = if ratio == 0. {
                (widget_width, widget_height)
            } else {
                let widget_ratio = widget_width / widget_height;

                if widget_ratio > ratio {
                    (widget_width, widget_width / ratio)
                } else {
                    (widget_height * ratio, widget_height)
                }
            };

            let x = ((widget_width - width) / 2.).round();
            let y = ((widget_height - height) / 2.).round();

            snapshot.save();
            snapshot.translate(&graphene::Point::new(x as f32, y as f32));
            paintable.snapshot(snapshot.upcast_ref(), width, height);
            snapshot.append_color(
                &gdk::RGBA::new(0., 0., 0., self.dim.get()),
                &graphene::Rect::new(0., 0., width as f32, height as f32),
            );
            snapshot.restore();
        }
    }

    impl Background {
        pub fn set_paintable(&self, value: Option<gdk::Paintable>) {
            let value = value.unwrap_or_else(|| gdk::Paintable::new_empty(0, 0));

            let mut paintable = self.paintable.borrow_mut();
            if *paintable != value {
                *paintable = value;

                let obj = self.instance();
                obj.notify("paintable");
                obj.queue_draw();
            }
        }

        pub fn set_dim(&self, value: f32) {
            if self.dim.get() != value {
                self.dim.set(value);

                let obj = self.instance();
                obj.notify("dim");
                obj.queue_draw();
            }
        }
    }
}

glib::wrapper! {
    pub struct Background(ObjectSubclass<imp::Background>)
        @extends gtk::Widget;
}

impl Background {
    pub fn new() -> Self {
        glib::Object::new(&[]).unwrap()
    }

    pub fn set_paintable(&self, value: Option<impl IsA<gdk::Paintable>>) {
        self.imp().set_paintable(value.map(|x| x.upcast()))
    }
}

impl Default for Background {
    fn default() -> Self {
        Self::new()
    }
}
