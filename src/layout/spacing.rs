/// Horizontal distance between depth levels (pixels in canvas coords).
/// Must be > MAX_NODE_WIDTH (280) to prevent horizontal overlap of wide nodes.
pub const LEVEL_GAP: f32 = 360.0;

/// Vertical gap between sibling nodes (edge-to-edge).
pub const SIBLING_GAP: f32 = 30.0;

/// Vertical gap between separate subtree groups.
pub const SUBTREE_GAP: f32 = 44.0;
