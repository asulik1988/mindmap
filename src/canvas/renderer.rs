use super::{edge_renderer, grid, node_renderer, viewport::Viewport};
use crate::interaction::input::DragState;
use crate::model::{MindmapTree, Selection};
use crate::style::colors::DepthColorConfig;
use crate::style::wobble::{self, RoughOptions};
use crate::model::NodeId;
use egui::{epaint::PathShape, Color32, Painter, Rect, Stroke};
use std::collections::{HashMap, HashSet};

/// Node screen rects for hit testing, keyed by NodeId.
pub type NodeRects = HashMap<usize, Rect>;

/// Main render function: draws the entire canvas.
pub fn draw_canvas(
    painter: &Painter,
    tree: &MindmapTree,
    viewport: &Viewport,
    screen_rect: Rect,
    selection: &Selection,
    drag_state: &Option<DragState>,
    color_config: &DepthColorConfig,
    search_matches: &HashSet<NodeId>,
    search_current: Option<NodeId>,
    dark_mode: bool,
) -> NodeRects {
    // 1. Draw grid background
    grid::draw_grid(painter, viewport, screen_rect, dark_mode);

    // 2. Get visible nodes
    let visible = tree.visible_nodes();

    // 3. Draw edges (behind nodes)
    edge_renderer::draw_edges(painter, tree, &visible, viewport, screen_rect, color_config, dark_mode);

    let dragged_node_id = drag_state.as_ref().map(|ds| ds.node_id);

    // 4. Draw nodes and collect screen rects
    let mut node_rects = HashMap::new();
    for &node_id in &visible {
        let node = &tree.nodes[node_id];
        let depth = tree.depth(node_id);

        // Viewport culling: skip nodes far off-screen
        let screen_pos = viewport.canvas_to_screen(node.layout_pos, screen_rect);
        let margin = 400.0; // generous margin for large nodes
        if screen_pos.x < screen_rect.min.x - margin
            || screen_pos.x > screen_rect.max.x + margin
            || screen_pos.y < screen_rect.min.y - margin
            || screen_pos.y > screen_rect.max.y + margin
        {
            continue;
        }

        // Dim the dragged node to 30% opacity
        let alpha = if dragged_node_id == Some(node_id) { 0.3 } else { 1.0 };
        let is_search_match = search_matches.contains(&node_id);
        let is_current_search = search_current == Some(node_id);
        let rect = node_renderer::draw_node(painter, node, depth, viewport, screen_rect, selection, alpha, color_config, is_search_match, is_current_search, dark_mode);
        node_rects.insert(node_id, rect);
    }

    // Draw drop target highlight (wobbled style to match hand-drawn aesthetic)
    if let Some(ds) = drag_state {
        if let Some(target_id) = ds.drop_target {
            if let Some(&rect) = node_rects.get(&target_id) {
                let highlight_rect = rect.expand(4.0);
                let rounding = 10.0 * viewport.zoom;
                let seed = (target_id as u32).wrapping_mul(2654435761).wrapping_add(7777);

                // Wobbled highlight border
                let rough_opts = RoughOptions {
                    roughness: 0.5,
                    max_randomness_offset: 1.0,
                    bowing: 0.5,
                    ..Default::default()
                };
                let paths = wobble::rough_rounded_rect(highlight_rect, rounding, seed, &rough_opts);
                let stroke = Stroke::new(2.5, Color32::from_rgba_premultiplied(66, 133, 244, 180));
                for path in paths {
                    if path.len() >= 2 {
                        painter.add(PathShape::line(path, stroke));
                    }
                }
            }
        }

        // Draw ghost node last (always on top)
        let ghost_center = ds.cursor_pos - ds.grab_offset;
        let node = &tree.nodes[ds.node_id];
        let depth = tree.depth(ds.node_id);
        node_renderer::draw_node_ghost(
            painter,
            node,
            depth,
            viewport,
            ghost_center,
            0.5,  // 50% opacity
            2.5,  // 2.5 degree clockwise tilt
            color_config,
            dark_mode,
        );
    }

    node_rects
}
