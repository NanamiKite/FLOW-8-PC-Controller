#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod fonts;
mod i18n;
mod logging;
mod metrics;
mod theme;
mod widgets;

use app::Flow8App;
use eframe::egui;
use logging::init_tracing;

fn main() -> eframe::Result {
    let (logging_error, _log_maintenance) = init_tracing();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FLOW 8 PC Controller")
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/flow8.png"))
                    .expect("bundled FLOW 8 window icon must be a valid PNG"),
            )
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([1040.0, 700.0]),
        renderer: eframe::Renderer::Glow,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "FLOW 8 PC Controller",
        options,
        Box::new(move |context| Ok(Box::new(Flow8App::new(context, logging_error.as_deref())))),
    )
}
