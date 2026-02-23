use crate::layout::spacing::sibling_gap;
use egui::{Pos2, Rect, Vec2};

const DEFAULT_ZOOM_FLOOR: f32 = 0.02;
const MAX_ZOOM: f32 = 4.0;
const MAX_FIT_ZOOM: f32 = 2.0;

/// Compute the minimum zoom level for a tree with the given max depth.
/// For shallow trees (depth <= 100 visible nodes vertically), returns the default floor.
/// For deep trees, returns a higher floor so that ~100 vertical nodes remain
/// visible and readable near the root.
pub fn depth_zoom_floor(max_depth: usize, screen_height: f32) -> f32 {
    const MAX_VISIBLE_NODES: usize = 100;

    if max_depth <= MAX_VISIBLE_NODES {
        return DEFAULT_ZOOM_FLOOR;
    }

    // Approximate vertical space per node: sibling gap + typical node height (~40px).
    // Use depth-0 sibling gap at zoom 1.0 as reference.
    let node_spacing = sibling_gap(0, 1.0) + 40.0;
    let total_height = MAX_VISIBLE_NODES as f32 * node_spacing;
    (screen_height / total_height).max(DEFAULT_ZOOM_FLOOR)
}

pub struct Viewport {
    pub offset: Vec2, // pan offset in screen pixels
    pub zoom: f32,    // 1.0 = 100%
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

    pub fn zoom_around(
        &mut self,
        screen_pivot: Pos2,
        delta: f32,
        screen_rect: Rect,
        min_zoom: f32,
    ) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * (1.0 + delta)).clamp(min_zoom, MAX_ZOOM);

        // Adjust offset so the point under the cursor stays fixed
        let center = screen_rect.center();
        let pivot_rel = screen_pivot - center;
        self.offset = pivot_rel - (pivot_rel - self.offset) * (self.zoom / old_zoom);
    }

    /// Fit all given canvas positions into the viewport with padding.
    /// If the tree is too large to fit at the minimum zoom, centers on the
    /// canvas origin (where root lives) so the user can navigate from there.
    pub fn fit_to_bounds(&mut self, bounds: Rect, screen_rect: Rect, padding: f32, min_zoom: f32) {
        if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
            return;
        }

        let available_w = screen_rect.width() - padding * 2.0;
        let available_h = screen_rect.height() - padding * 2.0;

        let zoom_w = available_w / bounds.width();
        let zoom_h = available_h / bounds.height();
        let ideal_zoom = zoom_w.min(zoom_h);
        self.zoom = ideal_zoom.clamp(min_zoom, MAX_FIT_ZOOM);

        if ideal_zoom < min_zoom {
            // Tree too large to fit — center on origin (root node) instead
            self.offset = Vec2::ZERO;
        } else {
            // Center the bounds
            let bounds_center = bounds.center();
            self.offset = Vec2::new(-bounds_center.x * self.zoom, -bounds_center.y * self.zoom);
        }
    }
}
