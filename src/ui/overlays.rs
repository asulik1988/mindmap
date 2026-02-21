use crate::style::colors::{self, DepthColorConfig};
use crate::style::wobble::{self, RoughOptions};
use eframe::egui;
use egui::epaint::{PathShape, RectShape, StrokeKind};
use std::path::PathBuf;

pub(crate) enum WelcomeAction {
    None,
    NewMap,
    OpenFile,
    OpenRecentFile(usize),
}

pub(crate) fn draw_welcome_screen(
    ui: &egui::Ui,
    screen_rect: egui::Rect,
    color_config: &DepthColorConfig,
    recent_files: &[PathBuf],
) -> WelcomeAction {
    let painter = ui.painter();
    let cx = screen_rect.center().x;

    // Position content group at ~35% vertical height (shift up to leave room for recent files)
    let group_top = screen_rect.min.y + screen_rect.height() * 0.35 - 95.0;
    let mut y = group_top;

    // Title
    let title_galley = painter.layout_no_wrap(
        "Mindmap".to_string(),
        egui::FontId::proportional(36.0),
        egui::Color32::from_rgb(30, 30, 30),
    );
    painter.galley(
        egui::pos2(cx - title_galley.size().x / 2.0, y),
        title_galley.clone(),
        egui::Color32::from_rgb(30, 30, 30),
    );
    y += title_galley.size().y + 20.0;

    // Subtitle
    let sub_galley = painter.layout_no_wrap(
        "Create or open a mind map".to_string(),
        egui::FontId::proportional(16.0),
        egui::Color32::from_rgb(107, 107, 107),
    );
    painter.galley(
        egui::pos2(cx - sub_galley.size().x / 2.0, y),
        sub_galley.clone(),
        egui::Color32::from_rgb(107, 107, 107),
    );
    y += sub_galley.size().y + 40.0;

    // Button dimensions
    let btn_w = 240.0;
    let btn_h = 56.0;
    let btn_gap = 32.0;
    let total_w = btn_w * 2.0 + btn_gap;

    let new_rect =
        egui::Rect::from_min_size(egui::pos2(cx - total_w / 2.0, y), egui::vec2(btn_w, btn_h));
    let open_rect = egui::Rect::from_min_size(
        egui::pos2(cx - total_w / 2.0 + btn_w + btn_gap, y),
        egui::vec2(btn_w, btn_h),
    );

    // Hover detection
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let new_hovered = pointer_pos.map_or(false, |p| new_rect.contains(p));
    let open_hovered = pointer_pos.map_or(false, |p| open_rect.contains(p));

    // Draw buttons (use depth-1 and depth-2 palette colors)
    let btn_fill_1 = colors::node_palette(1, color_config).fill;
    let btn_fill_2 = colors::node_palette(2, color_config).fill;
    draw_welcome_button(
        painter,
        new_rect,
        "New Mind Map",
        btn_fill_1,
        new_hovered,
        42,
    );
    draw_welcome_button(
        painter,
        open_rect,
        "Open Existing File",
        btn_fill_2,
        open_hovered,
        77,
    );

    y += btn_h + 24.0;

    // Hint text
    let hint_galley = painter.layout_no_wrap(
        "or drag and drop a .mm file".to_string(),
        egui::FontId::proportional(14.0),
        egui::Color32::from_rgb(155, 155, 155),
    );
    painter.galley(
        egui::pos2(cx - hint_galley.size().x / 2.0, y),
        hint_galley.clone(),
        egui::Color32::from_rgb(155, 155, 155),
    );
    y += hint_galley.size().y;

    // Recent files section
    let mut recent_hovered: Option<usize> = None;
    let section_w = 512.0;
    let row_h = 48.0;
    let max_recent = 5;
    let entries: Vec<&PathBuf> = recent_files.iter().take(max_recent).collect();

    if !entries.is_empty() {
        y += 32.0;

        // Wobbly divider
        let div_x0 = cx - section_w / 2.0 + (section_w - 280.0) / 2.0;
        let div_x1 = div_x0 + 280.0;
        let div_paths = wobble::rough_line(
            egui::pos2(div_x0, y),
            egui::pos2(div_x1, y),
            12345,
            &RoughOptions {
                roughness: 0.4,
                max_randomness_offset: 0.8,
                bowing: 0.3,
                disable_multi_stroke: true,
                ..Default::default()
            },
        );
        let div_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0xD0, 0xC8, 0xC0));
        for path in div_paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, div_stroke));
            }
        }
        y += 24.0;

        // "Recent" label
        let recent_label = painter.layout_no_wrap(
            "Recent".to_string(),
            egui::FontId::proportional(14.0),
            egui::Color32::from_rgb(0x9B, 0x9B, 0x9B),
        );
        painter.galley(
            egui::pos2(cx - recent_label.size().x / 2.0, y),
            recent_label.clone(),
            egui::Color32::from_rgb(0x9B, 0x9B, 0x9B),
        );
        y += recent_label.size().y + 12.0;

        // Each recent file row
        let row_x = cx - section_w / 2.0;
        for (idx, path) in entries.iter().enumerate() {
            let row_rect =
                egui::Rect::from_min_size(egui::pos2(row_x, y), egui::vec2(section_w, row_h));
            let hovered = pointer_pos.map_or(false, |p| row_rect.contains(p));
            if hovered {
                recent_hovered = Some(idx);
                painter.rect_filled(
                    row_rect,
                    egui::CornerRadius::same(6),
                    egui::Color32::from_rgb(0xF0, 0xED, 0xE8),
                );
            }

            // Filename
            let filename = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name_galley = painter.layout_no_wrap(
                filename,
                egui::FontId::proportional(15.0),
                egui::Color32::from_rgb(0x1E, 0x1E, 0x1E),
            );
            let text_x = row_x + 12.0;
            let name_y = y + (row_h / 2.0) - name_galley.size().y - 1.0;
            painter.galley(
                egui::pos2(text_x, name_y),
                name_galley.clone(),
                egui::Color32::from_rgb(0x1E, 0x1E, 0x1E),
            );

            // Parent dir
            let parent_str = path
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let parent_max_w = section_w - 24.0;
            let parent_galley = painter.layout(
                parent_str,
                egui::FontId::proportional(12.0),
                egui::Color32::from_rgb(0x9B, 0x9B, 0x9B),
                parent_max_w,
            );
            let parent_y = y + (row_h / 2.0) + 1.0;
            painter.galley(
                egui::pos2(text_x, parent_y),
                parent_galley,
                egui::Color32::from_rgb(0x9B, 0x9B, 0x9B),
            );

            y += row_h;
        }
    }

    // Cursor feedback
    if new_hovered || open_hovered || recent_hovered.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Click detection
    if ui.input(|i| i.pointer.primary_clicked()) {
        if new_hovered {
            return WelcomeAction::NewMap;
        } else if open_hovered {
            return WelcomeAction::OpenFile;
        } else if let Some(idx) = recent_hovered {
            return WelcomeAction::OpenRecentFile(idx);
        }
    }

    WelcomeAction::None
}

pub(crate) fn draw_welcome_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    fill_color: egui::Color32,
    hovered: bool,
    seed: u32,
) {
    let rounding = 12.0;
    let stroke_color = egui::Color32::from_rgb(30, 30, 30);
    let stroke_width = if hovered { 2.0 } else { 1.5 };
    let hatch_stroke_width = if hovered { 2.5 } else { 2.0 };

    // 1. White background
    painter.add(RectShape::new(
        rect,
        egui::CornerRadius::same(rounding as u8),
        egui::Color32::WHITE,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // 2. Hachure fill
    let hatch_opts = RoughOptions {
        roughness: 0.8,
        max_randomness_offset: 1.5,
        bowing: 1.0,
        disable_multi_stroke: true,
        ..Default::default()
    };
    let hatch_paths = wobble::hachure_fill_rect(
        rect,
        -41.0,
        5.0,
        seed.wrapping_mul(2654435761).wrapping_add(5555),
        &hatch_opts,
    );
    let hatch_stroke = egui::Stroke::new(hatch_stroke_width, fill_color);
    for path in hatch_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, hatch_stroke));
        }
    }

    // 3. Wobbled border
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(rect, rounding, seed, &rough_opts);
    let border_stroke = egui::Stroke::new(stroke_width, stroke_color);
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // 4. Centered label
    let galley = painter.layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(16.0),
        egui::Color32::from_rgb(30, 30, 30),
    );
    painter.galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        egui::Color32::from_rgb(30, 30, 30),
    );
}

/// Returns true if the overlay was dismissed.
pub(crate) fn draw_help_overlay(
    ui: &mut egui::Ui,
    screen_rect: egui::Rect,
    dark_mode: bool,
) -> bool {
    let panel_w = 500.0;
    let col_label_w = 120.0;
    let row_h = 22.0;
    let pad = 16.0;

    // Two columns of shortcuts
    let left_shortcuts: &[(&str, &str)] = &[
        ("F2", "Edit node"),
        ("Tab", "Add child"),
        ("Enter", "Add sibling"),
        ("Del", "Delete node"),
        ("Ctrl+.", "Fold / Unfold"),
        ("Ctrl+Shift+-", "Fold all"),
        ("Ctrl+Shift+=", "Unfold all"),
        ("F / Home", "Fit to screen"),
        ("Ctrl+X/C/V", "Cut / Copy / Paste"),
        ("Ctrl+Shift+N", "Notes panel"),
    ];
    let right_shortcuts: &[(&str, &str)] = &[
        ("Ctrl+Z", "Undo"),
        ("Ctrl+Shift+Z", "Redo"),
        ("Ctrl+F", "Search / Replace"),
        ("Ctrl+B", "Bold"),
        ("Ctrl+S", "Save"),
        ("Ctrl+Shift+S", "Save As"),
        ("Ctrl+O", "Open file"),
        ("Ctrl+N", "New map"),
        ("Escape", "Close panels"),
        ("?", "This help"),
    ];

    let rows = left_shortcuts.len().max(right_shortcuts.len()) as f32;
    let title_h = 36.0;
    let footer_h = 28.0;
    let panel_h = pad * 2.0 + title_h + rows * row_h + footer_h;

    let cx = screen_rect.center().x;
    let cy = screen_rect.center().y;
    let panel_rect = egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(panel_w, panel_h));

    let painter = ui.painter();

    // Dim overlay
    painter.rect_filled(
        screen_rect,
        0.0,
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 60),
    );

    // Shadow
    let shadow_rect = panel_rect.translate(egui::vec2(4.0, 4.0));
    painter.add(RectShape::new(
        shadow_rect,
        egui::CornerRadius::same(10),
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 30),
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    // Background
    painter.add(RectShape::new(
        panel_rect,
        egui::CornerRadius::same(10),
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
    let border_paths = wobble::rough_rounded_rect(panel_rect, 10.0, 9999, &rough_opts);
    let border_stroke = egui::Stroke::new(1.5, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Title
    let mut y = panel_rect.min.y + pad;
    painter.text(
        egui::pos2(cx, y + title_h / 2.0),
        egui::Align2::CENTER_CENTER,
        "Keyboard Shortcuts",
        egui::FontId::proportional(18.0),
        colors::ui_text(dark_mode),
    );
    y += title_h;

    // Title divider
    let div_paths = wobble::rough_line(
        egui::pos2(panel_rect.min.x + 16.0, y),
        egui::pos2(panel_rect.max.x - 16.0, y),
        11111,
        &RoughOptions {
            roughness: 0.4,
            max_randomness_offset: 0.7,
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

    // Two columns layout
    let col_w = panel_w / 2.0;
    let key_color = colors::ui_text_muted(dark_mode);
    let action_color = colors::ui_text(dark_mode);

    for (col_idx, shortcuts) in [left_shortcuts, right_shortcuts].iter().enumerate() {
        let col_x = panel_rect.min.x + col_idx as f32 * col_w + 16.0;
        for (row_idx, (key, action)) in shortcuts.iter().enumerate() {
            let ry = y + row_idx as f32 * row_h + row_h / 2.0;
            painter.text(
                egui::pos2(col_x, ry),
                egui::Align2::LEFT_CENTER,
                *key,
                egui::FontId::proportional(13.0),
                key_color,
            );
            painter.text(
                egui::pos2(col_x + col_label_w, ry),
                egui::Align2::LEFT_CENTER,
                *action,
                egui::FontId::proportional(13.0),
                action_color,
            );
        }
    }

    y += rows * row_h;

    // Footer hint
    painter.text(
        egui::pos2(cx, y + footer_h / 2.0),
        egui::Align2::CENTER_CENTER,
        "Press ? or Escape to close",
        egui::FontId::proportional(12.0),
        colors::ui_text_muted(dark_mode),
    );

    // Dismiss on click outside the panel
    let clicked_outside = ui.input(|i| {
        if i.pointer.primary_clicked() {
            i.pointer
                .hover_pos()
                .map_or(true, |p| !panel_rect.contains(p))
        } else {
            false
        }
    });
    clicked_outside
}
