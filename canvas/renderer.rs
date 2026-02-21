use super::{edge_renderer, grid, node_renderer, viewport::Viewport};
use crate::model::{MindmapTree, Selection};
use egui::{Painter, Rect};
use std::collections::HashMap;

/// Node screen rects for hit testing, keyed by NodeId.
pub type NodeRects = HashMap<usize, Rect>;

/// Main render function: draws the entire canvas.
pub fn draw_canvas(
    painter: &Painter,
    tree: &MindmapTree,
    viewport: &Viewport,
    screen_rect: Rect,
    selection: &Selection,
) -> NodeRects {
    // 1. Draw grid background
    grid::draw_grid(painter, viewport, screen_rect);

    // 2. Get visible nodes
    let visible = tree.visible_nodes();

    // 3. Draw edges (behind nodes)
    edge_renderer::draw_edges(painter, tree, &visible, viewport, screen_rect);

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

        let rect = node_renderer::draw_node(painter, node, depth, viewport, screen_rect, selection);
        node_rects.insert(node_id, rect);
    }

    node_rects
}
