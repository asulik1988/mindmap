const LEVEL_GAP_BASE: f32 = 360.0;

/// Horizontal distance between depth levels, growing with depth when zoomed out.
/// Deeper levels spread wider horizontally → canopy shape fans outward.
/// At high zoom (≥1.5), level gap is constant (no depth scaling).
pub fn level_gap(depth: usize, zoom: f32) -> f32 {
    // Inverse of gap_decay: grows at depth when zoomed out, stays ~1.0 when zoomed in
    let spread = 1.0 / gap_decay(zoom).sqrt();
    LEVEL_GAP_BASE * spread.powi(depth as i32)
}

const SIBLING_GAP_BASE: f32 = 30.0;
const SUBTREE_GAP_BASE: f32 = 44.0;

/// Vertical gap between sibling nodes, decaying with depth and zoom.
/// At low zoom, gaps decay aggressively → compact canopy shape.
/// At high zoom, gaps approach the base value → readable spacing.
pub fn sibling_gap(depth: usize, zoom: f32) -> f32 {
    SIBLING_GAP_BASE * gap_decay(zoom).powi(depth as i32)
}

/// Vertical gap between separate subtree groups (root's children).
pub fn subtree_gap(zoom: f32) -> f32 {
    SUBTREE_GAP_BASE * gap_decay(zoom)
}

/// Scale factor for a node's layout height at a given depth/zoom.
/// Deep nodes are compressed vertically when zoomed out, creating canopy shape.
/// At high zoom, nodes use their full height.
pub fn node_height_scale(depth: usize, zoom: f32) -> f32 {
    gap_decay(zoom).powi(depth as i32).max(MIN_HEIGHT_SCALE)
}

const MIN_HEIGHT_SCALE: f32 = 0.05;

/// Decay factor: lerps from 0.4 (aggressive, zoomed out) to 1.0 (no decay, zoomed in).
fn gap_decay(zoom: f32) -> f32 {
    let t = ((zoom - 0.1) / 1.4).clamp(0.0, 1.0);
    0.4 + 0.6 * t
}
