use crate::model::{MindmapTree, NodeId, Side};
use crate::style::colors::{self, DepthColorConfig};
use eframe::egui;

const PADDING: f32 = 40.0;

fn color_to_hex(c: egui::Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// Export the visible tree to an SVG string.
pub fn export_svg(tree: &MindmapTree, color_config: &DepthColorConfig) -> String {
    let visible = tree.visible_nodes();
    if visible.is_empty() {
        return "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>".to_string();
    }

    // Compute bounds
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for &id in &visible {
        let node = &tree.nodes[id];
        let hw = node.layout_size.x / 2.0;
        let hh = node.layout_size.y / 2.0;
        min_x = min_x.min(node.layout_pos.x - hw);
        max_x = max_x.max(node.layout_pos.x + hw);
        min_y = min_y.min(node.layout_pos.y - hh);
        max_y = max_y.max(node.layout_pos.y + hh);
    }

    let offset_x = -min_x + PADDING;
    let offset_y = -min_y + PADDING;
    let svg_w = max_x - min_x + PADDING * 2.0;
    let svg_h = max_y - min_y + PADDING * 2.0;

    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {:.1} {:.1}\" width=\"{:.0}\" height=\"{:.0}\">\n",
        svg_w, svg_h, svg_w, svg_h
    ));
    out.push_str("  <style>text { font-family: Virgil, cursive, sans-serif; }</style>\n");

    // Draw edges first (behind nodes)
    let visible_set: std::collections::HashSet<NodeId> = visible.iter().copied().collect();
    for &node_id in &visible {
        let node = &tree.nodes[node_id];
        if let Some(parent_id) = node.parent {
            if !visible_set.contains(&parent_id) {
                continue;
            }
            let parent = &tree.nodes[parent_id];
            let side = tree.effective_side(node_id).unwrap_or(Side::Right);

            let (src_x, src_y, tgt_x, tgt_y) = match side {
                Side::Right => (
                    parent.layout_pos.x + parent.layout_size.x / 2.0,
                    parent.layout_pos.y,
                    node.layout_pos.x - node.layout_size.x / 2.0,
                    node.layout_pos.y,
                ),
                Side::Left => (
                    parent.layout_pos.x - parent.layout_size.x / 2.0,
                    parent.layout_pos.y,
                    node.layout_pos.x + node.layout_size.x / 2.0,
                    node.layout_pos.y,
                ),
            };

            let sx = src_x + offset_x;
            let sy = src_y + offset_y;
            let tx = tgt_x + offset_x;
            let ty = tgt_y + offset_y;

            let dx = tx - sx;
            let cp1x = sx + dx * 0.4;
            let cp1y = sy;
            let cp2x = tx - dx * 0.4;
            let cp2y = ty;

            out.push_str(&format!(
                "  <path d=\"M {:.1} {:.1} C {:.1} {:.1} {:.1} {:.1} {:.1} {:.1}\" stroke=\"#1e1e1e\" stroke-width=\"1.5\" fill=\"none\"/>\n",
                sx, sy, cp1x, cp1y, cp2x, cp2y, tx, ty
            ));
        }
    }

    // Draw nodes (DFS order = visible order)
    let visible_dfs = tree.dfs_order();
    for &node_id in &visible_dfs {
        if !visible_set.contains(&node_id) {
            continue;
        }
        let node = &tree.nodes[node_id];
        let depth = tree.depth(node_id);
        let palette = colors::node_palette(depth, color_config);

        let fill_color = node.background_color.unwrap_or(palette.fill);
        let stroke_color = palette.stroke;

        let nx = node.layout_pos.x - node.layout_size.x / 2.0 + offset_x;
        let ny = node.layout_pos.y - node.layout_size.y / 2.0 + offset_y;
        let nw = node.layout_size.x;
        let nh = node.layout_size.y;

        // White background first
        out.push_str(&format!(
            "  <rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"6\" ry=\"6\" fill=\"white\"/>\n",
            nx, ny, nw, nh
        ));
        // Colored fill with semi-transparency
        out.push_str(&format!(
            "  <rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"6\" ry=\"6\" fill=\"{}\" fill-opacity=\"0.7\" stroke=\"{}\" stroke-width=\"1.5\"/>\n",
            nx, ny, nw, nh,
            color_to_hex(fill_color),
            color_to_hex(stroke_color),
        ));

        // Text lines
        let font_size = colors::font_size_for_depth(depth);
        let text_color = color_to_hex(palette.text);
        let lines: Vec<&str> = node.display_text.split('\n').collect();
        let line_h = font_size * 1.3;
        let total_text_h = lines.len() as f32 * line_h;
        let text_start_y = node.layout_pos.y + offset_y - total_text_h / 2.0 + font_size * 0.8;
        let cx = node.layout_pos.x + offset_x;

        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let ty = text_start_y + i as f32 * line_h;
            let text_content = escape_xml(line);
            let font_weight = if node.bold { "bold" } else { "normal" };
            out.push_str(&format!(
                "  <text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" font-size=\"{:.1}\" font-weight=\"{}\" fill=\"{}\">{}</text>\n",
                cx, ty, font_size, font_weight, text_color, text_content
            ));
        }
    }

    out.push_str("</svg>\n");
    out
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
