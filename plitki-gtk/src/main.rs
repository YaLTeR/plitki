use glib::{GlibLogger, GlibLoggerDomain, GlibLoggerFormat};
use gtk::gdk;
use gtk::prelude::*;
use log::info;

mod long_note;

mod plitki_view;
use plitki_core::map::Map;
use plitki_view::PlitkiView;

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

    let app = gtk::Application::builder().build();
    app.connect_startup(on_startup);
    app.connect_activate(on_activate);
    app.run();
}

fn on_startup(_app: &gtk::Application) {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(include_bytes!("style.css"));
    if let Some(display) = gdk::Display::default() {
        gtk::StyleContext::add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn on_activate(app: &gtk::Application) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .default_width(800)
        .default_height(600)
        .build();

    let bytes = include_bytes!("../../plitki-map-qua/tests/data/actual_map.qua");
    let qua = plitki_map_qua::from_reader(&bytes[..]).unwrap();
    let map: Map = qua.into();
    let view = PlitkiView::new(map);

    let scrolled_window = gtk::ScrolledWindowBuilder::new()
        .child(&view)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0., 255., 1.);
    scale.set_draw_value(true);
    scale.set_hexpand(true);
    view.bind_property("scroll-speed", &scale.adjustment(), "value")
        .flags(glib::BindingFlags::BIDIRECTIONAL | glib::BindingFlags::SYNC_CREATE)
        .build()
        .unwrap();

    let scroll_speed_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    scroll_speed_box.append(&gtk::Label::new(Some("Scroll Speed")));
    scroll_speed_box.append(&scale);

    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 0);
    box_.append(&scrolled_window);
    box_.append(&scroll_speed_box);

    window.set_child(Some(&box_));

    window.present();
}
