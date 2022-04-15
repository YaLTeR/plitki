use gtk::glib;
use gtk::subclass::prelude::*;

mod imp {
    use std::cell::Cell;

    use adw::subclass::prelude::*;
    use glib::closure;
    use gtk::prelude::*;
    use gtk::CompositeTemplate;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/plitki-gnome/accuracy.ui")]
    pub struct Accuracy {
        #[template_child]
        label: TemplateChild<gtk::Label>,

        accuracy: Cell<f32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Accuracy {
        const NAME: &'static str = "PlitkiAccuracy";
        type Type = super::Accuracy;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.set_css_name("plitki-accuracy");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Accuracy {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.property_expression("accuracy")
                .chain_closure::<String>(closure!(|_: Option<glib::Object>, accuracy: f32| {
                    format!("{accuracy:.02}%")
                }))
                .bind(&*self.label, "label", None::<&Self::Type>);
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecFloat::new(
                    "accuracy",
                    "",
                    "",
                    0.,
                    100.,
                    0.,
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
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
                "accuracy" => self.set_accuracy(value.get().unwrap()),
                _ => unreachable!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "accuracy" => self.accuracy().to_value(),
                _ => unreachable!(),
            }
        }
    }

    impl WidgetImpl for Accuracy {}
    impl BinImpl for Accuracy {}

    impl Accuracy {
        pub fn set_accuracy(&self, value: f32) {
            if self.accuracy.get() != value {
                self.accuracy.set(value);
                self.instance().notify("accuracy");
            }
        }

        pub fn accuracy(&self) -> f32 {
            self.accuracy.get()
        }
    }
}

glib::wrapper! {
    pub struct Accuracy(ObjectSubclass<imp::Accuracy>)
        @extends adw::Bin, gtk::Widget;
}

impl Accuracy {
    pub fn new() -> Self {
        glib::Object::new(&[]).unwrap()
    }

    pub fn set_accuracy(&self, value: f32) {
        self.imp().set_accuracy(value);
    }

    pub fn accuracy(&self) -> f32 {
        self.imp().accuracy()
    }
}

impl Default for Accuracy {
    fn default() -> Self {
        Self::new()
    }
}
