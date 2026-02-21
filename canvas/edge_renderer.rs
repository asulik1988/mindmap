use crate::model::{MindmapNode, MindmapTree, Side};
use crate::style::colors;
use super::node_renderer;
use super::viewport::Viewport;
use egui::{epaint::PathShape, Color32, Painter, Pos2, Rect, Stroke};

/// Draw bezier edges from parent to each visible child.
pub fn draw_edges(
    painter: &Painter,
    tree: &MindmapTree,
    visible_nodes: &[usize],
    viewport: &Viewport,
    screen_rect: Rect,
) {
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
                    let tgt = Pos2::new(
                        node.layout_pos.x - child_size.x / 2.0,
                        node.layout_pos.y,
                    );
                    (src, tgt)
                }
                Side::Left => {
                    let src = Pos2::new(
                        parent.layout_pos.x - parent_size.x / 2.0,
                        parent.layout_pos.y,
                    );
                    let tgt = Pos2::new(
                        node.layout_pos.x + child_size.x / 2.0,
                        node.layout_pos.y,
                    );
                    (src, tgt)
                }
            };

            // Convert to screen coords
            let src_screen = viewport.canvas_to_screen(source, screen_rect);
            let tgt_screen = viewport.canvas_to_screen(target, screen_rect);

            // Generate bezier curve points
            let dx = tgt_screen.x - src_screen.x;
            let cp1 = Pos2::new(src_screen.x + dx * 0.4, src_screen.y);
            let cp2 = Pos2::new(tgt_screen.x - dx * 0.4, tgt_screen.y);

            let points = cubic_bezier_points(src_screen, cp1, cp2, tgt_screen, 20);

            // Edge color: parent stroke color at reduced opacity
            let palette = colors::node_palette(parent_depth);
            let opacity = if child_depth <= 2 { 140 } else { 100 };
            let edge_color = Color32::from_rgba_premultiplied(
                palette.stroke.r(),
                palette.stroke.g(),
                palette.stroke.b(),
                opacity,
            );

            let stroke_width = if child_depth <= 1 { 1.5 } else { 1.0 } * viewport.zoom;

            painter.add(PathShape::line(
                points,
                Stroke::new(stroke_width, edge_color),
            ));
        }
    }
}

fn cubic_bezier_points(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, segments: usize) -> Vec<Pos2> {
    (0..=segments)
        .map(|i| {
            let t = i as f32 / segments as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let mt = 1.0 - t;
            let mt2 = mt * mt;
            let mt3 = mt2 * mt;

            Pos2::new(
                mt3 * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t3 * p3.x,
                mt3 * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t3 * p3.y,
            )
        })
        .collect()
}
