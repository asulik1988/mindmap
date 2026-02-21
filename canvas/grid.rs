use super::viewport::Viewport;
use egui::{Color32, Painter, Pos2, Rect};

const GRID_SPACING: f32 = 20.0;
const DOT_RADIUS: f32 = 1.0;
const DOT_COLOR: Color32 = Color32::from_rgba_premultiplied(51, 50, 47, 20); // #E2E0DC at ~8%

pub fn draw_grid(painter: &Painter, viewport: &Viewport, screen_rect: Rect) {
    let visible = viewport.canvas_visible_rect(screen_rect);

    // Align to grid
    let start_x = (visible.min.x / GRID_SPACING).floor() as i32;
    let end_x = (visible.max.x / GRID_SPACING).ceil() as i32;
    let start_y = (visible.min.y / GRID_SPACING).floor() as i32;
    let end_y = (visible.max.y / GRID_SPACING).ceil() as i32;

    // Skip dots if zoomed too far out (they'd be invisible/too dense)
    if viewport.zoom < 0.3 {
        return;
    }

    let dot_radius = DOT_RADIUS * viewport.zoom;

    for gx in start_x..=end_x {
        for gy in start_y..=end_y {
            let canvas_pos = Pos2::new(gx as f32 * GRID_SPACING, gy as f32 * GRID_SPACING);
            let screen_pos = viewport.canvas_to_screen(canvas_pos, screen_rect);
            painter.circle_filled(screen_pos, dot_radius, DOT_COLOR);
        }
    }
}
