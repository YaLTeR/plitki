use std::sync::RwLock;

use gtk::{gdk, gio};
use once_cell::sync::Lazy;

pub(crate) static SKIN: Lazy<RwLock<Skin>> = Lazy::new(|| RwLock::new(Skin::Arrows));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skin {
    Arrows,
    Bars,
    Circles,
}

impl Skin {
    pub(crate) fn load_texture(self, filename: &str) -> gdk::Texture {
        let folder = match self {
            Skin::Arrows => "arrows",
            Skin::Bars => "bars",
            Skin::Circles => "circles",
        };

        gdk::Texture::from_file(&gio::File::for_path(format!(
            "skin/{}/{}",
            folder, filename
        )))
        .unwrap()
    }
}

pub(crate) fn load_texture(filename: &str) -> gdk::Texture {
    SKIN.read().unwrap().load_texture(filename)
}
