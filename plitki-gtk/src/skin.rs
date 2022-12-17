use std::cell::{Ref, RefMut};
use std::collections::HashMap;

use glib::subclass::prelude::*;
use gtk::gdk;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneSkin {
    pub object: gdk::Texture,
    pub ln_head: gdk::Texture,
    pub ln_body: gdk::Texture,
    pub ln_tail: gdk::Texture,
}

#[derive(Debug, Clone)]
pub struct Store {
    elements: HashMap<usize, Vec<LaneSkin>>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
        }
    }

    pub fn insert(&mut self, lane_count: usize, element: Vec<LaneSkin>) {
        assert!(lane_count > 0);
        assert!(element.len() == lane_count);

        self.elements.insert(lane_count, element);
    }

    pub fn get(&self, lane_count: usize, lane: usize) -> &LaneSkin {
        assert!(lane_count > 0);
        assert!(lane < lane_count);

        if let Some(element) = self.elements.get(&lane_count) {
            return &element[lane];
        }

        todo!()
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

mod imp {
    use std::cell::RefCell;

    use glib::prelude::*;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug, Default)]
    pub struct Skin {
        store: RefCell<Store>,
        name: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Skin {
        const NAME: &'static str = "PlitkiSkin";
        type Type = super::Skin;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Skin {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecString::builder("name")
                    .explicit_notify()
                    .build()]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "name" => self.name().to_value(),
                _ => unreachable!(),
            }
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "name" => {
                    let value: Option<String> = value.get().unwrap();
                    self.set_name(value);
                }
                _ => unreachable!(),
            }
        }
    }

    impl Skin {
        pub fn store(&self) -> Ref<'_, Store> {
            self.store.borrow()
        }

        pub fn store_mut(&self) -> RefMut<'_, Store> {
            self.store.borrow_mut()
        }

        pub fn name(&self) -> Ref<'_, Option<String>> {
            self.name.borrow()
        }

        pub fn set_name(&self, value: Option<String>) {
            if *self.name.borrow() == value {
                return;
            }

            self.name.replace(value);
            self.obj().notify("name");
        }
    }
}

glib::wrapper! {
    pub struct Skin(ObjectSubclass<imp::Skin>);
}

impl Skin {
    pub fn new(name: Option<String>) -> Self {
        glib::Object::builder().property("name", name).build()
    }

    pub fn store(&self) -> Ref<'_, Store> {
        self.imp().store()
    }

    pub fn store_mut(&self) -> RefMut<'_, Store> {
        self.imp().store_mut()
    }

    pub fn name(&self) -> Ref<'_, Option<String>> {
        self.imp().name()
    }

    pub fn set_name(&self, value: Option<String>) {
        self.imp().set_name(value)
    }
}
