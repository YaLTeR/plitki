#[macro_use]
extern crate tracing;

use std::env;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio};
use plitki_audio::AudioEngine;
use tracing_subscriber::prelude::*;
use window::Window;

mod accuracy;
mod background;
mod combo;
mod hit_error;
mod hit_light;
mod judgement;
mod key_binding_indicator;
mod statistics;
mod window;

fn main() {
    let (chrome_layer, _guard) = if env::var_os("PLITKI_PROFILE").is_some() {
        let (layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
            .file("trace.json")
            .include_args(true)
            .include_locations(false)
            .build();
        (Some(layer), Some(guard))
    } else {
        (None, None)
    };

    tracing_subscriber::registry()
        .with(chrome_layer)
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::filter::LevelFilter::DEBUG)
        .init();

    info!(
        "{} version {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    );

    gio::resources_register_include!("compiled.gresource").unwrap();

    let app = adw::Application::builder()
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_startup(on_startup);
    app.connect_activate(on_activate);
    app.connect_open(on_open);
    app.run();
}

fn on_open(app: &adw::Application, files: &[gio::File], _hint: &str) {
    let audio = Rc::new(AudioEngine::new());

    let window = Window::new(app, audio);

    if let Some(file) = files.get(0) {
        window.open_file(file.clone());
    }

    window.present();
}

fn on_startup(app: &adw::Application) {
    // Load our CSS.
    let provider = gtk::CssProvider::new();
    provider.load_from_resource("/plitki-gnome/style.css");
    if let Some(display) = gdk::Display::default() {
        gtk::StyleContext::add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // Set dark style as default since playfields are usually dark.
    app.style_manager()
        .set_color_scheme(adw::ColorScheme::PreferDark);
}

fn on_activate(app: &adw::Application) {
    let audio = Rc::new(AudioEngine::new());

    let window = Window::new(app, audio);

    window.present();
}
