use crate::model::{MindmapNode, NodeState, Selection};
use crate::style::colors;
use super::viewport::Viewport;
use egui::{
    epaint::RectShape, Color32, FontId, Painter, Pos2, Rect, Rounding, Stroke, Vec2,
};

const NODE_PADDING_H: f32 = 16.0;
const NODE_PADDING_V: f32 = 8.0;
const NODE_ROUNDING: f32 = 8.0;
const MIN_NODE_WIDTH: f32 = 80.0;
const MAX_NODE_WIDTH: f32 = 280.0;
const SELECTION_COLOR: Color32 = Color32::from_rgb(30, 136, 229); // #1E88E5

/// Measure a node's size in canvas coordinates.
pub fn measure_node(node: &MindmapNode, depth: usize, painter: &Painter) -> Vec2 {
    let font_size = colors::font_size_for_depth(depth);
    let font_id = FontId::proportional(font_size);

    let galley = painter.layout_no_wrap(node.text.clone(), font_id, Color32::BLACK);
    let text_width = galley.size().x.clamp(MIN_NODE_WIDTH - NODE_PADDING_H * 2.0, MAX_NODE_WIDTH - NODE_PADDING_H * 2.0);
    let text_height = galley.size().y;

    Vec2::new(
        text_width + NODE_PADDING_H * 2.0,
        text_height + NODE_PADDING_V * 2.0,
    )
}

/// Draw a single node. Returns the screen-space rect for hit testing.
pub fn draw_node(
    painter: &Painter,
    node: &MindmapNode,
    depth: usize,
    viewport: &Viewport,
    screen_rect: Rect,
    selection: &Selection,
) -> Rect {
    let screen_pos = viewport.canvas_to_screen(node.layout_pos, screen_rect);
    let size = measure_node(node, depth, painter);
    let scaled_size = size * viewport.zoom;

    // Node rect centered on layout_pos
    let node_rect = Rect::from_center_size(screen_pos, scaled_size);

    let palette = colors::node_palette(depth);
    let is_selected = selection.is_selected(node.id);
    let is_hovered = selection.hovered == Some(node.id);

    // Determine colors based on state
    let (fill, stroke_color, stroke_width) = match node.state {
        NodeState::Editing => (
            Color32::WHITE,
            SELECTION_COLOR,
            palette.stroke_width + 1.0,
        ),
        _ if is_selected => (
            palette.fill,
            SELECTION_COLOR,
            palette.stroke_width + 0.5,
        ),
        _ if is_hovered => (
            lighten(palette.fill, 0.05),
            palette.stroke,
            palette.stroke_width + 0.5,
        ),
        _ => (palette.fill, palette.stroke, palette.stroke_width),
    };

    // Shadow for depth 0-2
    if depth <= 2 {
        let shadow_offset = Vec2::new(2.0 * viewport.zoom, 3.0 * viewport.zoom);
        let shadow_rect = node_rect.translate(shadow_offset);
        painter.add(RectShape::filled(
            shadow_rect,
            Rounding::same(NODE_ROUNDING * viewport.zoom),
            Color32::from_rgba_premultiplied(0, 0, 0, 15),
        ));
    }

    // Node background
    painter.add(RectShape::new(
        node_rect,
        Rounding::same(NODE_ROUNDING * viewport.zoom),
        fill,
        Stroke::new(stroke_width * viewport.zoom, stroke_color),
    ));

    // Selection ring
    if is_selected {
        let ring_rect = node_rect.expand(3.0 * viewport.zoom);
        painter.add(RectShape::new(
            ring_rect,
            Rounding::same((NODE_ROUNDING + 3.0) * viewport.zoom),
            Color32::from_rgba_premultiplied(30, 136, 229, 15),
            Stroke::new(1.5 * viewport.zoom, Color32::from_rgba_premultiplied(30, 136, 229, 200)),
        ));
    }

    // Text
    let font_size = colors::font_size_for_depth(depth) * viewport.zoom;
    let font_id = FontId::proportional(font_size);
    let text_color = palette.text;

    let galley = painter.layout_no_wrap(node.text.clone(), font_id, text_color);
    let text_pos = Pos2::new(
        node_rect.center().x - galley.size().x / 2.0,
        node_rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_pos, galley, text_color);

    // Fold indicator
    if node.folded && !node.children.is_empty() {
        let desc_count = node.descendant_count(&[]); // We'll fix this later
        let badge_text = format!("+{}", node.children.len());
        let badge_font = FontId::proportional(10.0 * viewport.zoom);
        let badge_galley = painter.layout_no_wrap(badge_text, badge_font, Color32::WHITE);

        let badge_pos = Pos2::new(
            node_rect.max.x - badge_galley.size().x - 4.0 * viewport.zoom,
            node_rect.max.y - badge_galley.size().y - 2.0 * viewport.zoom,
        );

        let badge_rect = Rect::from_min_size(
            badge_pos - Vec2::new(3.0, 1.0) * viewport.zoom,
            badge_galley.size() + Vec2::new(6.0, 2.0) * viewport.zoom,
        );
        painter.add(RectShape::filled(
            badge_rect,
            Rounding::same(6.0 * viewport.zoom),
            Color32::from_rgb(117, 117, 117),
        ));
        painter.galley(badge_pos, badge_galley, Color32::WHITE);
    }

    node_rect
}

fn lighten(color: Color32, amount: f32) -> Color32 {
    let r = (color.r() as f32 + 255.0 * amount).min(255.0) as u8;
    let g = (color.g() as f32 + 255.0 * amount).min(255.0) as u8;
    let b = (color.b() as f32 + 255.0 * amount).min(255.0) as u8;
    Color32::from_rgb(r, g, b)
}
