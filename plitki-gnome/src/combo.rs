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
    #[template(resource = "/plitki-gnome/combo.ui")]
    pub struct Combo {
        #[template_child]
        label: TemplateChild<gtk::Label>,

        combo: Cell<u32>,
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
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            obj.property_expression("combo")
                .chain_closure::<String>(closure!(|_: Option<glib::Object>, combo: u32| {
                    format!("{combo}×")
                }))
                .bind(&*self.label, "label", None::<&Self::Type>);
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecUInt::new(
                    "combo",
                    "",
                    "",
                    0,
                    u32::MAX,
                    0,
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
                "combo" => self.set_combo(value.get().unwrap()),
                _ => unreachable!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "combo" => self.combo().to_value(),
                _ => unreachable!(),
            }
        }
    }

    impl WidgetImpl for Combo {}
    impl BinImpl for Combo {}

    impl Combo {
        pub fn set_combo(&self, value: u32) {
            if self.combo.get() != value {
                self.combo.set(value);
                self.instance().notify("combo");
            }
        }

        pub fn combo(&self) -> u32 {
            self.combo.get()
        }
    }
}

glib::wrapper! {
    pub struct Combo(ObjectSubclass<imp::Combo>)
        @extends adw::Bin, gtk::Widget;
}

impl Combo {
    pub fn new() -> Self {
        glib::Object::new(&[]).unwrap()
    }

    pub fn set_combo(&self, value: u32) {
        self.imp().set_combo(value);
    }

    pub fn combo(&self) -> u32 {
        self.imp().combo()
    }
}

impl Default for Combo {
    fn default() -> Self {
        Self::new()
    }
}
