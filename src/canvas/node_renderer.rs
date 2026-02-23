use super::viewport::Viewport;
use crate::model::{MindmapNode, NodeState, Selection};
use crate::style::colors::{self, DepthColorConfig};
use crate::style::wobble::{self, RoughOptions};
use egui::{
    epaint::{PathShape, RectShape, StrokeKind},
    Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, Vec2,
};

const NODE_PADDING_H: f32 = 20.0;
const NODE_PADDING_V: f32 = 10.0;
const NODE_ROUNDING: f32 = 12.0;
const MIN_NODE_WIDTH: f32 = 80.0;
const MAX_NODE_WIDTH: f32 = 280.0;
const ROUGHNESS: f32 = 0.5;
const SELECTION_COLOR: Color32 = Color32::from_rgb(30, 136, 229); // #1E88E5
/// Zoom threshold below which we skip wobble/hachure and render simple shapes.
const LOD_DETAIL_ZOOM: f32 = 0.3;
/// Minimum screen-space font size to render text (below this, text is unreadable).
const MIN_TEXT_SCREEN_SIZE: f32 = 4.0;

fn cr(r: f32) -> CornerRadius {
    CornerRadius::same(r.round().clamp(0.0, 255.0) as u8)
}

fn with_alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        color.r(),
        color.g(),
        color.b(),
        (color.a() as f32 * alpha).round().clamp(0.0, 255.0) as u8,
    )
}

fn rotate_point(p: Pos2, center: Pos2, angle_rad: f32) -> Pos2 {
    let dx = p.x - center.x;
    let dy = p.y - center.y;
    Pos2::new(
        center.x + dx * angle_rad.cos() - dy * angle_rad.sin(),
        center.y + dx * angle_rad.sin() + dy * angle_rad.cos(),
    )
}

/// Pre-measure all nodes and store sizes + pre-wrapped display text for layout.
/// Only measures visible (not folded-away) nodes for performance on large trees.
/// Skips nodes that are already measured (measured == true) for fast unfold.
///
/// Uses a time budget to avoid blocking the UI thread on large trees.
/// Returns `true` if there are still unmeasured visible nodes remaining.
pub fn measure_all_nodes(tree: &mut crate::model::MindmapTree, painter: &Painter) -> bool {
    let visible = tree.visible_nodes();
    let start = std::time::Instant::now();
    // Budget: 32ms keeps the app responsive at ~30fps while measuring
    let budget = std::time::Duration::from_millis(32);
    let mut count = 0u32;

    for &node_id in &visible {
        if tree.nodes[node_id].measured {
            continue; // already measured, skip
        }
        if tree.nodes[node_id].text.is_empty() {
            continue; // skip deleted nodes
        }

        // Check time budget every 500 nodes (Instant::now is not free)
        count += 1;
        if count % 500 == 0 && start.elapsed() > budget {
            let remaining: usize = visible
                .iter()
                .filter(|&&id| !tree.nodes[id].measured && !tree.nodes[id].text.is_empty())
                .count();
            log::info!(
                "measure_all_nodes: budget exhausted after {} nodes, ~{} remaining",
                count,
                remaining
            );
            return true; // more work remaining
        }

        let depth = tree.nodes[node_id].cached_depth;
        let font_size = colors::font_size_for_depth(depth);
        let font_id = FontId::proportional(font_size);
        let max_text_width = MAX_NODE_WIDTH - NODE_PADDING_H * 2.0;

        let galley = painter.layout(
            tree.nodes[node_id].text.clone(),
            font_id,
            Color32::BLACK,
            max_text_width,
        );

        let text_width = galley.size().x.max(MIN_NODE_WIDTH - NODE_PADDING_H * 2.0);
        let text_height = galley.size().y;

        tree.nodes[node_id].layout_size = Vec2::new(
            text_width + NODE_PADDING_H * 2.0,
            text_height + NODE_PADDING_V * 2.0,
        );

        // Extract line breaks from the galley to build pre-wrapped display text.
        // This ensures text wraps identically at all zoom levels.
        let mut display_text = String::new();
        for (i, row) in galley.rows.iter().enumerate() {
            if i > 0 {
                display_text.push('\n');
            }
            for glyph in &row.glyphs {
                if glyph.chr != '\n' {
                    display_text.push(glyph.chr);
                }
            }
        }
        tree.nodes[node_id].display_text = display_text;
        tree.nodes[node_id].measured = true;
    }
    false // all done
}

/// Draw a single node. Returns the screen-space rect for hit testing.
/// `alpha` controls overall opacity (1.0 = normal, 0.3 = dimmed for drag source).
#[allow(clippy::too_many_arguments)]
pub fn draw_node(
    painter: &Painter,
    node: &MindmapNode,
    depth: usize,
    viewport: &Viewport,
    screen_rect: Rect,
    selection: &Selection,
    alpha: f32,
    color_config: &DepthColorConfig,
    is_search_match: bool,
    is_current_search_match: bool,
    dark_mode: bool,
) -> Rect {
    let screen_pos = viewport.canvas_to_screen(node.layout_pos, screen_rect);
    let palette = colors::node_palette_themed(depth, dark_mode, color_config);

    // Use layout-determined size (scaled by zoom) so box is stable across zoom levels
    let node_w = node.layout_size.x * viewport.zoom;
    let node_h = node.layout_size.y * viewport.zoom;
    let node_rect = Rect::from_center_size(screen_pos, Vec2::new(node_w, node_h));

    let is_selected = selection.is_selected(node.id);
    let is_hovered = selection.hovered == Some(node.id);

    // LOD: simplified rendering when zoomed out (no wobble/hachure — pure geometry)
    if viewport.zoom < LOD_DETAIL_ZOOM {
        let (fill, stroke_color, stroke_width) = match node.state {
            NodeState::Editing => (Color32::WHITE, SELECTION_COLOR, palette.stroke_width + 1.0),
            _ if is_selected => (palette.fill, SELECTION_COLOR, palette.stroke_width + 0.5),
            _ if is_hovered => (
                lighten(palette.fill, 0.05),
                palette.stroke,
                palette.stroke_width + 0.5,
            ),
            _ => (palette.fill, palette.stroke, palette.stroke_width),
        };
        let rounding = NODE_ROUNDING * viewport.zoom;
        let sw = stroke_width * viewport.zoom;

        // Single filled rect with border
        painter.add(RectShape::new(
            node_rect,
            cr(rounding),
            with_alpha(fill, alpha),
            Stroke::new(sw, with_alpha(stroke_color, alpha)),
            StrokeKind::Outside,
        ));

        // Text only if screen-size font is readable
        let font_size = colors::font_size_for_depth(depth) * viewport.zoom;
        if font_size >= MIN_TEXT_SCREEN_SIZE {
            let font_id = FontId::proportional(font_size);
            let pad_h = NODE_PADDING_H * viewport.zoom;
            let pad_v = NODE_PADDING_V * viewport.zoom;
            let lod_text = if node.display_text.is_empty() && !node.text.is_empty() {
                &node.text
            } else {
                &node.display_text
            };
            let text_galley = painter.layout(
                lod_text.clone(),
                font_id,
                with_alpha(palette.text, alpha),
                f32::INFINITY,
            );
            let text_pos = Pos2::new(node_rect.min.x + pad_h, node_rect.min.y + pad_v);
            if node.bold {
                painter.galley(
                    text_pos + egui::vec2(0.7, 0.0),
                    text_galley.clone(),
                    with_alpha(palette.text, alpha),
                );
            }
            painter.galley(text_pos, text_galley, with_alpha(palette.text, alpha));
        }

        // Selection ring (simple)
        if is_selected && alpha > 0.9 {
            let ring_rect = node_rect.expand(3.0 * viewport.zoom);
            let ring_rounding = (NODE_ROUNDING + 3.0) * viewport.zoom;
            painter.add(RectShape::new(
                ring_rect,
                cr(ring_rounding),
                Color32::from_rgba_premultiplied(30, 136, 229, 15),
                Stroke::new(
                    1.5 * viewport.zoom,
                    Color32::from_rgba_premultiplied(30, 136, 229, 200),
                ),
                StrokeKind::Outside,
            ));
        }

        // Search highlight ring (simple)
        if (is_search_match || is_current_search_match) && alpha > 0.9 {
            let ring_rect = node_rect.expand(5.0 * viewport.zoom);
            let ring_rounding = (NODE_ROUNDING + 5.0) * viewport.zoom;
            let (ring_color, ring_sw) = if is_current_search_match {
                (Color32::from_rgb(245, 166, 35), 2.0 * viewport.zoom)
            } else {
                (
                    Color32::from_rgba_premultiplied(245, 166, 35, 160),
                    1.5 * viewport.zoom,
                )
            };
            painter.add(RectShape::new(
                ring_rect,
                cr(ring_rounding),
                Color32::TRANSPARENT,
                Stroke::new(ring_sw, ring_color),
                StrokeKind::Outside,
            ));
        }

        return node_rect;
    }

    // Render pre-wrapped display text at screen-scale font size.
    // Line breaks were determined once at canvas scale, so text is stable across zoom levels.
    // Fall back to raw text for unmeasured nodes (display_text empty before first measurement).
    let font_size = colors::font_size_for_depth(depth) * viewport.zoom;
    let font_id = FontId::proportional(font_size);
    let pad_h = NODE_PADDING_H * viewport.zoom;
    let pad_v = NODE_PADDING_V * viewport.zoom;
    let render_text = if node.display_text.is_empty() && !node.text.is_empty() {
        &node.text
    } else {
        &node.display_text
    };
    let text_galley = painter.layout(
        render_text.clone(),
        font_id,
        with_alpha(palette.text, alpha),
        f32::INFINITY, // no re-wrapping — line breaks are already in display_text
    );

    // Determine colors based on state
    let (fill, stroke_color, stroke_width) = match node.state {
        NodeState::Editing => (Color32::WHITE, SELECTION_COLOR, palette.stroke_width + 1.0),
        _ if is_selected => (palette.fill, SELECTION_COLOR, palette.stroke_width + 0.5),
        _ if is_hovered => (
            lighten(palette.fill, 0.05),
            palette.stroke,
            palette.stroke_width + 0.5,
        ),
        _ => (palette.fill, palette.stroke, palette.stroke_width),
    };

    let seed = (node.id as u32).wrapping_mul(2654435761);
    let rounding = NODE_ROUNDING * viewport.zoom;
    let sw = stroke_width * viewport.zoom;

    // 1. Background fill — color comes from hatch lines only (like Excalidraw)
    let node_bg = if dark_mode {
        Color32::from_rgb(22, 22, 26)
    } else {
        Color32::WHITE
    };
    painter.add(RectShape::new(
        node_rect,
        cr(rounding),
        with_alpha(node_bg, alpha),
        Stroke::NONE,
        StrokeKind::Outside,
    ));

    // 1.5. Hachure fill pattern — single direction, like Excalidraw
    let hatch_color = with_alpha(fill, alpha);
    let hatch_gap = 5.0_f32;
    let hatch_opts = RoughOptions {
        roughness: 0.8,
        max_randomness_offset: 1.5,
        bowing: 1.0,
        disable_multi_stroke: true,
        ..Default::default()
    };
    let hatch_paths = wobble::hachure_fill_rect(
        node_rect,
        -41.0,
        hatch_gap,
        seed.wrapping_add(5555),
        &hatch_opts,
    );
    let hatch_stroke = Stroke::new(2.0, hatch_color);
    for path in hatch_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, hatch_stroke));
        }
    }

    // 2. Hand-drawn strokes (roughjs bezier curves with double-stroke)
    let rough_opts = RoughOptions {
        roughness: ROUGHNESS,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let paths = wobble::rough_rounded_rect(node_rect, rounding, seed, &rough_opts);
    let stroke = Stroke::new(sw, with_alpha(stroke_color, alpha));
    for path in paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(path, stroke));
        }
    }

    // Selection ring
    if is_selected && alpha > 0.9 {
        let ring_rect = node_rect.expand(3.0 * viewport.zoom);
        let ring_rounding = (NODE_ROUNDING + 3.0) * viewport.zoom;
        let sel_color = Color32::from_rgba_premultiplied(30, 136, 229, 200);

        // Clean fill for selection tint
        painter.add(RectShape::new(
            ring_rect,
            cr(ring_rounding),
            Color32::from_rgba_premultiplied(30, 136, 229, 15),
            Stroke::NONE,
            StrokeKind::Outside,
        ));

        // Wobbled selection ring strokes
        let ring_opts = RoughOptions {
            roughness: ROUGHNESS,
            max_randomness_offset: 1.0,
            bowing: 0.5,
            ..Default::default()
        };
        let ring_paths = wobble::rough_rounded_rect(
            ring_rect,
            ring_rounding,
            seed.wrapping_add(999),
            &ring_opts,
        );
        let ring_stroke = Stroke::new(1.5 * viewport.zoom, sel_color);
        for path in ring_paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, ring_stroke));
            }
        }
    }

    // Search highlight ring (drawn outside selection ring)
    if (is_search_match || is_current_search_match) && alpha > 0.9 {
        let ring_rect = node_rect.expand(5.0 * viewport.zoom);
        let ring_rounding = (NODE_ROUNDING + 5.0) * viewport.zoom;

        let (ring_color, ring_sw, fill_tint) = if is_current_search_match {
            (
                Color32::from_rgb(245, 166, 35),
                2.0 * viewport.zoom,
                Color32::from_rgba_premultiplied(245, 166, 35, 18),
            )
        } else {
            (
                Color32::from_rgba_premultiplied(245, 166, 35, 160),
                1.5 * viewport.zoom,
                Color32::TRANSPARENT,
            )
        };

        // Fill tint for current match
        if fill_tint != Color32::TRANSPARENT {
            painter.add(RectShape::new(
                ring_rect,
                cr(ring_rounding),
                fill_tint,
                Stroke::NONE,
                StrokeKind::Outside,
            ));
        }

        // Wobbled search ring
        let ring_opts = RoughOptions {
            roughness: ROUGHNESS,
            max_randomness_offset: 1.0,
            bowing: 0.5,
            ..Default::default()
        };
        let ring_paths = wobble::rough_rounded_rect(
            ring_rect,
            ring_rounding,
            seed.wrapping_add(8888),
            &ring_opts,
        );
        let ring_stroke = Stroke::new(ring_sw, ring_color);
        for path in ring_paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, ring_stroke));
            }
        }
    }

    // Text — pin to top-left with padding for stable position across zoom levels
    let text_pos = Pos2::new(node_rect.min.x + pad_h, node_rect.min.y + pad_v);
    // Bold: draw twice with a small horizontal offset for faux-weight effect
    if node.bold {
        painter.galley(
            text_pos + egui::vec2(0.7, 0.0),
            text_galley.clone(),
            with_alpha(palette.text, alpha),
        );
    }
    painter.galley(text_pos, text_galley, with_alpha(palette.text, alpha));

    // Fold indicator — pill badge just outside the right edge, vertically centred
    if node.folded && !node.children.is_empty() {
        let badge_text = format!("({})", node.children.len());
        let badge_font = FontId::proportional(11.0 * viewport.zoom);
        let badge_text_color = with_alpha(palette.text, alpha);
        let badge_galley = painter.layout_no_wrap(badge_text, badge_font, badge_text_color);

        let pad_x = 5.0 * viewport.zoom;
        let pad_y = 2.0 * viewport.zoom;
        let badge_w = badge_galley.size().x + pad_x * 2.0;
        let badge_h = badge_galley.size().y + pad_y * 2.0;
        let badge_rect = Rect::from_min_size(
            Pos2::new(
                node_rect.max.x + 5.0 * viewport.zoom,
                node_rect.center().y - badge_h / 2.0,
            ),
            Vec2::new(badge_w, badge_h),
        );
        // Pill background in the node's palette fill color
        painter.add(RectShape::filled(
            badge_rect,
            cr(badge_h / 2.0),
            with_alpha(palette.fill, alpha),
        ));
        // Wobbled border to match the Excalidraw aesthetic
        let badge_opts = RoughOptions {
            roughness: 0.4,
            max_randomness_offset: 0.7,
            bowing: 0.3,
            disable_multi_stroke: true,
            ..Default::default()
        };
        let badge_paths = wobble::rough_rounded_rect(
            badge_rect,
            badge_h / 2.0,
            seed.wrapping_add(7777),
            &badge_opts,
        );
        let badge_border = Stroke::new(1.0 * viewport.zoom, with_alpha(palette.stroke, alpha));
        for path in badge_paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, badge_border));
            }
        }
        let text_pos = Pos2::new(
            badge_rect.min.x + pad_x,
            badge_rect.center().y - badge_galley.size().y / 2.0,
        );
        painter.galley(text_pos, badge_galley, badge_text_color);
    }

    // Notes dot — small filled circle inside bottom-right of node
    if !node.notes.is_empty() {
        let dot_r = 3.0 * viewport.zoom;
        let dot_center = Pos2::new(
            node_rect.max.x - 6.0 * viewport.zoom,
            node_rect.max.y - 6.0 * viewport.zoom,
        );
        let dot_color = with_alpha(
            Color32::from_rgba_premultiplied(
                palette.text.r(),
                palette.text.g(),
                palette.text.b(),
                100,
            ),
            alpha,
        );
        painter.circle_filled(dot_center, dot_r, dot_color);
    }

    // Link dot — small blue filled circle, just left of the notes dot
    if node.link.is_some() {
        let link_r = 3.0 * viewport.zoom;
        let link_center = Pos2::new(
            node_rect.max.x - 14.0 * viewport.zoom,
            node_rect.max.y - 6.0 * viewport.zoom,
        );
        painter.circle_filled(
            link_center,
            link_r,
            with_alpha(Color32::from_rgb(30, 136, 229), alpha),
        );
    }

    node_rect
}

/// Draw a ghost node at an arbitrary screen position with rotation and transparency.
/// Used for drag-and-drop visual feedback.
#[allow(clippy::too_many_arguments)]
pub fn draw_node_ghost(
    painter: &Painter,
    node: &MindmapNode,
    depth: usize,
    viewport: &Viewport,
    ghost_center: Pos2,
    alpha: f32,
    rotation_deg: f32,
    color_config: &DepthColorConfig,
    dark_mode: bool,
) {
    let palette = colors::node_palette_themed(depth, dark_mode, color_config);
    let angle_rad = rotation_deg.to_radians();

    let node_w = node.layout_size.x * viewport.zoom;
    let node_h = node.layout_size.y * viewport.zoom;
    let node_rect = Rect::from_center_size(ghost_center, Vec2::new(node_w, node_h));

    let font_size = colors::font_size_for_depth(depth) * viewport.zoom;
    let font_id = FontId::proportional(font_size);
    let pad_h = NODE_PADDING_H * viewport.zoom;
    let pad_v = NODE_PADDING_V * viewport.zoom;

    let fill = palette.fill;
    let stroke_color = palette.stroke;
    let stroke_width = palette.stroke_width;

    let seed = (node.id as u32).wrapping_mul(2654435761);
    let rounding = NODE_ROUNDING * viewport.zoom;
    let sw = stroke_width * viewport.zoom;

    // Helper to rotate a path
    let rotate_path = |path: Vec<Pos2>| -> Vec<Pos2> {
        path.into_iter()
            .map(|p| rotate_point(p, ghost_center, angle_rad))
            .collect()
    };

    // 1. Background fill (rotated)
    // For the background, we draw a rotated polygon instead of a rect
    let ghost_bg = if dark_mode {
        Color32::from_rgb(22, 22, 26)
    } else {
        Color32::WHITE
    };
    let corners = [
        node_rect.left_top(),
        node_rect.right_top(),
        node_rect.right_bottom(),
        node_rect.left_bottom(),
    ];
    let rotated_corners: Vec<Pos2> = corners
        .iter()
        .map(|&p| rotate_point(p, ghost_center, angle_rad))
        .collect();
    painter.add(egui::Shape::convex_polygon(
        rotated_corners,
        with_alpha(ghost_bg, alpha),
        Stroke::NONE,
    ));

    // 1.5. Hachure fill pattern
    let hatch_opts = RoughOptions {
        roughness: 0.8,
        max_randomness_offset: 1.5,
        bowing: 1.0,
        disable_multi_stroke: true,
        ..Default::default()
    };
    let hatch_paths =
        wobble::hachure_fill_rect(node_rect, -41.0, 5.0, seed.wrapping_add(5555), &hatch_opts);
    let hatch_stroke = Stroke::new(2.0, with_alpha(fill, alpha));
    for path in hatch_paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(rotate_path(path), hatch_stroke));
        }
    }

    // 2. Hand-drawn border strokes
    let rough_opts = RoughOptions {
        roughness: ROUGHNESS,
        max_randomness_offset: 1.0,
        bowing: 0.5,
        ..Default::default()
    };
    let paths = wobble::rough_rounded_rect(node_rect, rounding, seed, &rough_opts);
    let stroke = Stroke::new(sw, with_alpha(stroke_color, alpha));
    for path in paths {
        if path.len() >= 2 {
            painter.add(PathShape::line(rotate_path(path), stroke));
        }
    }

    // 3. Text (rotated)
    let text_pos = Pos2::new(node_rect.min.x + pad_h, node_rect.min.y + pad_v);
    let rotated_text_pos = rotate_point(text_pos, ghost_center, angle_rad);
    let text_galley = painter.layout(
        node.display_text.clone(),
        font_id,
        with_alpha(palette.text, alpha),
        f32::INFINITY,
    );
    painter.galley(
        rotated_text_pos,
        text_galley,
        with_alpha(palette.text, alpha),
    );

    // 4. Fold indicator (if applicable) — same pill style, rotated with the ghost
    if node.folded && !node.children.is_empty() {
        let badge_text = format!("({})", node.children.len());
        let badge_font = FontId::proportional(11.0 * viewport.zoom);
        let badge_text_color = with_alpha(palette.text, alpha);
        let badge_galley = painter.layout_no_wrap(badge_text, badge_font, badge_text_color);

        let pad_x = 5.0 * viewport.zoom;
        let pad_y = 2.0 * viewport.zoom;
        let badge_w = badge_galley.size().x + pad_x * 2.0;
        let badge_h = badge_galley.size().y + pad_y * 2.0;
        // Position outside right edge at vertical center (unrotated), then rotate
        let badge_rect = Rect::from_min_size(
            Pos2::new(
                node_rect.max.x + 5.0 * viewport.zoom,
                node_rect.center().y - badge_h / 2.0,
            ),
            Vec2::new(badge_w, badge_h),
        );
        let badge_corners: Vec<Pos2> = [
            badge_rect.left_top(),
            badge_rect.right_top(),
            badge_rect.right_bottom(),
            badge_rect.left_bottom(),
        ]
        .iter()
        .map(|&p| rotate_point(p, ghost_center, angle_rad))
        .collect();
        painter.add(egui::Shape::convex_polygon(
            badge_corners,
            with_alpha(palette.fill, alpha),
            Stroke::NONE,
        ));
        let badge_text_pos = Pos2::new(
            badge_rect.min.x + pad_x,
            badge_rect.center().y - badge_galley.size().y / 2.0,
        );
        let rotated_text_pos = rotate_point(badge_text_pos, ghost_center, angle_rad);
        painter.galley(rotated_text_pos, badge_galley, badge_text_color);
    }
}

/// Draw a compact "+N more" pill for sibling aggregation.
/// Returns the screen-space rect for hit testing.
pub fn draw_aggregation_placeholder(
    painter: &Painter,
    placeholder: &crate::model::tree::AggregationPlaceholder,
    viewport: &Viewport,
    screen_rect: Rect,
    color_config: &DepthColorConfig,
    dark_mode: bool,
) -> Rect {
    let screen_pos = viewport.canvas_to_screen(placeholder.layout_pos, screen_rect);
    let pill_w = placeholder.layout_size.x * viewport.zoom;
    let pill_h = placeholder.layout_size.y * viewport.zoom;
    let pill_rect = Rect::from_center_size(screen_pos, Vec2::new(pill_w, pill_h));

    let depth = placeholder.depth;
    let palette = colors::node_palette_themed(depth, dark_mode, color_config);

    let text = format!("+{} more", placeholder.hidden_count);
    let font_size = 11.0 * viewport.zoom;
    let font_id = FontId::proportional(font_size);
    let text_color = palette.stroke;
    let text_galley = painter.layout_no_wrap(text, font_id, text_color);

    // Pill background — lighter than normal nodes
    let bg = if dark_mode {
        Color32::from_rgb(35, 35, 40)
    } else {
        Color32::from_rgb(245, 245, 245)
    };
    let rounding = pill_h / 2.0;

    // Use LOD threshold for detail level
    if viewport.zoom < LOD_DETAIL_ZOOM {
        // Simple pill
        painter.add(RectShape::new(
            pill_rect,
            cr(rounding),
            bg,
            Stroke::new(1.0 * viewport.zoom, palette.stroke),
            StrokeKind::Outside,
        ));
    } else {
        // Filled background
        painter.add(RectShape::filled(pill_rect, cr(rounding), bg));
        // Wobbled border
        let seed = (placeholder.parent_id as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(9999);
        let badge_opts = RoughOptions {
            roughness: 0.4,
            max_randomness_offset: 0.7,
            bowing: 0.3,
            disable_multi_stroke: true,
            ..Default::default()
        };
        let badge_paths =
            wobble::rough_rounded_rect(pill_rect, rounding, seed, &badge_opts);
        let border = Stroke::new(1.0 * viewport.zoom, palette.stroke);
        for path in badge_paths {
            if path.len() >= 2 {
                painter.add(PathShape::line(path, border));
            }
        }
    }

    // Center text in pill
    let text_pos = Pos2::new(
        pill_rect.center().x - text_galley.size().x / 2.0,
        pill_rect.center().y - text_galley.size().y / 2.0,
    );
    painter.galley(text_pos, text_galley, text_color);

    pill_rect
}

fn lighten(color: Color32, amount: f32) -> Color32 {
    let r = (color.r() as f32 + 255.0 * amount).min(255.0) as u8;
    let g = (color.g() as f32 + 255.0 * amount).min(255.0) as u8;
    let b = (color.b() as f32 + 255.0 * amount).min(255.0) as u8;
    Color32::from_rgb(r, g, b)
}
