use gtk::glib;
use gtk::subclass::prelude::*;

mod imp {
    use std::cell::Cell;

    use adw::prelude::*;
    use once_cell::sync::Lazy;
    use once_cell::unsync::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct HitLight {
        opacity_animation: OnceCell<adw::Animation>,
        downscroll: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HitLight {
        const NAME: &'static str = "PlitkiHitLight";
        type Type = super::HitLight;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("plitki-hit-light");
        }
    }

    impl ObjectImpl for HitLight {
        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();

            obj.set_opacity(0.);

            let opacity_animation = adw::TimedAnimation::new(
                &*obj,
                1.,
                0.,
                500,
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
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecBoolean::builder("downscroll")
                    .explicit_notify()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "downscroll" => self.downscroll().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "downscroll" => self.set_downscroll(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for HitLight {}

    impl HitLight {
        pub fn downscroll(&self) -> bool {
            self.downscroll.get()
        }

        pub fn set_downscroll(&self, value: bool) {
            if self.downscroll.get() == value {
                return;
            }

            self.downscroll.set(value);

            if value {
                self.obj().add_css_class("upside-down");
            } else {
                self.obj().remove_css_class("upside-down");
            }

            self.obj().notify("downscroll");
        }

        pub fn fire(&self) {
            self.opacity_animation.get().unwrap().play();
        }
    }
}

glib::wrapper! {
    pub struct HitLight(ObjectSubclass<imp::HitLight>)
        @extends gtk::Widget;
}

impl HitLight {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn fire(&self) {
        self.imp().fire()
    }
}

impl Default for HitLight {
    fn default() -> Self {
        Self::new()
    }
}
