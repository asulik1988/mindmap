use crate::canvas::viewport::Viewport;
use crate::interaction::search::SearchState;
use crate::model::{MindmapTree, NodeId};
use crate::style::colors::{self, DepthColorConfig};
use crate::style::wobble::{self, RoughOptions};
use eframe::egui;
use egui::epaint::{PathShape, RectShape, StrokeKind};

#[derive(PartialEq)]
pub(crate) enum SearchBarAction {
    None,
    Close,
    Next,
    Prev,
    ZoomTo,
    ReplaceOne,
    ReplaceAll,
}

pub(crate) fn draw_search_bar(
    ui: &mut egui::Ui,
    search: &mut SearchState,
    screen_rect: egui::Rect,
    dark_mode: bool,
) -> SearchBarAction {
    let mut action = SearchBarAction::None;

    // Bar dimensions
    let caret_w = 24.0;
    let caret_gap = 4.0;
    let bar_w = 350.0;
    let row_h = 36.0;
    let bar_h = if search.replace_active {
        row_h * 2.0 + 4.0
    } else {
        row_h
    };
    // Center the bar + caret together
    let total_w = caret_w + caret_gap + bar_w;
    let bar_x = screen_rect.center().x - total_w / 2.0 + caret_w + caret_gap;
    let bar_y = screen_rect.min.y + 12.0;
    let bar_rect = egui::Rect::from_min_size(egui::pos2(bar_x, bar_y), egui::vec2(bar_w, bar_h));

    // Caret button (sits to the left of bar, vertically centered on the first row)
    let caret_rect = egui::Rect::from_min_size(
        egui::pos2(bar_x - caret_gap - caret_w, bar_y),
        egui::vec2(caret_w, row_h),
    );
    let pointer_pos_early = ui.input(|i| i.pointer.hover_pos());
    let caret_hovered = pointer_pos_early.is_some_and(|p| caret_rect.contains(p));
    let caret_clicked = caret_hovered && ui.input(|i| i.pointer.primary_clicked());
    if caret_clicked {
        search.replace_active = !search.replace_active;
    }
    if caret_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Draw caret button background + border
    {
        let painter = ui.painter();
        let caret_bg = if caret_hovered || search.replace_active {
            colors::hover_bg(dark_mode)
        } else if dark_mode {
            egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
        } else {
            egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
        };
        painter.add(RectShape::new(
            caret_rect,
            egui::CornerRadius::same(8),
            caret_bg,
            egui::Stroke::NONE,
            StrokeKind::Outside,
        ));
        let caret_border = wobble::rough_rounded_rect(
            caret_rect,
            8.0,
            4444,
            &RoughOptions {
                roughness: 0.5,
                max_randomness_offset: 1.0,
                bowing: 0.5,
                ..Default::default()
            },
        );
        let caret_stroke = egui::Stroke::new(1.0, colors::border_color(dark_mode));
        for path in caret_border {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, caret_stroke));
            }
        }
        // Draw ▶ or ▼ symbol
        let caret_char = if search.replace_active { "▼" } else { "▶" };
        painter.text(
            caret_rect.center(),
            egui::Align2::CENTER_CENTER,
            caret_char,
            egui::FontId::proportional(10.0),
            colors::ui_text_muted(dark_mode),
        );
    }

    // Pre-compute shapes to add (avoids holding painter borrow across new_child)
    let mut shapes: Vec<egui::Shape> = Vec::new();

    // Shadow
    let shadow_rect = bar_rect.translate(egui::vec2(3.0, 3.0));
    shapes.push(
        RectShape::new(
            shadow_rect,
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
    let border_paths = wobble::rough_rounded_rect(bar_rect, 8.0, 5555, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            shapes.push(PathShape::line(path, border_stroke).into());
        }
    }

    // Magnifying glass icon (always in first row)
    let icon_cx = bar_rect.min.x + 12.0 + 7.0;
    let icon_cy = bar_rect.min.y + row_h / 2.0;
    let icon_color = egui::Color32::from_rgb(150, 150, 150);
    let icon_r = 5.0;
    let n_pts = 16;
    let mut circle_pts = Vec::with_capacity(n_pts + 1);
    for i in 0..=n_pts {
        let angle = std::f32::consts::TAU * (i as f32) / (n_pts as f32);
        circle_pts.push(egui::pos2(
            icon_cx + icon_r * angle.cos(),
            icon_cy + icon_r * angle.sin(),
        ));
    }
    shapes.push(PathShape::line(circle_pts, egui::Stroke::new(1.5, icon_color)).into());
    // Handle
    let handle_angle: f32 = std::f32::consts::FRAC_PI_4;
    let handle_start = egui::pos2(
        icon_cx + icon_r * handle_angle.cos(),
        icon_cy + icon_r * handle_angle.sin(),
    );
    let handle_end = egui::pos2(
        icon_cx + (icon_r + 4.0) * handle_angle.cos(),
        icon_cy + (icon_r + 4.0) * handle_angle.sin(),
    );
    shapes.push(egui::Shape::line_segment(
        [handle_start, handle_end],
        egui::Stroke::new(1.5, icon_color),
    ));

    // Add all pre-computed shapes
    ui.painter().extend(shapes);

    // TextEdit input area (first row only)
    let input_x = bar_rect.min.x + 12.0 + 14.0 + 8.0;
    let input_w = bar_w - 12.0 - 14.0 - 8.0 - 8.0 - 60.0 - 8.0 - 20.0 - 12.0;
    let input_rect = egui::Rect::from_min_size(
        egui::pos2(input_x, bar_rect.min.y + 4.0),
        egui::vec2(input_w, row_h - 8.0),
    );

    // TextEdit for search input (requires mutable borrow of ui)
    let text_edit_id = egui::Id::new("search_text_edit");
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(input_rect));
    let te_response = child_ui.add(
        egui::TextEdit::singleline(&mut search.query)
            .font(egui::FontId::proportional(14.0))
            .text_color(colors::ui_text(dark_mode))
            .frame(false)
            .hint_text(
                egui::RichText::new("Search nodes...").color(colors::ui_text_muted(dark_mode)),
            )
            .desired_width(input_w)
            .id(text_edit_id),
    );
    te_response.request_focus();

    // Select all text if Ctrl+F was pressed while search was already open
    if search.select_all_pending {
        search.select_all_pending = false;
        let mut state = egui::TextEdit::load_state(ui.ctx(), text_edit_id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(search.query.len()),
            )));
        egui::TextEdit::store_state(ui.ctx(), text_edit_id, state);
    }

    // If TextEdit lost focus (Escape pressed inside it), close the search bar
    if te_response.lost_focus() {
        let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if escape_pressed {
            return SearchBarAction::Close;
        }
        // Re-focus if lost focus for any other reason (e.g. clicking elsewhere)
        te_response.request_focus();
    }

    // Handle Tab/Shift+Tab for next/prev, Enter to zoom to current match
    let keys = ui.input(|i| i.events.clone());
    for event in &keys {
        match event {
            egui::Event::Key {
                key: egui::Key::Tab,
                pressed: true,
                modifiers,
                ..
            } => {
                if modifiers.shift {
                    action = SearchBarAction::Prev;
                } else {
                    action = SearchBarAction::Next;
                }
            }
            egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                ..
            } => {
                action = SearchBarAction::ZoomTo;
            }
            _ => {}
        }
    }

    // First-row vertical center (used for X button and counter)
    let row1_cy = bar_rect.min.y + row_h / 2.0;

    // X button hit test (need pointer info before painting)
    let x_btn_size = 20.0;
    let x_btn_rect = egui::Rect::from_min_size(
        egui::pos2(
            bar_rect.max.x - 12.0 - x_btn_size,
            row1_cy - x_btn_size / 2.0,
        ),
        egui::vec2(x_btn_size, x_btn_size),
    );
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let x_hovered = pointer_pos.is_some_and(|p| x_btn_rect.contains(p));
    let x_clicked = x_hovered && ui.input(|i| i.pointer.primary_clicked());

    if x_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if x_clicked {
        action = SearchBarAction::Close;
    }

    // Now paint the counter and X button (new painter borrow)
    let painter = ui.painter();

    // Counter display
    if !search.query.is_empty() {
        let counter_x = bar_rect.max.x - 12.0 - 20.0 - 8.0 - 60.0;
        let counter_text;
        let counter_color;
        if search.matches.is_empty() {
            counter_text = "0 / 0".to_string();
            counter_color = egui::Color32::from_rgb(200, 100, 100);
        } else {
            counter_text = format!("{} / {}", search.current_index + 1, search.matches.len());
            counter_color = egui::Color32::from_rgb(150, 150, 150);
        }
        painter.text(
            egui::pos2(counter_x + 30.0, row1_cy),
            egui::Align2::CENTER_CENTER,
            counter_text,
            egui::FontId::proportional(13.0),
            counter_color,
        );
    }

    // X hover background
    if x_hovered {
        painter.circle_filled(
            x_btn_rect.center(),
            x_btn_size / 2.0,
            colors::hover_bg(dark_mode),
        );
    }

    // X lines (wobbled)
    let x_color = if x_hovered {
        egui::Color32::from_rgb(80, 80, 80)
    } else {
        egui::Color32::from_rgb(150, 150, 150)
    };
    let x_cx = x_btn_rect.center().x;
    let x_cy = x_btn_rect.center().y;
    let x_half = 4.0;
    let x_stroke = egui::Stroke::new(1.5, x_color);
    let x_line_opts = RoughOptions {
        roughness: 0.6,
        max_randomness_offset: 0.8,
        bowing: 0.3,
        disable_multi_stroke: true,
        ..Default::default()
    };
    let x1_paths = wobble::rough_line(
        egui::pos2(x_cx - x_half, x_cy - x_half),
        egui::pos2(x_cx + x_half, x_cy + x_half),
        6666,
        &x_line_opts,
    );
    let x2_paths = wobble::rough_line(
        egui::pos2(x_cx + x_half, x_cy - x_half),
        egui::pos2(x_cx - x_half, x_cy + x_half),
        6677,
        &x_line_opts,
    );
    for path in x1_paths.into_iter().chain(x2_paths.into_iter()) {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, x_stroke));
        }
    }

    // --- Replace row (drawn when replace_active) ---
    if search.replace_active {
        let row2_y = bar_rect.min.y + row_h + 4.0;
        let row2_cy = row2_y + row_h / 2.0;

        // Divider line between rows
        let div_paths = wobble::rough_line(
            egui::pos2(bar_rect.min.x + 12.0, bar_rect.min.y + row_h + 2.0),
            egui::pos2(bar_rect.max.x - 12.0, bar_rect.min.y + row_h + 2.0),
            7777,
            &RoughOptions {
                roughness: 0.4,
                max_randomness_offset: 0.6,
                bowing: 0.2,
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

        // Replace icon (arrows ↔ style)
        painter.text(
            egui::pos2(bar_rect.min.x + 12.0 + 7.0, row2_cy),
            egui::Align2::CENTER_CENTER,
            "↔",
            egui::FontId::proportional(13.0),
            colors::ui_text_muted(dark_mode),
        );

        // Replace text input
        let repl_input_x = bar_rect.min.x + 12.0 + 14.0 + 8.0;
        let btn_w = 60.0;
        let btn_gap = 4.0;
        let repl_input_w = bar_w - 12.0 - 14.0 - 8.0 - 8.0 - btn_w * 2.0 - btn_gap - 8.0;
        let repl_rect = egui::Rect::from_min_size(
            egui::pos2(repl_input_x, row2_y + 4.0),
            egui::vec2(repl_input_w, row_h - 8.0),
        );
        let mut child2 = ui.new_child(egui::UiBuilder::new().max_rect(repl_rect));
        child2.add(
            egui::TextEdit::singleline(&mut search.replace_text)
                .font(egui::FontId::proportional(14.0))
                .text_color(colors::ui_text(dark_mode))
                .frame(false)
                .hint_text(
                    egui::RichText::new("Replace with...").color(colors::ui_text_muted(dark_mode)),
                )
                .desired_width(repl_input_w),
        );

        // "Replace" button
        let btn1_x = bar_rect.max.x - 8.0 - btn_w * 2.0 - btn_gap;
        let btn1_rect = egui::Rect::from_min_size(
            egui::pos2(btn1_x, row2_y + 4.0),
            egui::vec2(btn_w, row_h - 8.0),
        );
        let btn1_hovered = pointer_pos.is_some_and(|p| btn1_rect.contains(p));
        let btn1_clicked = btn1_hovered && ui.input(|i| i.pointer.primary_clicked());

        // "All" button
        let btn2_x = btn1_x + btn_w + btn_gap;
        let btn2_rect = egui::Rect::from_min_size(
            egui::pos2(btn2_x, row2_y + 4.0),
            egui::vec2(btn_w, row_h - 8.0),
        );
        let btn2_hovered = pointer_pos.is_some_and(|p| btn2_rect.contains(p));
        let btn2_clicked = btn2_hovered && ui.input(|i| i.pointer.primary_clicked());

        if btn1_hovered || btn2_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let painter2 = ui.painter();
        for (btn_rect, label, hovered) in &[
            (btn1_rect, "Replace", btn1_hovered),
            (btn2_rect, "All", btn2_hovered),
        ] {
            let seed = if *label == "Replace" {
                8881u32
            } else {
                8882u32
            };
            let bg = if *hovered {
                colors::selected_bg(dark_mode)
            } else {
                colors::hover_bg(dark_mode)
            };
            painter2.add(RectShape::new(
                *btn_rect,
                egui::CornerRadius::same(5),
                bg,
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
            let btn_border_paths = wobble::rough_rounded_rect(
                *btn_rect,
                5.0,
                seed,
                &RoughOptions {
                    roughness: 0.4,
                    max_randomness_offset: 0.8,
                    bowing: 0.3,
                    ..Default::default()
                },
            );
            let btn_stroke = egui::Stroke::new(1.0, colors::border_color(dark_mode));
            for path in btn_border_paths {
                if path.len() >= 2 {
                    painter2.add(PathShape::line(path, btn_stroke));
                }
            }
            painter2.text(
                btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                *label,
                egui::FontId::proportional(13.0),
                colors::ui_text(dark_mode),
            );
        }

        if btn1_clicked && action == SearchBarAction::None {
            action = SearchBarAction::ReplaceOne;
        } else if btn2_clicked && action == SearchBarAction::None {
            action = SearchBarAction::ReplaceAll;
        }
    }

    action
}

pub(crate) fn compute_all_nodes_bounds(tree: &MindmapTree) -> egui::Rect {
    // Use visible nodes only — folded-away nodes have stale layout_pos
    // and iterating 1M arena slots per frame is too slow for large files.
    compute_tree_bounds(tree)
}

pub(crate) fn draw_minimap(
    painter: &egui::Painter,
    tree: &MindmapTree,
    viewport: &Viewport,
    screen_rect: egui::Rect,
    color_config: &DepthColorConfig,
    dark_mode: bool,
) -> egui::Rect {
    let mm_w = 200.0_f32;
    let mm_h = 120.0_f32;
    let status_bar_h = 28.0_f32;
    let margin = 8.0_f32;
    let minimap_rect = egui::Rect::from_min_size(
        egui::pos2(
            screen_rect.max.x - mm_w - margin,
            screen_rect.max.y - status_bar_h - margin - mm_h,
        ),
        egui::vec2(mm_w, mm_h),
    );

    let bounds = compute_all_nodes_bounds(tree);
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return minimap_rect;
    }

    // Drop shadow
    painter.add(RectShape::new(
        minimap_rect.translate(egui::vec2(3.0, 4.0)),
        egui::CornerRadius::same(6),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 35),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Panel bg — slightly off-white to contrast with the canvas
    let panel_bg = if dark_mode {
        egui::Color32::from_rgba_premultiplied(45, 45, 50, 245)
    } else {
        egui::Color32::from_rgba_premultiplied(242, 240, 236, 245)
    };
    painter.add(RectShape::new(
        minimap_rect,
        egui::CornerRadius::same(6),
        panel_bg,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Border — a bit thicker/darker for visibility
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(minimap_rect, 6.0, 8765, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Scale to fit bounds into minimap with centering
    let scale = (mm_w / bounds.width()).min(mm_h / bounds.height());
    let scaled_w = bounds.width() * scale;
    let scaled_h = bounds.height() * scale;
    let offset_x = (mm_w - scaled_w) / 2.0;
    let offset_y = (mm_h - scaled_h) / 2.0;

    let canvas_to_mm = |cp: egui::Pos2| -> egui::Pos2 {
        egui::pos2(
            minimap_rect.min.x + offset_x + (cp.x - bounds.min.x) * scale,
            minimap_rect.min.y + offset_y + (cp.y - bounds.min.y) * scale,
        )
    };

    // Draw visible nodes as tiny rects (cap at 5000 to avoid drawing 1M sub-pixel rects)
    let visible = tree.visible_nodes();
    let minimap_limit = 5000;
    let vis_slice = if visible.len() > minimap_limit { &visible[..minimap_limit] } else { &visible };
    for &nid in vis_slice {
        let node = &tree.nodes[nid];
        let depth = node.cached_depth;
        let palette = colors::node_palette_themed(depth, dark_mode, color_config);
        let node_min = canvas_to_mm(egui::pos2(
            node.layout_pos.x - node.layout_size.x / 2.0,
            node.layout_pos.y - node.layout_size.y / 2.0,
        ));
        let node_max = canvas_to_mm(egui::pos2(
            node.layout_pos.x + node.layout_size.x / 2.0,
            node.layout_pos.y + node.layout_size.y / 2.0,
        ));
        let w = (node_max.x - node_min.x).max(2.0);
        let h = (node_max.y - node_min.y).max(1.5);
        let nr = egui::Rect::from_min_size(node_min, egui::vec2(w, h));
        // Clip to minimap
        if minimap_rect.intersects(nr) {
            let f = palette.fill;
            let mm_fill = if dark_mode {
                // Dark mode fills are already dark — lighten them so they show against the dark panel
                egui::Color32::from_rgb(
                    (f.r() as u32 * 5 / 2).min(255) as u8,
                    (f.g() as u32 * 5 / 2).min(255) as u8,
                    (f.b() as u32 * 5 / 2).min(255) as u8,
                )
            } else {
                // Light mode: darken the pastels to ~55% so they read clearly
                egui::Color32::from_rgb(
                    (f.r() as f32 * 0.55) as u8,
                    (f.g() as f32 * 0.55) as u8,
                    (f.b() as f32 * 0.55) as u8,
                )
            };
            painter.add(RectShape::new(
                nr,
                egui::CornerRadius::same(1),
                mm_fill,
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
        }
    }

    // Draw viewport indicator
    let vis = viewport.canvas_visible_rect(screen_rect);
    let vp_min = canvas_to_mm(vis.min);
    let vp_max = canvas_to_mm(vis.max);
    let vp_rect = egui::Rect::from_min_max(
        egui::pos2(
            vp_min.x.max(minimap_rect.min.x),
            vp_min.y.max(minimap_rect.min.y),
        ),
        egui::pos2(
            vp_max.x.min(minimap_rect.max.x),
            vp_max.y.min(minimap_rect.max.y),
        ),
    );
    if vp_rect.width() > 0.0 && vp_rect.height() > 0.0 {
        painter.add(RectShape::new(
            vp_rect,
            egui::CornerRadius::same(2),
            egui::Color32::from_rgba_premultiplied(30, 136, 229, 30),
            egui::Stroke::new(
                1.5,
                egui::Color32::from_rgba_premultiplied(30, 136, 229, 160),
            ),
            StrokeKind::Outside,
        ));
    }

    minimap_rect
}

pub(crate) fn compute_tree_bounds(tree: &MindmapTree) -> egui::Rect {
    let visible = tree.visible_nodes();
    if visible.is_empty() {
        return egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO);
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for &id in &visible {
        let node = &tree.nodes[id];
        let half_w = node.layout_size.x / 2.0;
        let half_h = node.layout_size.y / 2.0;
        min_x = min_x.min(node.layout_pos.x - half_w);
        max_x = max_x.max(node.layout_pos.x + half_w);
        min_y = min_y.min(node.layout_pos.y - half_h);
        max_y = max_y.max(node.layout_pos.y + half_h);
    }
    egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y))
}

/// Adjust viewport offset so that the given node is visible on screen.
pub(crate) fn ensure_node_visible(
    node_id: NodeId,
    viewport: &mut Viewport,
    screen_rect: egui::Rect,
    tree: &MindmapTree,
) {
    let node = &tree.nodes[node_id];
    let screen_pos = viewport.canvas_to_screen(node.layout_pos, screen_rect);
    let half_w = (node.layout_size.x / 2.0) * viewport.zoom + 40.0;
    let half_h = (node.layout_size.y / 2.0) * viewport.zoom + 40.0;

    let node_screen_rect =
        egui::Rect::from_center_size(screen_pos, egui::vec2(half_w * 2.0, half_h * 2.0));

    let mut dx = 0.0f32;
    let mut dy = 0.0f32;

    if node_screen_rect.min.x < screen_rect.min.x {
        dx = screen_rect.min.x - node_screen_rect.min.x;
    } else if node_screen_rect.max.x > screen_rect.max.x {
        dx = screen_rect.max.x - node_screen_rect.max.x;
    }

    if node_screen_rect.min.y < screen_rect.min.y {
        dy = screen_rect.min.y - node_screen_rect.min.y;
    } else if node_screen_rect.max.y > screen_rect.max.y {
        dy = screen_rect.max.y - node_screen_rect.max.y;
    }

    if dx != 0.0 || dy != 0.0 {
        viewport.offset += egui::vec2(dx, dy);
    }
}
