use super::spacing::{level_gap, node_height_scale, sibling_gap, subtree_gap};
use crate::model::tree::AggregationPlaceholder;
use crate::model::{MindmapTree, NodeId, Side};
use egui::{Pos2, Vec2};

/// Precompute subtree heights for visible nodes in a single O(visible) bottom-up pass.
/// Node heights are scaled by depth/zoom to create canopy-shaped compression.
/// Returns (heights, depths) Vecs indexed by NodeId.
fn compute_all_subtree_heights(tree: &MindmapTree, zoom: f32) -> (Vec<f32>, Vec<usize>) {
    let n = tree.nodes.len();
    let mut heights: Vec<f32> = vec![0.0; n];
    let mut depths: Vec<usize> = vec![0; n];

    // Iterative post-order DFS using a stack of (NodeId, children_processed).
    let mut stack: Vec<(NodeId, bool)> = Vec::with_capacity(1024);
    stack.push((tree.root, false));

    while let Some((id, processed)) = stack.pop() {
        if processed {
            // All children done — compute this node's height from cached children.
            let node = &tree.nodes[id];
            let depth = depths[id];
            let scale = node_height_scale(depth, zoom);
            let node_height = node.layout_size.y * scale;

            let vis_children = tree.visible_children(id);
            if node.folded || vis_children.is_empty() {
                heights[id] = node_height;
            } else {
                let gap = sibling_gap(depth, zoom);
                let children_height: f32 = vis_children.iter().map(|&c| heights[c]).sum::<f32>()
                    + gap * (vis_children.len() as f32 - 1.0).max(0.0);
                heights[id] = children_height.max(node_height);
            }
        } else {
            // First visit — re-push as processed, then push children.
            stack.push((id, true));
            let node = &tree.nodes[id];
            if !node.folded {
                let child_depth = depths[id] + 1;
                for &child_id in tree.visible_children(id).iter().rev() {
                    depths[child_id] = child_depth;
                    stack.push((child_id, false));
                }
            }
        }
    }

    (heights, depths)
}

/// Run the layout algorithm, assigning `layout_pos` to all visible nodes.
/// `zoom` controls gap decay: lower zoom → more aggressive compression at depth.
pub fn layout(tree: &mut MindmapTree, zoom: f32) {
    // Precompute all subtree heights in O(n)
    let (heights, depths) = compute_all_subtree_heights(tree, zoom);

    // Root at origin
    tree.nodes[tree.root].layout_pos = Pos2::ZERO;
    tree.nodes[tree.root].cached_depth = 0;
    tree.nodes[tree.root].cached_side = None;

    // Split root's visible children into left and right groups
    let root_children: Vec<NodeId> = tree.visible_children(tree.root).to_vec();

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

    let st_gap = subtree_gap(zoom);

    // Layout right side (positive X)
    layout_side(
        tree,
        &heights,
        &depths,
        &right_children,
        1.0,
        Side::Right,
        zoom,
        st_gap,
    );

    // Layout left side (negative X)
    layout_side(
        tree,
        &heights,
        &depths,
        &left_children,
        -1.0,
        Side::Left,
        zoom,
        st_gap,
    );

    // Cache the max depth across all visible nodes
    tree.cached_max_depth = depths.iter().copied().max().unwrap_or(0);

    // Compute aggregation placeholders for parents with hidden children
    compute_aggregation_placeholders(tree, &depths, zoom);
}

fn layout_side(
    tree: &mut MindmapTree,
    heights: &[f32],
    depths: &[usize],
    children: &[NodeId],
    x_direction: f32,
    side: Side,
    zoom: f32,
    st_gap: f32,
) {
    if children.is_empty() {
        return;
    }

    // First pass: look up precomputed subtree heights
    let subtree_heights: Vec<f32> = children.iter().map(|&id| heights[id]).collect();

    let total_height: f32 =
        subtree_heights.iter().sum::<f32>() + st_gap * (children.len() as f32 - 1.0).max(0.0);

    // Position children vertically, centered around y=0
    let mut current_y = -total_height / 2.0;

    for (i, &child_id) in children.iter().enumerate() {
        let subtree_h = subtree_heights[i];
        let center_y = current_y + subtree_h / 2.0;

        layout_subtree(
            tree,
            heights,
            depths,
            child_id,
            level_gap(0, zoom) * x_direction,
            center_y,
            1,
            x_direction,
            &side,
            zoom,
        );

        current_y += subtree_h + st_gap;
    }
}

fn layout_subtree(
    tree: &mut MindmapTree,
    heights: &[f32],
    depths: &[usize],
    node_id: NodeId,
    x: f32,
    y: f32,
    depth: usize,
    x_direction: f32,
    side: &Side,
    zoom: f32,
) {
    tree.nodes[node_id].layout_pos = Pos2::new(x, y);
    tree.nodes[node_id].cached_depth = depth;
    tree.nodes[node_id].cached_side = Some(side.clone());

    // If folded, don't layout children
    if tree.nodes[node_id].folded {
        return;
    }

    let children: Vec<NodeId> = tree.visible_children(node_id).to_vec();
    if children.is_empty() {
        return;
    }

    // Look up precomputed subtree heights
    let subtree_heights: Vec<f32> = children.iter().map(|&id| heights[id]).collect();

    let gap = sibling_gap(depth, zoom);
    let total_height: f32 =
        subtree_heights.iter().sum::<f32>() + gap * (children.len() as f32 - 1.0).max(0.0);

    let child_x = x + level_gap(depth, zoom) * x_direction;
    let mut current_y = y - total_height / 2.0;

    for (i, &child_id) in children.iter().enumerate() {
        let subtree_h = subtree_heights[i];
        let center_y = current_y + subtree_h / 2.0;

        layout_subtree(
            tree,
            heights,
            depths,
            child_id,
            child_x,
            center_y,
            depth + 1,
            x_direction,
            side,
            zoom,
        );

        current_y += subtree_h + gap;
    }
}

/// After layout, compute placeholder positions for parents with hidden children.
/// Each placeholder sits just below the last visible child of that parent.
fn compute_aggregation_placeholders(tree: &mut MindmapTree, depths: &[usize], zoom: f32) {
    tree.aggregation_placeholders.clear();

    let visible = tree.visible_nodes();
    // Check each visible node to see if it has hidden children
    for &id in &visible {
        let hidden = tree.hidden_child_count(id);
        if hidden == 0 {
            continue;
        }

        let vis_children = tree.visible_children(id);
        if vis_children.is_empty() {
            continue;
        }

        let last_child_id = *vis_children.last().unwrap();
        let last_child = &tree.nodes[last_child_id];
        let depth = depths.get(last_child_id).copied().unwrap_or(1);
        let gap = sibling_gap(depth.saturating_sub(1), zoom);

        // Position the placeholder just below the last visible child
        let placeholder_h = 24.0; // compact pill height
        let placeholder_w = 80.0; // approximate width for "+N more" text
        let pos = Pos2::new(
            last_child.layout_pos.x,
            last_child.layout_pos.y + last_child.layout_size.y / 2.0 + gap + placeholder_h / 2.0,
        );

        tree.aggregation_placeholders.push(AggregationPlaceholder {
            parent_id: id,
            hidden_count: hidden,
            layout_pos: pos,
            layout_size: Vec2::new(placeholder_w, placeholder_h),
            side: last_child.cached_side.clone(),
            depth,
        });
    }
}
