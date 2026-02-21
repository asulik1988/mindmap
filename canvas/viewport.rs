use egui::{Pos2, Rect, Vec2};

pub struct Viewport {
    pub offset: Vec2,  // pan offset in screen pixels
    pub zoom: f32,     // 1.0 = 100%
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl Viewport {
    pub fn screen_to_canvas(&self, screen_pos: Pos2, screen_rect: Rect) -> Pos2 {
        let center = screen_rect.center();
        Pos2::new(
            (screen_pos.x - center.x - self.offset.x) / self.zoom,
            (screen_pos.y - center.y - self.offset.y) / self.zoom,
        )
    }

    pub fn canvas_to_screen(&self, canvas_pos: Pos2, screen_rect: Rect) -> Pos2 {
        let center = screen_rect.center();
        Pos2::new(
            canvas_pos.x * self.zoom + center.x + self.offset.x,
            canvas_pos.y * self.zoom + center.y + self.offset.y,
        )
    }

    pub fn canvas_visible_rect(&self, screen_rect: Rect) -> Rect {
        let top_left = self.screen_to_canvas(screen_rect.min, screen_rect);
        let bottom_right = self.screen_to_canvas(screen_rect.max, screen_rect);
        Rect::from_min_max(top_left, bottom_right)
    }

    pub fn zoom_around(&mut self, screen_pivot: Pos2, delta: f32, screen_rect: Rect) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * (1.0 + delta)).clamp(0.1, 4.0);

        // Adjust offset so the point under the cursor stays fixed
        let center = screen_rect.center();
        let pivot_rel = screen_pivot - center;
        self.offset = pivot_rel - (pivot_rel - self.offset) * (self.zoom / old_zoom);
    }

    /// Fit all given canvas positions into the viewport with padding.
    pub fn fit_to_bounds(&mut self, bounds: Rect, screen_rect: Rect, padding: f32) {
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return;
        }

        let available_w = screen_rect.width() - padding * 2.0;
        let available_h = screen_rect.height() - padding * 2.0;

        let zoom_w = available_w / bounds.width();
        let zoom_h = available_h / bounds.height();
        self.zoom = zoom_w.min(zoom_h).clamp(0.1, 2.0);

        // Center the bounds
        let bounds_center = bounds.center();
        self.offset = Vec2::new(
            -bounds_center.x * self.zoom,
            -bounds_center.y * self.zoom,
        );
    }
}
