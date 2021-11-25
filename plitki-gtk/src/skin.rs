use std::collections::HashMap;
use std::rc::Rc;

use gtk::gdk;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, glib::GSharedBoxed)]
#[gshared_boxed(type_name = "PlitkiSkin")]
pub struct Skin(Rc<Store>);

impl Skin {
    pub fn new(store: Store) -> Self {
        Self(Rc::new(store))
    }

    pub fn store(&self) -> &Store {
        &self.0
    }
}
