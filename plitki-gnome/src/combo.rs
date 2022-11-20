use gtk::glib;
use gtk::subclass::prelude::*;

mod imp {
    use std::cell::Cell;

    use adw::subclass::prelude::*;
    use glib::closure;
    use gtk::prelude::*;
    use gtk::{graphene, CompositeTemplate};
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/plitki-gnome/combo.ui")]
    pub struct Combo {
        #[template_child]
        label: TemplateChild<gtk::Label>,

        combo: Cell<u32>,
        scale: Cell<f32>,
    }

    impl Default for Combo {
        fn default() -> Self {
            Self {
                label: Default::default(),
                combo: Default::default(),
                scale: Cell::new(1.),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Combo {
        const NAME: &'static str = "PlitkiCombo";
        type Type = super::Combo;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.set_css_name("plitki-combo");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Combo {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj()
                .property_expression("combo")
                .chain_closure::<String>(closure!(|_: Option<glib::Object>, combo: u32| {
                    format!("{combo}×")
                }))
                .bind(&*self.label, "label", None::<&Self::Type>);
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecUInt::builder("combo")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecFloat::builder("scale")
                        .minimum(0.)
                        .default_value(1.)
                        .explicit_notify()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "combo" => self.set_combo(value.get().unwrap()),
                "scale" => self.set_scale(value.get().unwrap()),
                _ => unreachable!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "combo" => self.combo().to_value(),
                "scale" => self.scale().to_value(),
                _ => unreachable!(),
            }
        }
    }

    impl WidgetImpl for Combo {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let scale = self.scale.get();
            if scale < 0.01 {
                return;
            }

            let widget = self.obj();
            let width = widget.width() as f32;
            let height = widget.height() as f32;
            let scaled_width = width * scale;
            let scaled_height = height * scale;

            snapshot.save();
            snapshot.translate(&graphene::Point::new(
                (width - scaled_width) / 2.,
                (height - scaled_height) / 2.,
            ));
            snapshot.scale(scale, scale);
            self.parent_snapshot(snapshot);
            snapshot.restore();
        }
    }

    impl BinImpl for Combo {}

    impl Combo {
        pub fn set_combo(&self, value: u32) {
            if self.combo.get() != value {
                self.combo.set(value);
                self.obj().notify("combo");
            }
        }

        pub fn combo(&self) -> u32 {
            self.combo.get()
        }

        pub fn set_scale(&self, value: f32) {
            if self.scale.get() != value {
                self.scale.set(value);
                self.obj().notify("scale");
                self.obj().queue_draw();
            }
        }

        pub fn scale(&self) -> f32 {
            self.scale.get()
        }
    }
}

glib::wrapper! {
    pub struct Combo(ObjectSubclass<imp::Combo>)
        @extends adw::Bin, gtk::Widget;
}

impl Combo {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn set_combo(&self, value: u32) {
        self.imp().set_combo(value);
    }

    pub fn combo(&self) -> u32 {
        self.imp().combo()
    }

    pub fn set_scale(&self, value: f32) {
        self.imp().set_scale(value);
    }

    pub fn scale(&self) -> f32 {
        self.imp().scale()
    }
}

impl Default for Combo {
    fn default() -> Self {
        Self::new()
    }
}
