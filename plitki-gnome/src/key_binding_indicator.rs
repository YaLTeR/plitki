use gtk::glib;
use gtk::subclass::prelude::*;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::prelude::*;
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct KeyBindingIndicator {
        indicator: gtk::ShortcutLabel,
        accelerator: RefCell<Option<String>>,
        opacity_animation: OnceCell<adw::Animation>,
        downscroll: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KeyBindingIndicator {
        const NAME: &'static str = "PlitkiKeyBindingIndicator";
        type Type = super::KeyBindingIndicator;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();

            klass.set_css_name("plitki-key-binding-indicator");
        }
    }

    impl ObjectImpl for KeyBindingIndicator {
        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();

            obj.set_opacity(0.);

            self.indicator.set_parent(&*obj);
            self.indicator.set_halign(gtk::Align::Center);
            obj.bind_property("accelerator", &self.indicator, "accelerator")
                .sync_create()
                .build();

            let opacity_animation = adw::TimedAnimation::new(
                &*obj,
                1.,
                0.,
                5000,
                &adw::PropertyAnimationTarget::new(&*obj, "opacity"),
            );
            self.opacity_animation
                .set(opacity_animation.upcast())
                .unwrap();
        }

        fn dispose(&self) {
            // PropertyAnimationTarget does not like it when the target object is finalized first,
            // so get rid of it.
            let opacity_animation = self.opacity_animation.get().unwrap();
            opacity_animation.set_target(&adw::CallbackAnimationTarget::new(|_| ()));

            self.indicator.unparent();
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoolean::builder("downscroll")
                        .explicit_notify()
                        .build(),
                    glib::ParamSpecString::builder("accelerator")
                        .explicit_notify()
                        .build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "downscroll" => self.downscroll().to_value(),
                "accelerator" => self.accelerator().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "downscroll" => self.set_downscroll(value.get().unwrap()),
                "accelerator" => self.set_accelerator(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for KeyBindingIndicator {}

    impl KeyBindingIndicator {
        pub fn downscroll(&self) -> bool {
            self.downscroll.get()
        }

        pub fn set_downscroll(&self, value: bool) {
            if self.downscroll.get() == value {
                return;
            }

            self.downscroll.set(value);
            self.obj().notify("downscroll");
        }

        pub fn accelerator(&self) -> Option<String> {
            self.accelerator.borrow().clone()
        }

        pub fn set_accelerator(&self, value: Option<String>) {
            if *self.accelerator.borrow() == value {
                return;
            }

            self.accelerator.replace(value);
            self.obj().notify("accelerator");
        }

        pub fn fire(&self) {
            self.opacity_animation.get().unwrap().play();
        }
    }
}

glib::wrapper! {
    pub struct KeyBindingIndicator(ObjectSubclass<imp::KeyBindingIndicator>)
        @extends gtk::Widget;
}

impl KeyBindingIndicator {
    pub fn new(accelerator: Option<String>) -> Self {
        glib::Object::builder()
            .property("accelerator", accelerator)
            .build()
    }

    pub fn fire(&self) {
        self.imp().fire()
    }
}

impl Default for KeyBindingIndicator {
    fn default() -> Self {
        Self::new(None)
    }
}
