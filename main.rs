mod app;
mod canvas;
mod history;
mod interaction;
mod io;
mod layout;
mod model;
mod style;

use eframe::egui;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Mindmap"),
        ..Default::default()
    };

    eframe::run_native(
        "Mindmap",
        options,
        Box::new(|cc| Ok(Box::new(app::MindmapApp::new(cc)))),
    )
}
