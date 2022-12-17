use adw::prelude::*;
use glib::{GlibLogger, GlibLoggerDomain, GlibLoggerFormat};
use gtk::{gdk, gio};
use log::info;

mod lane_conveyor;
mod long_note;
mod playfield;
mod skin;
mod state;
mod utils;
mod window;

fn main() {
    static GLIB_LOGGER: GlibLogger =
        GlibLogger::new(GlibLoggerFormat::LineAndFile, GlibLoggerDomain::CrateTarget);
    let _ = log::set_logger(&GLIB_LOGGER);
    log::set_max_level(log::LevelFilter::Debug);

    info!(
        "{} version {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
    );

    gio::resources_register_include!("compiled.gresource").unwrap();

    let app = adw::Application::builder().build();
    app.connect_startup(on_startup);
    app.connect_activate(on_activate);
    app.run();
}

fn on_startup(app: &adw::Application) {
    // Load our CSS.
    let provider = gtk::CssProvider::new();
    provider.load_from_resource("/plitki-gtk/style.css");
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
    let window = window::ApplicationWindow::new(app);
    window.present();
}
