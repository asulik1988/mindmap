use super::viewport::Viewport;
use crate::model::{MindmapTree, NodeId, Side};
use crate::style::colors;
use crate::style::wobble::{self, RoughOptions};
use egui::{epaint::PathShape, Color32, Painter, Pos2, Rect, Stroke};
use std::collections::HashSet;

/// Zoom threshold below which we draw smooth bezier edges instead of rough ones.
const LOD_DETAIL_ZOOM: f32 = 0.3;

/// Minimum screen-space edge length (pixels) to bother rendering.
const MIN_EDGE_SCREEN_PX: f32 = 1.0;

/// Draw bezier edges from parent to each visible child.
pub fn draw_edges(
    painter: &Painter,
    tree: &MindmapTree,
    visible_nodes: &[usize],
    visible_set: &HashSet<NodeId>,
    viewport: &Viewport,
    screen_rect: Rect,
    dark_mode: bool,
) {
    let base_edge_color = colors::edge_color(dark_mode);
    let margin = 100.0;
    let cull_rect = screen_rect.expand(margin);

    for &node_id in visible_nodes {
        let node = &tree.nodes[node_id];
        if let Some(parent_id) = node.parent {
            // Only draw if parent is also visible
            if !visible_set.contains(&parent_id) {
                continue;
            }

            let parent = &tree.nodes[parent_id];

            // Cheap screen-space culling using node centers BEFORE any expensive work
            let src_approx = viewport.canvas_to_screen(parent.layout_pos, screen_rect);
            let tgt_approx = viewport.canvas_to_screen(node.layout_pos, screen_rect);

            // Viewport culling
            let edge_bounds = Rect::from_two_pos(src_approx, tgt_approx);
            if !edge_bounds.intersects(cull_rect) {
                continue;
            }

            // LOD culling: skip sub-pixel edges
            let dx = tgt_approx.x - src_approx.x;
            let dy = tgt_approx.y - src_approx.y;
            let edge_len_sq = dx * dx + dy * dy;
            if edge_len_sq < MIN_EDGE_SCREEN_PX * MIN_EDGE_SCREEN_PX {
                continue;
            }

            let child_depth = node.cached_depth;
            let parent_size = parent.layout_size;
            let child_size = node.layout_size;

            // Use cached side (O(1)) instead of walking up the tree
            let side = node.cached_side.as_ref().unwrap_or(&Side::Right);

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
            let bx = tgt_screen.x - src_screen.x;
            let cp1 = Pos2::new(src_screen.x + bx * 0.4, src_screen.y);
            let cp2 = Pos2::new(tgt_screen.x - bx * 0.4, tgt_screen.y);

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

            if viewport.zoom < LOD_DETAIL_ZOOM {
                // Simple smooth bezier (no rough double-stroke)
                let points: Vec<Pos2> = (0..=8)
                    .map(|i| {
                        let t = i as f32 / 8.0;
                        let u = 1.0 - t;
                        Pos2::new(
                            u * u * u * src_screen.x
                                + 3.0 * u * u * t * cp1.x
                                + 3.0 * u * t * t * cp2.x
                                + t * t * t * tgt_screen.x,
                            u * u * u * src_screen.y
                                + 3.0 * u * u * t * cp1.y
                                + 3.0 * u * t * t * cp2.y
                                + t * t * t * tgt_screen.y,
                        )
                    })
                    .collect();
                painter.add(PathShape::line(points, stroke));
            } else {
                // Generate roughjs-style hand-drawn edge with double-stroke
                let edge_seed = (parent_id as u32).wrapping_mul(2654435761)
                    ^ (node_id as u32).wrapping_mul(2246822519);
                let rough_opts = RoughOptions {
                    roughness: 0.5,
                    max_randomness_offset: 1.0,
                    bowing: 0.3,
                    ..Default::default()
                };
                let paths = wobble::rough_bezier_edge(
                    src_screen, cp1, cp2, tgt_screen, edge_seed, &rough_opts,
                );
                for path in paths {
                    if path.len() >= 2 {
                        painter.add(PathShape::line(path, stroke));
                    }
                }
            }
        }
    }
}
