use crate::style::colors;
use crate::style::wobble::{self, RoughOptions};
use eframe::egui;
use egui::epaint::{PathShape, RectShape, StrokeKind};

pub(crate) fn draw_hamburger_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    hovered: bool,
    _menu_open: bool,
    dark_mode: bool,
) {
    let rounding = 8.0;
    let bg_color = if hovered {
        colors::hover_bg(dark_mode)
    } else if dark_mode {
        egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
    } else {
        egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
    };

    // Background
    painter.add(RectShape::new(
        rect,
        egui::CornerRadius::same(rounding as u8),
        bg_color,
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
    let border_paths = wobble::rough_rounded_rect(rect, rounding, 999, &rough_opts);
    let stroke_width = if hovered { 1.5 } else { 1.0 };
    let border_stroke = egui::Stroke::new(stroke_width, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Three wobbly horizontal lines
    let cx = rect.center().x;
    let cy = rect.center().y;
    let line_half_w = 8.0;
    let line_gap = 5.0;
    let line_color = colors::border_color(dark_mode);
    let line_stroke = egui::Stroke::new(1.5, line_color);

    let line_opts = RoughOptions {
        roughness: 0.6,
        max_randomness_offset: 0.8,
        bowing: 0.3,
        disable_multi_stroke: true,
        ..Default::default()
    };

    for (i, dy) in [-line_gap, 0.0, line_gap].iter().enumerate() {
        let y = cy + dy;
        let seed = 1000 + i as u32;
        let paths = wobble::rough_line(
            egui::pos2(cx - line_half_w, y),
            egui::pos2(cx + line_half_w, y),
            seed,
            &line_opts,
        );
        for path in paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, line_stroke));
            }
        }
    }
}

pub(crate) fn draw_search_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    hovered: bool,
    active: bool,
    dark_mode: bool,
) {
    let rounding = 8.0;
    let bg_color = if hovered || active {
        colors::hover_bg(dark_mode)
    } else if dark_mode {
        egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
    } else {
        egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
    };

    painter.add(RectShape::new(
        rect,
        egui::CornerRadius::same(rounding as u8),
        bg_color,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(rect, rounding, 3141, &rough_opts);
    let stroke_width = if hovered || active { 1.5 } else { 1.0 };
    let border_stroke = egui::Stroke::new(stroke_width, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Magnifying glass icon
    let cx = rect.center().x;
    let cy = rect.center().y;
    let icon_color = colors::border_color(dark_mode);
    let icon_r = 6.0;
    let n_pts = 18;
    let mut circle_pts = Vec::with_capacity(n_pts + 1);
    for i in 0..=n_pts {
        let angle = std::f32::consts::TAU * (i as f32) / (n_pts as f32);
        circle_pts.push(egui::pos2(
            cx - 1.5 + icon_r * angle.cos(),
            cy - 1.5 + icon_r * angle.sin(),
        ));
    }
    painter.add(PathShape::line(
        circle_pts,
        egui::Stroke::new(1.5, icon_color),
    ));
    let handle_angle: f32 = std::f32::consts::FRAC_PI_4;
    let handle_start = egui::pos2(
        cx - 1.5 + icon_r * handle_angle.cos(),
        cy - 1.5 + icon_r * handle_angle.sin(),
    );
    let handle_end = egui::pos2(
        cx - 1.5 + (icon_r + 5.0) * handle_angle.cos(),
        cy - 1.5 + (icon_r + 5.0) * handle_angle.sin(),
    );
    painter.line_segment(
        [handle_start, handle_end],
        egui::Stroke::new(2.0, icon_color),
    );
}

pub(crate) fn draw_style_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    hovered: bool,
    _panel_open: bool,
    dark_mode: bool,
) {
    let rounding = 8.0;
    let bg_color = if hovered {
        colors::hover_bg(dark_mode)
    } else if dark_mode {
        egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
    } else {
        egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
    };

    // Background
    painter.add(RectShape::new(
        rect,
        egui::CornerRadius::same(rounding as u8),
        bg_color,
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
    let border_paths = wobble::rough_rounded_rect(rect, rounding, 1234, &rough_opts);
    let stroke_width = if hovered { 1.5 } else { 1.0 };
    let border_stroke = egui::Stroke::new(stroke_width, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Draw a simple palette icon: a circle with colored dots
    let cx = rect.center().x;
    let cy = rect.center().y;
    let icon_color = colors::border_color(dark_mode);

    // Palette circle outline (wobbled)
    let palette_r = 9.0;
    // Draw an oval/circle shape as a palette
    let n_pts = 20;
    let mut pts = Vec::with_capacity(n_pts + 1);
    for i in 0..=n_pts {
        let angle = std::f32::consts::TAU * (i as f32) / (n_pts as f32);
        pts.push(egui::pos2(
            cx + palette_r * angle.cos(),
            cy + palette_r * 0.85 * angle.sin(),
        ));
    }
    painter.add(PathShape::line(pts, egui::Stroke::new(1.2, icon_color)));

    // Colored dots inside
    let dot_r = 2.5;
    let dots = [
        (cx - 4.0, cy - 3.0, egui::Color32::from_rgb(255, 186, 194)), // pink
        (cx + 3.0, cy - 3.0, egui::Color32::from_rgb(164, 216, 255)), // blue
        (cx - 1.0, cy + 3.0, egui::Color32::from_rgb(176, 232, 181)), // green
        (cx + 5.0, cy + 2.0, egui::Color32::from_rgb(255, 244, 168)), // yellow
    ];
    for (dx, dy, color) in dots {
        painter.circle_filled(egui::pos2(dx, dy), dot_r, color);
    }
}

pub(crate) fn draw_notes_button(
    painter: &egui::Painter,
    rect: egui::Rect,
    hovered: bool,
    active: bool,
    dark_mode: bool,
) {
    let rounding = 8.0;
    let bg_color = if hovered || active {
        colors::hover_bg(dark_mode)
    } else if dark_mode {
        egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
    } else {
        egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
    };

    painter.add(RectShape::new(
        rect,
        egui::CornerRadius::same(rounding as u8),
        bg_color,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));

    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let border_paths = wobble::rough_rounded_rect(rect, rounding, 7878, &rough_opts);
    let stroke_width = if hovered || active { 1.5 } else { 1.0 };
    let border_stroke = egui::Stroke::new(stroke_width, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }

    // Document icon: small rectangle outline with three lines
    let cx = rect.center().x;
    let cy = rect.center().y;
    let icon_color = colors::border_color(dark_mode);
    let doc_x = cx - 5.0;
    let doc_y = cy - 6.0;
    let doc_w = 10.0;
    let doc_h = 12.0;

    // Outline
    painter.rect_stroke(
        egui::Rect::from_min_size(egui::pos2(doc_x, doc_y), egui::vec2(doc_w, doc_h)),
        1.0,
        egui::Stroke::new(1.2, icon_color),
        StrokeKind::Outside,
    );

    // Three horizontal lines
    for y_offset in [2.5_f32, 5.0, 7.5] {
        painter.line_segment(
            [
                egui::pos2(doc_x + 1.5, doc_y + y_offset),
                egui::pos2(doc_x + doc_w - 1.5, doc_y + y_offset),
            ],
            egui::Stroke::new(1.0, icon_color),
        );
    }
}

pub(crate) fn draw_zoom_controls(
    painter: &egui::Painter,
    minus_rect: egui::Rect,
    zoom_display_rect: egui::Rect,
    plus_rect: egui::Rect,
    zoom_pct: i32,
    minus_hovered: bool,
    zoom_hovered: bool,
    plus_hovered: bool,
    dark_mode: bool,
) {
    let rounding = 8.0;
    let rough_opts = RoughOptions {
        roughness: 0.5,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };

    // Draw a single toolbar-style button
    let draw_btn =
        |painter: &egui::Painter, rect: egui::Rect, hovered: bool, label: &str, seed: u32| {
            let bg = if hovered {
                colors::hover_bg(dark_mode)
            } else if dark_mode {
                egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
            } else {
                egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
            };
            painter.add(RectShape::new(
                rect,
                egui::CornerRadius::same(rounding as u8),
                bg,
                egui::Stroke::NONE,
                StrokeKind::Outside,
            ));
            let border_paths = wobble::rough_rounded_rect(rect, rounding, seed, &rough_opts);
            let stroke_w = if hovered { 1.5 } else { 1.0 };
            let border_stroke = egui::Stroke::new(stroke_w, colors::border_color(dark_mode));
            for path in border_paths {
                if path.len() >= 2 {
                    painter.add(PathShape::line(path, border_stroke));
                }
            }
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(14.0),
                colors::ui_text(dark_mode),
            );
        };

    draw_btn(painter, minus_rect, minus_hovered, "−", 7001);
    draw_btn(painter, plus_rect, plus_hovered, "+", 7002);

    // Zoom display (clickable label)
    let display_label = format!("{}%", zoom_pct);
    let bg = if zoom_hovered {
        colors::hover_bg(dark_mode)
    } else if dark_mode {
        egui::Color32::from_rgba_premultiplied(40, 40, 45, 220)
    } else {
        egui::Color32::from_rgba_premultiplied(251, 251, 250, 200)
    };
    painter.add(RectShape::new(
        zoom_display_rect,
        egui::CornerRadius::same(rounding as u8),
        bg,
        egui::Stroke::NONE,
        StrokeKind::Outside,
    ));
    let border_paths = wobble::rough_rounded_rect(zoom_display_rect, rounding, 7003, &rough_opts);
    let stroke_w = if zoom_hovered { 1.5 } else { 1.0 };
    let border_stroke = egui::Stroke::new(stroke_w, colors::border_color(dark_mode));
    for path in border_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, border_stroke));
        }
    }
    painter.text(
        zoom_display_rect.center(),
        egui::Align2::CENTER_CENTER,
        &display_label,
        egui::FontId::proportional(12.0),
        colors::ui_text(dark_mode),
    );
}
