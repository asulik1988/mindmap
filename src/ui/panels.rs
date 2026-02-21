use crate::canvas::viewport::Viewport;
use crate::model::{MindmapTree, NodeId, Selection};
use crate::style::colors::{self, DepthColorConfig};
use crate::style::wobble::{self, RoughOptions};
use crate::ui::{MENU_PAD_Y, SWATCH_COLS, SWATCH_GAP, SWATCH_ROWS, SWATCH_SIZE};
use eframe::egui;
use egui::epaint::{PathShape, RectShape, StrokeKind};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Style panel
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
pub(crate) enum StyleAction {
    None,
    SelectDepth(usize),
    SetColor(usize, usize),
    ResetAll,
}

const STYLE_PANEL_WIDTH: f32 = 280.0;
const DEPTH_ROW_HEIGHT: f32 = 32.0;
const STYLE_TITLE_HEIGHT: f32 = 36.0;

const DEPTH_LABELS: [&str; 8] = [
    "Root", "Level 1", "Level 2", "Level 3", "Level 4", "Level 5", "Level 6", "Level 7",
];

const STYLE_FOOTER_HEIGHT: f32 = 36.0;

fn style_panel_height(selected_depth: Option<usize>) -> f32 {
    let mut h = MENU_PAD_Y * 2.0 + STYLE_TITLE_HEIGHT;
    h += DEPTH_ROW_HEIGHT * 8.0;
    if selected_depth.is_some() {
        let grid_h = SWATCH_ROWS as f32 * (SWATCH_SIZE + SWATCH_GAP) + SWATCH_GAP + 8.0;
        h += grid_h;
    }
    h += STYLE_FOOTER_HEIGHT;
    h
}

pub(crate) fn style_panel_rect(pos: egui::Pos2, selected_depth: Option<usize>) -> egui::Rect {
    egui::Rect::from_min_size(
        pos,
        egui::vec2(STYLE_PANEL_WIDTH, style_panel_height(selected_depth)),
    )
}

pub(crate) fn draw_style_panel(
    ui: &egui::Ui,
    pos: egui::Pos2,
    selected_depth: Option<usize>,
    config: &DepthColorConfig,
    dark_mode: bool,
) -> StyleAction {
    let painter = ui.painter();
    let panel_rect = style_panel_rect(pos, selected_depth);

    let seed: u32 = 4567;

    // Shadow
    let shadow_rect = panel_rect.translate(egui::vec2(3.0, 3.0));
    painter.add(RectShape::new(
        shadow_rect,
        egui::CornerRadius::same(8),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 20),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Background
    painter.add(RectShape::new(
        panel_rect,
        egui::CornerRadius::same(8),
        colors::panel_bg(dark_mode),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(panel_rect, 8.0, seed, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let clicked = ui.input(|i| i.pointer.primary_clicked());
    let mut action = StyleAction::None;

    let label_color = colors::ui_text(dark_mode);
    let muted_color = colors::ui_text_muted(dark_mode);

    // Title row
    let mut y = panel_rect.min.y + MENU_PAD_Y;
    painter.text(
        egui::pos2(panel_rect.min.x + 14.0, y + STYLE_TITLE_HEIGHT / 2.0),
        egui::Align2::LEFT_CENTER,
        "Depth Colors",
        egui::FontId::proportional(15.0),
        label_color,
    );

    // Reset button (if there are overrides)
    if config.has_overrides() {
        let reset_rect = egui::Rect::from_min_size(
            egui::pos2(panel_rect.max.x - 40.0, y + 4.0),
            egui::vec2(28.0, 28.0),
        );
        let reset_hovered = pointer_pos.map_or(false, |p| reset_rect.contains(p));
        if reset_hovered {
            painter.add(RectShape::new(
                reset_rect,
                egui::CornerRadius::same(4),
                colors::hover_bg(dark_mode),
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        // Reset icon: circular arrow (↺) as text
        painter.text(
            reset_rect.center(),
            egui::Align2::CENTER_CENTER,
            "\u{21BA}",
            egui::FontId::proportional(16.0),
            if reset_hovered {
                label_color
            } else {
                muted_color
            },
        );
        if reset_hovered && clicked {
            action = StyleAction::ResetAll;
        }
    }

    y += STYLE_TITLE_HEIGHT;

    // Divider under title
    let div_paths = wobble::rough_line(
        egui::pos2(panel_rect.min.x + 12.0, y),
        egui::pos2(panel_rect.max.x - 12.0, y),
        seed.wrapping_add(100),
        &RoughOptions {
            roughness: 0.6,
            max_randomness_offset: 0.8,
            bowing: 0.3,
            disable_multi_stroke: true,
            ..Default::default()
        },
    );
    let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
    for path in div_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, div_stroke));
        }
    }

    y += 2.0;

    // Depth rows
    for depth in 0..8usize {
        let row_rect = egui::Rect::from_min_size(
            egui::pos2(panel_rect.min.x + 4.0, y),
            egui::vec2(STYLE_PANEL_WIDTH - 8.0, DEPTH_ROW_HEIGHT),
        );

        let row_hovered = pointer_pos.map_or(false, |p| row_rect.contains(p));
        let is_selected = selected_depth == Some(depth);

        // Hover/selection background
        if is_selected {
            painter.add(RectShape::new(
                row_rect,
                egui::CornerRadius::same(4),
                colors::selected_bg(dark_mode),
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
        } else if row_hovered {
            painter.add(RectShape::new(
                row_rect,
                egui::CornerRadius::same(4),
                colors::hover_bg(dark_mode),
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
        }

        if row_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Depth number
        let arrow = if is_selected { "\u{25BC} " } else { "" };
        painter.text(
            egui::pos2(row_rect.min.x + 12.0, row_rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{}{}", arrow, depth),
            egui::FontId::proportional(13.0),
            muted_color,
        );

        // Label
        painter.text(
            egui::pos2(row_rect.min.x + 48.0, row_rect.center().y),
            egui::Align2::LEFT_CENTER,
            DEPTH_LABELS[depth],
            egui::FontId::proportional(14.0),
            label_color,
        );

        // Color swatch (current color for this depth)
        let fill_idx = config.get_fill_index(depth);
        let fill_color = colors::depth_fill_color(fill_idx);
        let swatch_rect = egui::Rect::from_min_size(
            egui::pos2(row_rect.max.x - 32.0, row_rect.center().y - 8.0),
            egui::vec2(16.0, 16.0),
        );
        painter.rect_filled(swatch_rect, 3.0, fill_color);
        painter.rect_stroke(
            swatch_rect,
            3.0,
            egui::Stroke::new(1.0, colors::border_color(dark_mode)),
            StrokeKind::Outside,
        );

        // Click handler
        if row_hovered && clicked && action == StyleAction::None {
            action = StyleAction::SelectDepth(depth);
        }

        y += DEPTH_ROW_HEIGHT;

        // If this depth is selected, draw the swatch grid
        if is_selected {
            let grid_x = panel_rect.min.x + 16.0;
            let grid_y = y + 4.0;

            for row in 0..SWATCH_ROWS {
                for col in 0..SWATCH_COLS {
                    let idx = row * SWATCH_COLS + col;
                    if idx >= colors::DEPTH_FILL_COUNT {
                        break;
                    }
                    let sx = grid_x + col as f32 * (SWATCH_SIZE + SWATCH_GAP);
                    let sy = grid_y + row as f32 * (SWATCH_SIZE + SWATCH_GAP);
                    let swatch_r = egui::Rect::from_min_size(
                        egui::pos2(sx, sy),
                        egui::vec2(SWATCH_SIZE, SWATCH_SIZE),
                    );

                    let color = colors::depth_fill_color(idx);
                    painter.rect_filled(swatch_r, 4.0, color);

                    // Highlight current selection
                    let is_current = fill_idx == idx;
                    if is_current {
                        painter.rect_stroke(
                            swatch_r.expand(1.0),
                            4.0,
                            egui::Stroke::new(2.0, colors::border_color(dark_mode)),
                            StrokeKind::Outside,
                        );
                    }

                    let swatch_hovered = pointer_pos.map_or(false, |p| swatch_r.contains(p));
                    if swatch_hovered {
                        painter.rect_stroke(
                            swatch_r,
                            4.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 80, 80)),
                            StrokeKind::Outside,
                        );
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);

                        if clicked && action == StyleAction::None {
                            action = StyleAction::SetColor(depth, idx);
                        }
                    }
                }
            }

            let grid_h = SWATCH_ROWS as f32 * (SWATCH_SIZE + SWATCH_GAP) + SWATCH_GAP + 8.0;
            y += grid_h;
        }
    }

    // Footer: explain color cycling
    let footer_color = colors::ui_text_muted(dark_mode);
    // Wobbled divider above footer
    let div_paths = wobble::rough_line(
        egui::pos2(panel_rect.min.x + 12.0, y + 2.0),
        egui::pos2(panel_rect.max.x - 12.0, y + 2.0),
        seed.wrapping_add(200),
        &RoughOptions {
            roughness: 0.6,
            max_randomness_offset: 0.8,
            bowing: 0.3,
            disable_multi_stroke: true,
            ..Default::default()
        },
    );
    let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
    for path in div_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, div_stroke));
        }
    }
    painter.text(
        egui::pos2(panel_rect.min.x + 14.0, y + STYLE_FOOTER_HEIGHT / 2.0 + 2.0),
        egui::Align2::LEFT_CENTER,
        "Colors cycle every 8 levels (8=Root, 9=L1...)",
        egui::FontId::proportional(11.0),
        footer_color,
    );

    action
}

// ---------------------------------------------------------------------------
// Notes panel
// ---------------------------------------------------------------------------

pub(crate) const NOTES_PANEL_WIDTH: f32 = 300.0;
pub(crate) const NOTES_PANEL_MIN_HEIGHT: f32 = 300.0;
const NOTES_HEADER_H: f32 = 36.0;
const NOTES_PAD: f32 = 12.0;

pub(crate) struct NotesPanelResult {
    pub close: bool,
    pub text_focused: bool,
    pub navigate_to: Option<NodeId>,
    pub notes_changed: bool,
}

fn collect_nodes_with_notes(tree: &MindmapTree) -> Vec<NodeId> {
    let mut result = Vec::new();
    dfs_collect_notes(tree, tree.root, &mut result);
    result
}

fn dfs_collect_notes(tree: &MindmapTree, id: NodeId, out: &mut Vec<NodeId>) {
    if !tree.nodes[id].notes.is_empty() {
        out.push(id);
    }
    for &child in &tree.nodes[id].children {
        dfs_collect_notes(tree, child, out);
    }
}

pub(crate) fn draw_notes_panel(
    ui: &mut egui::Ui,
    panel_rect: egui::Rect,
    tree: &mut MindmapTree,
    edit_node: &mut Option<NodeId>,
    selection: &Selection,
    depth_color_config: &DepthColorConfig,
    saved_alpha: f32,
    dark_mode: bool,
) -> NotesPanelResult {
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let clicked = ui.input(|i| i.pointer.primary_clicked());
    let mut close_clicked = false;
    let mut text_focused = false;
    let mut navigate_to: Option<NodeId> = None;
    let mut notes_changed = false;

    // Collect notes-bearing nodes once (only needed in browser mode)
    let notes_ids: Vec<NodeId> = if edit_node.is_none() {
        collect_nodes_with_notes(tree)
    } else {
        Vec::new()
    };

    // --- Background + border ---
    {
        let mut shapes: Vec<egui::Shape> = Vec::new();
        shapes.push(
            RectShape::new(
                panel_rect.translate(egui::vec2(3.0, 3.0)),
                egui::CornerRadius::same(8),
                egui::Color32::from_rgba_premultiplied(0, 0, 0, 20),
                egui::Stroke::NONE,
                StrokeKind::Outside,
            )
            .into(),
        );
        shapes.push(
            RectShape::new(
                panel_rect,
                egui::CornerRadius::same(8),
                colors::panel_bg(dark_mode),
                egui::Stroke::NONE,
                StrokeKind::Outside,
            )
            .into(),
        );
        let rough_opts = RoughOptions {
            roughness: 0.5,
            max_randomness_offset: 1.0,
            bowing: 0.5,
            ..Default::default()
        };
        let border_paths = wobble::rough_rounded_rect(panel_rect, 8.0, 9123, &rough_opts);
        let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
        for path in border_paths {
            if path.len() >= 2 {
                shapes.push(PathShape::line(path, border_stroke).into());
            }
        }
        ui.painter().extend(shapes);
    }

    // --- Close button ---
    let close_center = egui::pos2(
        panel_rect.max.x - NOTES_PAD,
        panel_rect.min.y + NOTES_HEADER_H / 2.0,
    );
    let close_btn_rect = egui::Rect::from_center_size(close_center, egui::vec2(20.0, 20.0));
    let close_hovered = pointer_pos.map_or(false, |p| close_btn_rect.contains(p));
    if close_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter()
            .circle_filled(close_btn_rect.center(), 10.0, colors::hover_bg(dark_mode));
    }
    let close_color = if close_hovered {
        colors::ui_text(dark_mode)
    } else {
        colors::ui_text_muted(dark_mode)
    };
    ui.painter().text(
        close_btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "×",
        egui::FontId::proportional(16.0),
        close_color,
    );
    if close_hovered && clicked {
        close_clicked = true;
    }

    // --- Header ---
    let header_center_y = panel_rect.min.y + NOTES_HEADER_H / 2.0;
    let header_x = panel_rect.min.x + NOTES_PAD;

    if edit_node.is_some() {
        // Edit mode: "← All Notes" link
        let back_color = egui::Color32::from_rgb(30, 136, 229);
        let back_hovered = pointer_pos.map_or(false, |p| {
            p.y >= panel_rect.min.y
                && p.y <= panel_rect.min.y + NOTES_HEADER_H
                && p.x >= header_x
                && p.x < close_btn_rect.min.x
        });
        if back_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let back_galley = ui.painter().layout_no_wrap(
            "← All Notes".to_string(),
            egui::FontId::proportional(13.0),
            back_color,
        );
        let back_h = back_galley.size().y;
        ui.painter().galley(
            egui::pos2(header_x, header_center_y - back_h / 2.0),
            back_galley,
            back_color,
        );
        if back_hovered && clicked {
            *edit_node = None;
        }

        // "Saved" indicator — centered in header, fades in/out via saved_alpha
        if saved_alpha > 0.0 {
            let alpha = (saved_alpha * 255.0) as u8;
            let saved_color = egui::Color32::from_rgba_unmultiplied(0x9E, 0x9E, 0x96, alpha);
            let saved_galley = ui.painter().layout_no_wrap(
                "Saved".to_string(),
                egui::FontId::proportional(13.0),
                saved_color,
            );
            let saved_w = saved_galley.size().x;
            let saved_h = saved_galley.size().y;
            let center_x = panel_rect.center().x;
            ui.painter().galley(
                egui::pos2(center_x - saved_w / 2.0, header_center_y - saved_h / 2.0),
                saved_galley,
                saved_color,
            );
        }
    } else {
        // Browser mode: "Notes" bold + " (N)" count muted
        let count = notes_ids.len();
        let notes_galley = ui.painter().layout_no_wrap(
            "Notes".to_string(),
            egui::FontId::proportional(14.0),
            colors::ui_text(dark_mode),
        );
        let notes_w = notes_galley.size().x;
        let notes_h = notes_galley.size().y;
        ui.painter().galley(
            egui::pos2(header_x, header_center_y - notes_h / 2.0),
            notes_galley,
            colors::ui_text(dark_mode),
        );
        let count_text = format!(" ({})", count);
        let count_galley = ui.painter().layout_no_wrap(
            count_text,
            egui::FontId::proportional(12.0),
            colors::ui_text_muted(dark_mode),
        );
        let count_h = count_galley.size().y;
        ui.painter().galley(
            egui::pos2(header_x + notes_w, header_center_y - count_h / 2.0),
            count_galley,
            colors::ui_text_muted(dark_mode),
        );
    }

    // --- Wobbly divider below header ---
    let divider_y = panel_rect.min.y + NOTES_HEADER_H;
    {
        let div_paths = wobble::rough_line(
            egui::pos2(panel_rect.min.x + NOTES_PAD, divider_y),
            egui::pos2(panel_rect.max.x - NOTES_PAD, divider_y),
            9999,
            &RoughOptions {
                roughness: 0.4,
                max_randomness_offset: 0.7,
                bowing: 0.3,
                disable_multi_stroke: true,
                ..Default::default()
            },
        );
        let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
        for path in div_paths {
            if path.len() >= 2 {
                ui.painter().add(PathShape::line(path, div_stroke));
            }
        }
    }

    // --- Content area ---
    let content_rect = egui::Rect::from_min_max(
        egui::pos2(panel_rect.min.x, divider_y + 8.0),
        egui::pos2(panel_rect.max.x, panel_rect.max.y - NOTES_PAD),
    );

    if let Some(id) = *edit_node {
        // Edit mode: label + TextEdit
        if id < tree.nodes.len() {
            let node_label = tree.nodes[id].text.clone();
            let label_max_w = content_rect.width() - NOTES_PAD * 2.0;
            let label_galley = ui.painter().layout(
                node_label,
                egui::FontId::proportional(13.0),
                colors::ui_text_muted(dark_mode),
                label_max_w,
            );
            let label_h = label_galley.size().y;
            ui.painter().galley(
                egui::pos2(content_rect.min.x + NOTES_PAD, content_rect.min.y),
                label_galley,
                colors::ui_text_muted(dark_mode),
            );

            let te_rect = egui::Rect::from_min_max(
                egui::pos2(
                    content_rect.min.x + NOTES_PAD,
                    content_rect.min.y + label_h + 8.0,
                ),
                egui::pos2(content_rect.max.x - NOTES_PAD, content_rect.max.y),
            );
            let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(te_rect));
            let te_response = child_ui.add(
                egui::TextEdit::multiline(&mut tree.nodes[id].notes)
                    .font(egui::FontId::proportional(14.0))
                    .frame(false)
                    .desired_width(f32::INFINITY)
                    .hint_text("Add notes…"),
            );
            text_focused = te_response.has_focus();
            if te_response.changed() {
                notes_changed = true;
            }
        }
    } else if notes_ids.is_empty() {
        // Empty state
        let cx = content_rect.center().x;
        let mut y = content_rect.min.y + 24.0;

        // Document icon (2.5× scaled)
        let scale = 2.5_f32;
        let doc_w = 10.0 * scale;
        let doc_h = 12.0 * scale;
        let doc_x = cx - doc_w / 2.0;
        let doc_y = y;
        let ic = colors::ui_text(dark_mode);
        let icon_color = egui::Color32::from_rgba_unmultiplied(ic.r(), ic.g(), ic.b(), 76);
        ui.painter().rect_stroke(
            egui::Rect::from_min_size(egui::pos2(doc_x, doc_y), egui::vec2(doc_w, doc_h)),
            2.0,
            egui::Stroke::new(1.5, icon_color),
            StrokeKind::Outside,
        );
        for y_frac in [2.5_f32 / 12.0, 5.0 / 12.0, 7.5 / 12.0] {
            let ly = doc_y + y_frac * doc_h;
            ui.painter().line_segment(
                [
                    egui::pos2(doc_x + 1.5 * scale, ly),
                    egui::pos2(doc_x + (10.0 - 1.5) * scale, ly),
                ],
                egui::Stroke::new(1.0, icon_color),
            );
        }
        y += doc_h + 16.0;

        let title_galley = ui.painter().layout_no_wrap(
            "No notes yet".to_string(),
            egui::FontId::proportional(15.0),
            colors::ui_text(dark_mode),
        );
        ui.painter().galley(
            egui::pos2(cx - title_galley.size().x / 2.0, y),
            title_galley.clone(),
            colors::ui_text(dark_mode),
        );
        y += title_galley.size().y + 8.0;

        let hint_text = "Right-click any node and\nchoose \"Notes\" to add one.".to_string();
        let hint_galley = ui.painter().layout(
            hint_text,
            egui::FontId::proportional(13.0),
            colors::ui_text_muted(dark_mode),
            content_rect.width() - NOTES_PAD * 2.0,
        );
        ui.painter().galley(
            egui::pos2(cx - hint_galley.size().x / 2.0, y),
            hint_galley,
            colors::ui_text_muted(dark_mode),
        );
    } else {
        // Browser list
        let dot_area = 14.0_f32;
        let pencil_area = 24.0_f32;
        let label_w = content_rect.width() - dot_area - pencil_area - NOTES_PAD * 2.0;
        let selected_id = selection.primary();
        let scroll_width = content_rect.width();

        let mut scroll_ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));
        egui::ScrollArea::vertical().show(&mut scroll_ui, |ui| {
            for (entry_idx, &node_id) in notes_ids.iter().enumerate() {
                let depth = tree.depth(node_id);
                let dot_color = tree.nodes[node_id]
                    .background_color
                    .unwrap_or_else(|| colors::node_palette(depth, depth_color_config).fill);

                let node_label = tree.nodes[node_id].text.clone();
                let notes_preview = tree.nodes[node_id].notes.clone();

                // Layout label (single line, no wrap)
                let label_galley = ui.painter().layout_no_wrap(
                    node_label,
                    egui::FontId::proportional(14.0),
                    colors::ui_text(dark_mode),
                );

                // Layout preview (wraps at label_w)
                let preview_galley = ui.painter().layout(
                    notes_preview,
                    egui::FontId::proportional(13.0),
                    colors::ui_text_muted(dark_mode),
                    label_w,
                );

                // Cap preview height at 3 rows
                let max_preview_rows = 3usize;
                let row_h = preview_galley
                    .rows
                    .first()
                    .map(|r| r.rect.height())
                    .unwrap_or(16.0);
                let preview_h = if preview_galley.rows.len() > max_preview_rows {
                    row_h * max_preview_rows as f32
                } else {
                    preview_galley.size().y
                };

                let label_h = label_galley.size().y;
                let entry_h = label_h + preview_h + 16.0;

                // Allocate entry space (returns screen-space rect)
                let (entry_rect, entry_response) =
                    ui.allocate_exact_size(egui::vec2(scroll_width, entry_h), egui::Sense::click());

                let ptr = ui.input(|i| i.pointer.hover_pos());
                let hovered = ptr.map_or(false, |p| entry_rect.contains(p));

                // Pencil hit rect (top-right of entry)
                let pencil_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        entry_rect.max.x - pencil_area,
                        entry_rect.min.y + (entry_h - 20.0) / 2.0,
                    ),
                    egui::vec2(20.0, 20.0),
                );
                let pencil_hovered = ptr.map_or(false, |p| pencil_rect.contains(p));

                // Hover background
                if hovered {
                    ui.painter()
                        .rect_filled(entry_rect, 0.0, colors::hover_bg(dark_mode));
                    if !pencil_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }

                // Left accent bar for currently-selected node
                if selected_id == Some(node_id) {
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(entry_rect.min, egui::vec2(3.0, entry_h)),
                        0.0,
                        egui::Color32::from_rgb(30, 136, 229),
                    );
                }

                // Colored dot
                let dot_x = entry_rect.min.x + NOTES_PAD + 3.0;
                let dot_y = entry_rect.min.y + 8.0 + label_h / 2.0;
                ui.painter()
                    .circle_filled(egui::pos2(dot_x, dot_y), 3.0, dot_color);

                // Label (single line)
                let label_x = entry_rect.min.x + NOTES_PAD + dot_area;
                let label_y = entry_rect.min.y + 8.0;
                ui.painter().galley(
                    egui::pos2(label_x, label_y),
                    label_galley,
                    colors::ui_text(dark_mode),
                );

                // Preview (clipped to 3 lines)
                let preview_y = label_y + label_h + 2.0;
                let clip_rect = egui::Rect::from_min_size(
                    egui::pos2(label_x, preview_y),
                    egui::vec2(label_w, preview_h),
                );
                ui.painter().with_clip_rect(clip_rect).galley(
                    egui::pos2(label_x, preview_y),
                    preview_galley,
                    colors::ui_text_muted(dark_mode),
                );

                // Pencil icon
                let pencil_alpha: u8 = if pencil_hovered { 204 } else { 102 };
                let pc = colors::ui_text(dark_mode);
                ui.painter().text(
                    pencil_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "✏",
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_rgba_unmultiplied(pc.r(), pc.g(), pc.b(), pencil_alpha),
                );
                if pencil_hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Click handling
                if entry_response.clicked() {
                    if pencil_hovered {
                        *edit_node = Some(node_id);
                    } else {
                        navigate_to = Some(node_id);
                    }
                }

                // Wobbly divider between entries
                if entry_idx + 1 < notes_ids.len() {
                    let div_y = entry_rect.max.y;
                    let div_paths = wobble::rough_line(
                        egui::pos2(entry_rect.min.x + NOTES_PAD, div_y),
                        egui::pos2(entry_rect.max.x - NOTES_PAD, div_y),
                        9999u32.wrapping_add(entry_idx as u32).wrapping_mul(1234567),
                        &RoughOptions {
                            roughness: 0.4,
                            max_randomness_offset: 0.7,
                            bowing: 0.3,
                            disable_multi_stroke: true,
                            ..Default::default()
                        },
                    );
                    let div_stroke = egui::Stroke::new(1.0, colors::divider_color(dark_mode));
                    for path in div_paths {
                        if path.len() >= 2 {
                            ui.painter().add(PathShape::line(path, div_stroke));
                        }
                    }
                }
            }
        });
    }

    NotesPanelResult {
        close: close_clicked,
        text_focused,
        navigate_to,
        notes_changed,
    }
}

// ---------------------------------------------------------------------------
// Link edit bar
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
pub(crate) enum LinkEditAction {
    None,
    Confirm,
    Cancel,
}

pub(crate) fn link_edit_bar_rect(screen_rect: egui::Rect) -> egui::Rect {
    let bar_w = 420.0;
    let bar_h = 36.0;
    let bar_x = screen_rect.center().x - bar_w / 2.0;
    let bar_y = screen_rect.max.y - 28.0 - 8.0 - bar_h;
    egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w, bar_h))
}

pub(crate) fn draw_link_edit_bar(
    ui: &mut egui::Ui,
    link_edit: &mut Option<(NodeId, String)>,
    screen_rect: egui::Rect,
    dark_mode: bool,
) -> LinkEditAction {
    let Some((_, ref mut url_text)) = link_edit else {
        return LinkEditAction::None;
    };

    let bar_rect = link_edit_bar_rect(screen_rect);

    let mut shapes: Vec<egui::Shape> = Vec::new();

    // Shadow
    shapes.push(
        RectShape::new(
            bar_rect.translate(egui::vec2(3.0, 3.0)),
            egui::CornerRadius::same(8),
            egui::Color32::from_rgba_premultiplied(0, 0, 0, 20),
            egui::Stroke::NONE,
            StrokeKind::Outside,
        )
        .into(),
    );

    // Background
    shapes.push(
        RectShape::new(
            bar_rect,
            egui::CornerRadius::same(8),
            colors::panel_bg(dark_mode),
            egui::Stroke::NONE,
            StrokeKind::Outside,
        )
        .into(),
    );

    // Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(bar_rect, 8.0, 6543, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            shapes.push(PathShape::line(path, border_stroke).into());
        }
    }

    for shape in shapes {
        ui.painter().add(shape);
    }

    // Hint label on the left
    ui.painter().text(
        egui::pos2(bar_rect.min.x + 10.0, bar_rect.center().y),
        egui::Align2::LEFT_CENTER,
        "URL:",
        egui::FontId::proportional(13.0),
        colors::ui_text_muted(dark_mode),
    );

    // Text input
    let te_rect = egui::Rect::from_min_max(
        egui::pos2(bar_rect.min.x + 42.0, bar_rect.min.y + 4.0),
        egui::pos2(bar_rect.max.x - 8.0, bar_rect.max.y - 4.0),
    );
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(te_rect));
    let te_response = child_ui.add(
        egui::TextEdit::singleline(url_text)
            .font(egui::FontId::proportional(13.0))
            .frame(false)
            .desired_width(f32::INFINITY)
            .hint_text("https://..."),
    );

    // Auto-focus
    if !te_response.has_focus() {
        te_response.request_focus();
    }

    // Enter = confirm, Escape = cancel
    if te_response.lost_focus() {
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            return LinkEditAction::Confirm;
        }
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return LinkEditAction::Cancel;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        return LinkEditAction::Confirm;
    }

    LinkEditAction::None
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

pub(crate) fn draw_status_bar(
    ui: &egui::Ui,
    screen_rect: egui::Rect,
    tree: &MindmapTree,
    file_path: &Option<PathBuf>,
    viewport: &Viewport,
    dirty: bool,
) {
    let painter = ui.painter();
    let bar_height = 28.0;
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
        .unwrap_or("Untitled");

    let node_count = tree.visible_nodes().len();
    let zoom_pct = (viewport.zoom * 100.0) as i32;

    let dirty_marker = if dirty { "● " } else { "" };
    let status = format!(
        "  {}{}  |  {} nodes  |  {}%",
        dirty_marker, file_name, node_count, zoom_pct
    );

    painter.text(
        egui::pos2(bar_rect.min.x + 8.0, bar_rect.center().y),
        egui::Align2::LEFT_CENTER,
        status,
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(100, 100, 100),
    );
}
