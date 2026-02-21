use super::node_renderer;
use super::viewport::Viewport;
use crate::model::{MindmapTree, Side};
use crate::style::colors::{self, DepthColorConfig};
use crate::style::wobble::{self, RoughOptions};
use egui::{epaint::PathShape, Color32, Painter, Pos2, Rect, Stroke};

/// Draw bezier edges from parent to each visible child.
pub fn draw_edges(
    painter: &Painter,
    tree: &MindmapTree,
    visible_nodes: &[usize],
    viewport: &Viewport,
    screen_rect: Rect,
    _color_config: &DepthColorConfig,
    dark_mode: bool,
) {
    let base_edge_color = colors::edge_color(dark_mode);
    for &node_id in visible_nodes {
        let node = &tree.nodes[node_id];
        if let Some(parent_id) = node.parent {
            // Only draw if parent is also visible
            if !visible_nodes.contains(&parent_id) {
                continue;
            }

            let parent = &tree.nodes[parent_id];
            let parent_depth = tree.depth(parent_id);
            let child_depth = tree.depth(node_id);

            let parent_size = node_renderer::measure_node(parent, parent_depth, painter);
            let child_size = node_renderer::measure_node(node, child_depth, painter);

            // Determine connection side
            let side = tree.effective_side(node_id).unwrap_or(Side::Right);

            let (source, target) = match side {
                Side::Right => {
                    let src = Pos2::new(
                        parent.layout_pos.x + parent_size.x / 2.0,
                        parent.layout_pos.y,
                    );
                    let tgt = Pos2::new(node.layout_pos.x - child_size.x / 2.0, node.layout_pos.y);
                    (src, tgt)
                }
                Side::Left => {
                    let src = Pos2::new(
                        parent.layout_pos.x - parent_size.x / 2.0,
                        parent.layout_pos.y,
                    );
                    let tgt = Pos2::new(node.layout_pos.x + child_size.x / 2.0, node.layout_pos.y);
                    (src, tgt)
                }
            };

            // Convert to screen coords
            let src_screen = viewport.canvas_to_screen(source, screen_rect);
            let tgt_screen = viewport.canvas_to_screen(target, screen_rect);

            // Bezier control points
            let dx = tgt_screen.x - src_screen.x;
            let cp1 = Pos2::new(src_screen.x + dx * 0.4, src_screen.y);
            let cp2 = Pos2::new(tgt_screen.x - dx * 0.4, tgt_screen.y);

            // Generate roughjs-style hand-drawn edge with double-stroke
            let edge_seed = (parent_id as u32).wrapping_mul(2654435761)
                ^ (node_id as u32).wrapping_mul(2246822519);
            let rough_opts = RoughOptions {
                roughness: 0.5,
                max_randomness_offset: 1.0,
                bowing: 0.3,
                ..Default::default()
            };
            let paths =
                wobble::rough_bezier_edge(src_screen, cp1, cp2, tgt_screen, edge_seed, &rough_opts);

            // Edge color at varying opacity
            let opacity = if child_depth <= 1 { 180 } else { 140 };
            let edge_color = Color32::from_rgba_premultiplied(
                base_edge_color.r(),
                base_edge_color.g(),
                base_edge_color.b(),
                opacity,
            );

            let stroke_width = 1.5 * viewport.zoom;
            let stroke = Stroke::new(stroke_width, edge_color);

            for path in paths {
                if path.len() >= 2 {
                    painter.add(PathShape::line(path, stroke));
                }
            }
        }
    }
}
