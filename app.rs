use crate::canvas::renderer::{self, NodeRects};
use crate::canvas::viewport::Viewport;
use crate::history::History;
use crate::interaction::editing::EditingState;
use crate::interaction::input;
use crate::layout::reingold_tilford;
use crate::model::{MindmapTree, Selection};
use crate::style::colors;
use eframe::egui;
use std::path::PathBuf;

pub struct MindmapApp {
    tree: Option<MindmapTree>,
    viewport: Viewport,
    selection: Selection,
    history: History,
    editing: EditingState,
    node_rects: NodeRects,
    file_path: Option<PathBuf>,
    needs_initial_fit: bool,
    show_file_dialog: bool,
}

impl MindmapApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            tree: None,
            viewport: Viewport::default(),
            selection: Selection::default(),
            history: History::default(),
            editing: EditingState::default(),
            node_rects: NodeRects::default(),
            file_path: None,
            needs_initial_fit: false,
            show_file_dialog: true,
        }
    }

    fn load_file(&mut self, path: PathBuf) {
        match crate::io::freemind_read::load_mm_file(&path) {
            Ok(mut tree) => {
                reingold_tilford::layout(&mut tree);
                self.tree = Some(tree);
                self.file_path = Some(path);
                self.selection = Selection::default();
                self.history = History::default();
                self.editing = EditingState::default();
                self.needs_initial_fit = true;
                log::info!("File loaded successfully");
            }
            Err(e) => {
                log::error!("Failed to load file: {}", e);
                eprintln!("Failed to load file: {}", e);
            }
        }
    }
}

impl eframe::App for MindmapApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // File dialog on startup
        if self.show_file_dialog {
            self.show_file_dialog = false;
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("FreeMind", &["mm"])
                .pick_file()
            {
                self.load_file(path);
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(colors::CANVAS_BG))
            .show(ctx, |ui| {
                let screen_rect = ui.max_rect();

                // Fit to bounds on first frame after loading
                if self.needs_initial_fit {
                    if let Some(ref tree) = self.tree {
                        let bounds = compute_tree_bounds(tree);
                        self.viewport.fit_to_bounds(bounds, screen_rect, 80.0);
                    }
                    self.needs_initial_fit = false;
                }

                // Allocate the full panel as an interactive area
                let response = ui.allocate_rect(screen_rect, egui::Sense::click_and_drag());

                if let Some(ref mut tree) = &mut self.tree {
                    let painter = ui.painter();

                    // Render canvas
                    self.node_rects = renderer::draw_canvas(
                        painter,
                        tree,
                        &self.viewport,
                        screen_rect,
                        &self.selection,
                    );

                    // Handle input
                    let needs_relayout = input::handle_input(
                        ui,
                        &response,
                        &mut self.viewport,
                        tree,
                        &mut self.selection,
                        &self.node_rects,
                        screen_rect,
                        &mut self.history,
                        &mut self.editing,
                        &mut self.file_path,
                    );

                    // Draw text editor overlay
                    let edit_relayout = self.editing.draw(
                        ui,
                        tree,
                        &self.viewport,
                        screen_rect,
                        &mut self.history,
                    );

                    // Re-run layout if needed
                    if needs_relayout || edit_relayout {
                        reingold_tilford::layout(tree);
                    }

                    // Draw status bar
                    draw_status_bar(ui, screen_rect, tree, &self.file_path, &self.viewport);
                } else {
                    // No file loaded - show welcome message
                    ui.centered_and_justified(|ui| {
                        if ui
                            .button("Open .mm file (or drag and drop)")
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("FreeMind", &["mm"])
                                .pick_file()
                            {
                                self.show_file_dialog = false;
                                // Can't call load_file from here due to borrow,
                                // so we'll set a flag
                            }
                        }
                    });
                }
            });

        // Handle file drop
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped_files.first() {
            if let Some(ref path) = file.path {
                let p = path.clone();
                self.load_file(p);
            }
        }
    }
}

fn compute_tree_bounds(tree: &MindmapTree) -> egui::Rect {
    let visible = tree.visible_nodes();
    if visible.is_empty() {
        return egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO);
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for &id in &visible {
        let pos = tree.nodes[id].layout_pos;
        min_x = min_x.min(pos.x - 150.0);
        max_x = max_x.max(pos.x + 150.0);
        min_y = min_y.min(pos.y - 20.0);
        max_y = max_y.max(pos.y + 20.0);
    }
    egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
}

fn draw_status_bar(
    ui: &egui::Ui,
    screen_rect: egui::Rect,
    tree: &MindmapTree,
    file_path: &Option<PathBuf>,
    viewport: &Viewport,
) {
    let painter = ui.painter();
    let bar_height = 24.0;
    let bar_rect = egui::Rect::from_min_size(
        egui::pos2(screen_rect.min.x, screen_rect.max.y - bar_height),
        egui::vec2(screen_rect.width(), bar_height),
    );

    painter.rect_filled(
        bar_rect,
        0.0,
        egui::Color32::from_rgba_premultiplied(255, 255, 255, 220),
    );

    let file_name = file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|f| f.to_str())
        .unwrap_or("No file");

    let node_count = tree.visible_nodes().len();
    let total_count = tree.nodes.iter().filter(|n| !n.text.is_empty()).count();
    let zoom_pct = (viewport.zoom * 100.0) as i32;

    let status = format!(
        "  {}  |  {} / {} nodes  |  {}%  |  Tab: add child  Enter: sibling  F2: edit  Del: delete  Ctrl+S: save  Ctrl+.: fold",
        file_name, node_count, total_count, zoom_pct
    );

    painter.text(
        egui::pos2(bar_rect.min.x + 8.0, bar_rect.center().y),
        egui::Align2::LEFT_CENTER,
        status,
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(100, 100, 100),
    );
}
