mod app;
mod canvas;
mod export;
mod history;
mod interaction;
mod io;
mod layout;
mod model;
mod style;

use eframe::egui;

fn main() -> eframe::Result<()> {
    env_logger::init();

    // Check for command-line file argument
    let file_arg = std::env::args().nth(1).map(std::path::PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Mindmap"),
        ..Default::default()
    };

    eframe::run_native(
        "Mindmap",
        options,
        Box::new(move |cc| Ok(Box::new(app::MindmapApp::new(cc, file_arg)))),
    )
}
