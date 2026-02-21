use super::spacing::{LEVEL_GAP, SIBLING_GAP, SUBTREE_GAP};
use crate::model::{MindmapTree, NodeId, Side};
use egui::Pos2;

/// Run the layout algorithm, assigning `layout_pos` to all visible nodes.
pub fn layout(tree: &mut MindmapTree) {
    // Root at origin
    tree.nodes[tree.root].layout_pos = Pos2::ZERO;

    // Split root's children into left and right groups
    let root_children: Vec<NodeId> = tree.nodes[tree.root].children.clone();

    let mut right_children = Vec::new();
    let mut left_children = Vec::new();

    for &child_id in &root_children {
        match tree.nodes[child_id].position {
            Some(Side::Left) => left_children.push(child_id),
            Some(Side::Right) => right_children.push(child_id),
            None => {
                // Auto-assign: balance the sides
                if right_children.len() <= left_children.len() {
                    right_children.push(child_id);
                } else {
                    left_children.push(child_id);
                }
            }
        }
    }

    // Layout right side (positive X)
    layout_side(tree, &right_children, 1.0);

    // Layout left side (negative X)
    layout_side(tree, &left_children, -1.0);
}

fn layout_side(tree: &mut MindmapTree, children: &[NodeId], x_direction: f32) {
    if children.is_empty() {
        return;
    }

    // First pass: compute subtree heights
    let mut subtree_heights: Vec<f32> = Vec::new();
    for &child_id in children {
        let h = compute_subtree_height(tree, child_id);
        subtree_heights.push(h);
    }

    let total_height: f32 =
        subtree_heights.iter().sum::<f32>() + SUBTREE_GAP * (children.len() as f32 - 1.0).max(0.0);

    // Position children vertically, centered around y=0
    let mut current_y = -total_height / 2.0;

    for (i, &child_id) in children.iter().enumerate() {
        let subtree_h = subtree_heights[i];
        let center_y = current_y + subtree_h / 2.0;

        layout_subtree(
            tree,
            child_id,
            LEVEL_GAP * x_direction,
            center_y,
            1,
            x_direction,
        );

        current_y += subtree_h + SUBTREE_GAP;
    }
}

fn layout_subtree(
    tree: &mut MindmapTree,
    node_id: NodeId,
    x: f32,
    y: f32,
    depth: usize,
    x_direction: f32,
) {
    tree.nodes[node_id].layout_pos = Pos2::new(x, y);

    // If folded, don't layout children
    if tree.nodes[node_id].folded {
        return;
    }

    let children: Vec<NodeId> = tree.nodes[node_id].children.clone();
    if children.is_empty() {
        return;
    }

    // Compute subtree heights for each child
    let mut subtree_heights: Vec<f32> = Vec::new();
    for &child_id in &children {
        subtree_heights.push(compute_subtree_height(tree, child_id));
    }

    let total_height: f32 =
        subtree_heights.iter().sum::<f32>() + SIBLING_GAP * (children.len() as f32 - 1.0).max(0.0);

    let child_x = x + LEVEL_GAP * x_direction;
    let mut current_y = y - total_height / 2.0;

    for (i, &child_id) in children.iter().enumerate() {
        let subtree_h = subtree_heights[i];
        let center_y = current_y + subtree_h / 2.0;

        layout_subtree(tree, child_id, child_x, center_y, depth + 1, x_direction);

        current_y += subtree_h + SIBLING_GAP;
    }
}

/// Compute the total height a subtree occupies (for spacing).
fn compute_subtree_height(tree: &MindmapTree, node_id: NodeId) -> f32 {
    let node = &tree.nodes[node_id];
    let node_height = node.layout_size.y; // use measured size

    if node.folded || node.children.is_empty() {
        return node_height;
    }

    let children_height: f32 = node
        .children
        .iter()
        .map(|&c| compute_subtree_height(tree, c))
        .sum::<f32>()
        + SIBLING_GAP * (node.children.len() as f32 - 1.0).max(0.0);

    children_height.max(node_height)
}
