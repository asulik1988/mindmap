use super::clipboard::SubtreeBlueprint;
use super::node::{MindmapNode, NodeId, Side};
use egui::{Pos2, Vec2};
use std::collections::{HashMap, HashSet};

/// Ephemeral rendering artifact for a "+N more" pill shown when a parent
/// has more children than `sibling_limit`.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct AggregationPlaceholder {
    pub parent_id: NodeId,
    pub hidden_count: usize,
    pub layout_pos: Pos2,
    pub layout_size: Vec2,
    pub side: Option<Side>,
    pub depth: usize,
}

#[derive(Clone, Debug)]
pub struct MindmapTree {
    pub nodes: Vec<MindmapNode>,
    pub root: NodeId,
    id_map: HashMap<String, NodeId>,
    next_freemind_id: u64,
    /// Cached result of visible_nodes(). Invalidated by `invalidate_visible_cache()`.
    cached_visible: Option<Vec<NodeId>>,
    /// Maximum depth of visible nodes, set by layout.
    pub cached_max_depth: usize,
    /// Maximum number of children shown per parent before aggregation.
    pub sibling_limit: usize,
    /// Parents whose "+N more" pill was clicked — show all children.
    pub force_expanded: HashSet<NodeId>,
    /// Rebuilt each layout pass — ephemeral placeholders for hidden siblings.
    pub aggregation_placeholders: Vec<AggregationPlaceholder>,
}

impl MindmapTree {
    pub fn new(nodes: Vec<MindmapNode>, root: NodeId) -> Self {
        let mut id_map = HashMap::new();
        let mut max_id: u64 = 0;
        for node in &nodes {
            id_map.insert(node.freemind_id.clone(), node.id);
            // Parse numeric part of freemind_id like "ID_1402002741"
            if let Some(num_str) = node.freemind_id.strip_prefix("ID_") {
                if let Ok(num) = num_str.parse::<u64>() {
                    max_id = max_id.max(num);
                }
            }
        }
        Self {
            nodes,
            root,
            id_map,
            next_freemind_id: max_id + 1,
            cached_visible: None,
            cached_max_depth: 0,
            sibling_limit: 20,
            force_expanded: HashSet::new(),
            aggregation_placeholders: Vec::new(),
        }
    }

    fn alloc_freemind_id(&mut self) -> String {
        let id = format!("ID_{}", self.next_freemind_id);
        self.next_freemind_id += 1;
        id
    }

    #[cfg(test)]
    pub fn depth(&self, node_id: NodeId) -> usize {
        self.nodes[node_id].depth(&self.nodes)
    }

    /// Returns the effective side for a node (inherited from depth-1 ancestor).
    pub fn effective_side(&self, node_id: NodeId) -> Option<Side> {
        let mut current = node_id;
        loop {
            let node = &self.nodes[current];
            if node.position.is_some() {
                return node.position.clone();
            }
            match node.parent {
                Some(p) if p != self.root => current = p,
                _ => return None,
            }
        }
    }

    /// Get all visible node IDs (skipping children of folded nodes).
    /// Returns a cached result when available. Call `invalidate_visible_cache()`
    /// after changing fold state or tree structure.
    pub fn visible_nodes(&self) -> Vec<NodeId> {
        if let Some(ref cached) = self.cached_visible {
            return cached.clone();
        }
        self.compute_visible_nodes()
    }

    /// Recompute and cache visible nodes. Call this once per frame or after
    /// fold/structure changes, then use `visible_nodes()` which returns the cache.
    pub fn cache_visible_nodes(&mut self) {
        let v = self.compute_visible_nodes();
        self.cached_visible = Some(v);
    }

    /// Invalidate the cached visible nodes. Called when fold state or tree
    /// structure changes.
    pub fn invalidate_visible_cache(&mut self) {
        self.cached_visible = None;
        self.aggregation_placeholders.clear();
    }

    /// Returns the visible children slice for a node, respecting sibling_limit.
    /// Zero-copy: returns a sub-slice of the node's children Vec.
    pub fn visible_children(&self, node_id: NodeId) -> &[NodeId] {
        let node = &self.nodes[node_id];
        let children = &node.children;
        if children.len() <= self.sibling_limit || self.force_expanded.contains(&node_id) {
            children
        } else {
            &children[..self.sibling_limit]
        }
    }

    /// Number of children hidden by sibling aggregation for a given parent.
    pub fn hidden_child_count(&self, node_id: NodeId) -> usize {
        let total = self.nodes[node_id].children.len();
        let visible = self.visible_children(node_id).len();
        total - visible
    }

    /// Expand a parent to show all its children (clicked "+N more").
    pub fn expand_siblings(&mut self, parent_id: NodeId) {
        self.force_expanded.insert(parent_id);
        self.invalidate_visible_cache();
    }

    fn compute_visible_nodes(&self) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            result.push(id);
            let node = &self.nodes[id];
            if !node.folded {
                // Use visible_children to respect sibling_limit
                for &child_id in self.visible_children(id).iter().rev() {
                    stack.push(child_id);
                }
            }
        }
        result
    }

    /// Add a child node to the given parent. Returns the new node's ID.
    pub fn add_child(&mut self, parent_id: NodeId, text: &str) -> NodeId {
        let new_id = self.nodes.len();
        let freemind_id = self.alloc_freemind_id();

        let mut node = MindmapNode::new(new_id, freemind_id.clone(), text.to_string());
        node.parent = Some(parent_id);

        // If parent is root, assign a side
        if parent_id == self.root {
            let right_count = self.nodes[parent_id]
                .children
                .iter()
                .filter(|&&c| self.nodes[c].position == Some(Side::Right))
                .count();
            let left_count = self.nodes[parent_id]
                .children
                .iter()
                .filter(|&&c| self.nodes[c].position == Some(Side::Left))
                .count();
            node.position = if right_count <= left_count {
                Some(Side::Right)
            } else {
                Some(Side::Left)
            };
        }

        self.id_map.insert(freemind_id, new_id);
        self.nodes.push(node);
        self.nodes[parent_id].children.push(new_id);

        // Unfold parent so the new child is visible
        self.nodes[parent_id].folded = false;
        self.cached_visible = None;

        new_id
    }

    /// Add a sibling node below the given node. Returns the new node's ID.
    pub fn add_sibling(&mut self, node_id: NodeId, text: &str) -> NodeId {
        let parent_id = match self.nodes[node_id].parent {
            Some(p) => p,
            None => return self.add_child(node_id, text), // root has no siblings
        };

        let new_id = self.nodes.len();
        let freemind_id = self.alloc_freemind_id();

        let mut node = MindmapNode::new(new_id, freemind_id.clone(), text.to_string());
        node.parent = Some(parent_id);

        // Inherit position from sibling
        node.position = self.nodes[node_id].position.clone();

        self.id_map.insert(freemind_id, new_id);
        self.nodes.push(node);

        // Insert after the sibling in parent's children
        let idx = self.nodes[parent_id]
            .children
            .iter()
            .position(|&c| c == node_id)
            .unwrap_or(0);
        self.nodes[parent_id].children.insert(idx + 1, new_id);
        self.cached_visible = None;

        new_id
    }

    /// Delete a node and all its descendants. Returns the deleted subtree for undo.
    pub fn delete_subtree(&mut self, node_id: NodeId) -> Option<Vec<MindmapNode>> {
        if node_id == self.root {
            return None; // cannot delete root
        }

        // Collect all nodes in subtree
        let mut subtree_ids = Vec::new();
        self.collect_subtree(node_id, &mut subtree_ids);

        // Remove from parent's children
        if let Some(parent_id) = self.nodes[node_id].parent {
            self.nodes[parent_id].children.retain(|&c| c != node_id);
        }

        // Save subtree nodes for undo (clone before marking deleted)
        let subtree: Vec<MindmapNode> = subtree_ids
            .iter()
            .map(|&id| self.nodes[id].clone())
            .collect();

        // Mark nodes as deleted (clear text to indicate deletion, keep in arena for index stability)
        for &id in &subtree_ids {
            self.nodes[id].children.clear();
            self.nodes[id].parent = None;
            self.nodes[id].text = String::new();
        }
        self.cached_visible = None;

        Some(subtree)
    }

    fn collect_subtree(&self, root: NodeId, result: &mut Vec<NodeId>) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            result.push(id);
            for &child_id in self.nodes[id].children.iter().rev() {
                stack.push(child_id);
            }
        }
    }

    pub fn toggle_fold(&mut self, node_id: NodeId) {
        if !self.nodes[node_id].children.is_empty() {
            self.nodes[node_id].folded = !self.nodes[node_id].folded;
            self.cached_visible = None;
        }
    }

    /// Progressively fold nodes from the deepest foldable level upward until
    /// the visible node count is at or below `max_visible`. Used on load to
    /// keep large files navigable. Already-folded nodes are left as-is.
    ///
    /// Single DFS to compute depths and group foldable nodes by depth, then
    /// folds from deepest upward. Counts descendant subtree sizes to update
    /// the visible count without re-traversing the tree.
    pub fn auto_fold_for_display(&mut self, max_visible: usize) {
        // Single DFS: compute depth for every visible node, and count
        // the number of visible descendants under each node.
        let n = self.nodes.len();
        let mut depth_of = vec![0usize; n];
        let mut subtree_visible = vec![1usize; n]; // each node counts itself
        let mut max_depth = 0usize;

        // Post-order DFS so we can compute subtree sizes bottom-up.
        let mut stack: Vec<(NodeId, bool)> = vec![(self.root, false)];
        while let Some((id, processed)) = stack.pop() {
            if processed {
                // Sum children's subtree sizes into this node.
                let node = &self.nodes[id];
                if !node.folded {
                    for &c in &node.children {
                        subtree_visible[id] += subtree_visible[c];
                    }
                }
            } else {
                stack.push((id, true));
                let node = &self.nodes[id];
                if !node.folded {
                    let child_depth = depth_of[id] + 1;
                    for &c in node.children.iter().rev() {
                        depth_of[c] = child_depth;
                        if child_depth > max_depth {
                            max_depth = child_depth;
                        }
                        stack.push((c, false));
                    }
                }
            }
        }

        let mut visible_count = subtree_visible[self.root];
        if visible_count <= max_visible {
            return;
        }

        // Group foldable (non-leaf, unfolded) node IDs by depth.
        let mut by_depth: Vec<Vec<NodeId>> = vec![Vec::new(); max_depth + 1];
        for id in 0..n {
            if !self.nodes[id].children.is_empty()
                && !self.nodes[id].folded
                && !self.nodes[id].text.is_empty()
            {
                by_depth[depth_of[id]].push(id);
            }
        }

        // Fold from deepest level upward.
        for d in (2..=max_depth).rev() {
            for &id in &by_depth[d] {
                if !self.nodes[id].folded {
                    self.nodes[id].folded = true;
                    // Folding this node hides its descendants (subtract them,
                    // but the node itself stays visible).
                    let hidden = subtree_visible[id] - 1;
                    visible_count -= hidden;
                }
            }
            if visible_count <= max_visible {
                break;
            }
        }
        self.cached_visible = None;
    }

    /// First visible (not folded-away) child of a node.
    pub fn first_visible_child(&self, node_id: NodeId) -> Option<NodeId> {
        let node = &self.nodes[node_id];
        if node.folded {
            return None;
        }
        self.visible_children(node_id).first().copied()
    }

    /// Previous sibling in parent's visible children list.
    pub fn prev_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let parent_id = self.nodes[node_id].parent?;
        let children = self.visible_children(parent_id);
        let idx = children.iter().position(|&c| c == node_id)?;
        if idx > 0 {
            Some(children[idx - 1])
        } else {
            None
        }
    }

    /// Next sibling in parent's visible children list.
    pub fn next_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let parent_id = self.nodes[node_id].parent?;
        let children = self.visible_children(parent_id);
        let idx = children.iter().position(|&c| c == node_id)?;
        if idx + 1 < children.len() {
            Some(children[idx + 1])
        } else {
            None
        }
    }

    /// Move node up among its siblings. Returns (parent_id, old_index, new_index) for undo.
    pub fn move_sibling_up(&mut self, node_id: NodeId) -> Option<(NodeId, usize, usize)> {
        let parent_id = self.nodes[node_id].parent?;
        let children = &self.nodes[parent_id].children;
        let idx = children.iter().position(|&c| c == node_id)?;
        if idx == 0 {
            return None;
        }
        let new_idx = idx - 1;
        self.nodes[parent_id].children.swap(idx, new_idx);
        Some((parent_id, idx, new_idx))
    }

    /// Move node down among its siblings. Returns (parent_id, old_index, new_index) for undo.
    pub fn move_sibling_down(&mut self, node_id: NodeId) -> Option<(NodeId, usize, usize)> {
        let parent_id = self.nodes[node_id].parent?;
        let children = &self.nodes[parent_id].children;
        let len = children.len();
        let idx = children.iter().position(|&c| c == node_id)?;
        if idx + 1 >= len {
            return None;
        }
        let new_idx = idx + 1;
        self.nodes[parent_id].children.swap(idx, new_idx);
        Some((parent_id, idx, new_idx))
    }

    /// Add a sibling node BEFORE the given node. Returns the new node's ID.
    pub fn add_sibling_before(&mut self, node_id: NodeId, text: &str) -> NodeId {
        let parent_id = match self.nodes[node_id].parent {
            Some(p) => p,
            None => return self.add_child(node_id, text), // root has no siblings
        };

        let new_id = self.nodes.len();
        let freemind_id = self.alloc_freemind_id();

        let mut node = MindmapNode::new(new_id, freemind_id.clone(), text.to_string());
        node.parent = Some(parent_id);
        node.position = self.nodes[node_id].position.clone();

        self.id_map.insert(freemind_id, new_id);
        self.nodes.push(node);

        // Insert BEFORE the sibling in parent's children
        let idx = self.nodes[parent_id]
            .children
            .iter()
            .position(|&c| c == node_id)
            .unwrap_or(0);
        self.nodes[parent_id].children.insert(idx, new_id);
        self.cached_visible = None;

        new_id
    }

    /// Check if `ancestor` is an ancestor of `descendant`.
    pub fn is_ancestor(&self, ancestor: NodeId, descendant: NodeId) -> bool {
        let mut current = self.nodes[descendant].parent;
        while let Some(pid) = current {
            if pid == ancestor {
                return true;
            }
            current = self.nodes[pid].parent;
        }
        false
    }

    /// Reparent a node: detach from old parent and attach as last child of new parent.
    /// Returns (old_parent_id, old_child_index, old_position) for undo.
    pub fn reparent_node(
        &mut self,
        node_id: NodeId,
        new_parent: NodeId,
    ) -> Option<(NodeId, usize, Option<Side>)> {
        let old_parent = self.nodes[node_id].parent?;
        let old_index = self.nodes[old_parent]
            .children
            .iter()
            .position(|&c| c == node_id)?;
        let old_position = self.nodes[node_id].position.clone();

        // Remove from old parent
        self.nodes[old_parent].children.remove(old_index);

        // Attach to new parent
        self.nodes[node_id].parent = Some(new_parent);
        self.nodes[new_parent].children.push(node_id);

        // Handle side assignment
        if new_parent == self.root {
            // Assign side to balance left/right
            let right_count = self.nodes[new_parent]
                .children
                .iter()
                .filter(|&&c| self.nodes[c].position == Some(Side::Right))
                .count();
            let left_count = self.nodes[new_parent]
                .children
                .iter()
                .filter(|&&c| self.nodes[c].position == Some(Side::Left))
                .count();
            self.nodes[node_id].position = if right_count <= left_count {
                Some(Side::Right)
            } else {
                Some(Side::Left)
            };
        } else {
            // Clear position — inherits from ancestor
            self.nodes[node_id].position = None;
        }

        // Unfold new parent so moved node is visible
        self.nodes[new_parent].folded = false;
        self.cached_visible = None;

        Some((old_parent, old_index, old_position))
    }

    /// Get the index of a node in its parent's children list.
    pub fn child_index(&self, node_id: NodeId) -> Option<usize> {
        let parent_id = self.nodes[node_id].parent?;
        self.nodes[parent_id]
            .children
            .iter()
            .position(|&c| c == node_id)
    }

    /// Filter out nodes whose ancestor is also in the selection set.
    pub fn deduplicate_selection(&self, selected: &HashSet<NodeId>) -> Vec<NodeId> {
        let mut result = Vec::new();
        'outer: for &id in selected {
            let mut current = self.nodes[id].parent;
            while let Some(pid) = current {
                if selected.contains(&pid) {
                    continue 'outer; // ancestor is selected, skip this one
                }
                current = self.nodes[pid].parent;
            }
            result.push(id);
        }
        result
    }

    /// Walk ancestors of `node_id` and unfold any that are folded.
    /// Also auto-expands parents where `node_id` is hidden by sibling aggregation.
    /// Returns true if any ancestor was unfolded or expanded.
    pub fn unfold_path_to(&mut self, node_id: NodeId) -> bool {
        let mut changed = false;

        // If this node's parent has sibling aggregation hiding it, force-expand
        if let Some(parent_id) = self.nodes[node_id].parent {
            let children = &self.nodes[parent_id].children;
            if children.len() > self.sibling_limit && !self.force_expanded.contains(&parent_id) {
                // Check if node_id is beyond the visible slice
                if let Some(idx) = children.iter().position(|&c| c == node_id) {
                    if idx >= self.sibling_limit {
                        self.force_expanded.insert(parent_id);
                        changed = true;
                    }
                }
            }
        }

        let mut current = self.nodes[node_id].parent;
        while let Some(pid) = current {
            if self.nodes[pid].folded {
                self.nodes[pid].folded = false;
                changed = true;
            }
            current = self.nodes[pid].parent;
        }
        if changed {
            self.cached_visible = None;
        }
        changed
    }

    /// Full DFS traversal of the tree, returning node IDs in order.
    pub fn dfs_order(&self) -> Vec<NodeId> {
        let mut result = Vec::new();
        self.collect_dfs(self.root, &mut result);
        result
    }

    fn collect_dfs(&self, root: NodeId, result: &mut Vec<NodeId>) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if self.nodes[id].text.is_empty() {
                continue; // skip deleted nodes
            }
            result.push(id);
            for &child_id in self.nodes[id].children.iter().rev() {
                stack.push(child_id);
            }
        }
    }

    /// Deep-clone a subtree into a blueprint for clipboard storage.
    pub fn clone_subtree(&self, node_id: NodeId) -> SubtreeBlueprint {
        let mut ids = Vec::new();
        self.collect_subtree(node_id, &mut ids);
        let nodes: Vec<MindmapNode> = ids.iter().map(|&id| self.nodes[id].clone()).collect();
        SubtreeBlueprint { nodes }
    }

    /// Create a minimal tree with just a root node, for testing/new maps.
    #[cfg(test)]
    pub fn new_empty(root_text: &str) -> Self {
        let root = MindmapNode::new(0, "ID_0".to_string(), root_text.to_string());
        Self::new(vec![root], 0)
    }

    /// Paste a blueprint as a child of `parent_id`.
    /// Allocates new arena slots, remaps all internal refs, attaches root to parent.
    /// Returns (new_root_id, all_new_ids).
    pub fn paste_subtree(
        &mut self,
        blueprint: &SubtreeBlueprint,
        parent_id: NodeId,
    ) -> (NodeId, Vec<NodeId>) {
        if blueprint.nodes.is_empty() {
            return (0, Vec::new());
        }

        // Build old_id → new_id mapping
        let mut id_remap: HashMap<NodeId, NodeId> = HashMap::new();
        let base = self.nodes.len();
        for (i, bp_node) in blueprint.nodes.iter().enumerate() {
            id_remap.insert(bp_node.id, base + i);
        }

        let mut all_new_ids = Vec::new();

        for (i, bp_node) in blueprint.nodes.iter().enumerate() {
            let new_id = base + i;
            let freemind_id = self.alloc_freemind_id();

            let mut node = bp_node.clone();
            node.id = new_id;
            node.freemind_id = freemind_id.clone();

            // Remap parent
            if i == 0 {
                // Root of pasted subtree → attach to target parent
                node.parent = Some(parent_id);
                // If pasting under root, assign a side
                if parent_id == self.root {
                    let right_count = self.nodes[parent_id]
                        .children
                        .iter()
                        .filter(|&&c| self.nodes[c].position == Some(Side::Right))
                        .count();
                    let left_count = self.nodes[parent_id]
                        .children
                        .iter()
                        .filter(|&&c| self.nodes[c].position == Some(Side::Left))
                        .count();
                    node.position = if right_count <= left_count {
                        Some(Side::Right)
                    } else {
                        Some(Side::Left)
                    };
                }
            } else {
                node.parent = bp_node
                    .parent
                    .map(|old| *id_remap.get(&old).unwrap_or(&old));
            }

            // Remap children
            node.children = bp_node
                .children
                .iter()
                .map(|&old| *id_remap.get(&old).unwrap_or(&old))
                .collect();

            self.id_map.insert(freemind_id, new_id);
            self.nodes.push(node);
            all_new_ids.push(new_id);
        }

        let new_root_id = base;

        // Attach to parent's children list
        self.nodes[parent_id].children.push(new_root_id);
        // Unfold parent so pasted child is visible
        self.nodes[parent_id].folded = false;
        self.cached_visible = None;

        (new_root_id, all_new_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build: root → { A (right), B (left) }, A → { A1, A2 }
    fn sample_tree() -> MindmapTree {
        let mut tree = MindmapTree::new_empty("Root");
        // First child goes right (right_count=0 <= left_count=0)
        let a = tree.add_child(tree.root, "A");
        assert_eq!(tree.nodes[a].position, Some(Side::Right));
        // Second child goes left (right=1 > left=0)
        let b = tree.add_child(tree.root, "B");
        assert_eq!(tree.nodes[b].position, Some(Side::Left));
        // Children of A (non-root, no position assigned)
        let _a1 = tree.add_child(a, "A1");
        let _a2 = tree.add_child(a, "A2");
        tree
    }

    fn find(tree: &MindmapTree, text: &str) -> NodeId {
        tree.nodes
            .iter()
            .find(|n| n.text == text)
            .unwrap_or_else(|| panic!("node '{}' not found", text))
            .id
    }

    #[test]
    fn add_child_balances_sides() {
        let tree = sample_tree();
        let root = tree.root;
        assert_eq!(tree.nodes[root].children.len(), 2);
        // Third child should go right again (right=1, left=1 → right)
        let mut tree = tree;
        let c = tree.add_child(root, "C");
        assert_eq!(tree.nodes[c].position, Some(Side::Right));
    }

    #[test]
    fn add_child_to_non_root() {
        let mut tree = sample_tree();
        let a = find(&tree, "A");
        let a3 = tree.add_child(a, "A3");
        assert_eq!(tree.nodes[a3].parent, Some(a));
        assert!(tree.nodes[a3].position.is_none()); // non-root children have no position
        assert_eq!(tree.nodes[a].children.len(), 3);
    }

    #[test]
    fn add_child_unfolds_parent() {
        let mut tree = sample_tree();
        let a = find(&tree, "A");
        tree.toggle_fold(a);
        assert!(tree.nodes[a].folded);
        tree.add_child(a, "A3");
        assert!(!tree.nodes[a].folded);
    }

    #[test]
    fn add_sibling_after() {
        let mut tree = sample_tree();
        let a1 = find(&tree, "A1");
        let a = find(&tree, "A");
        let s = tree.add_sibling(a1, "S");
        assert_eq!(tree.nodes[s].parent, Some(a));
        // S should be right after A1
        let children = &tree.nodes[a].children;
        let a1_idx = children.iter().position(|&c| c == a1).unwrap();
        assert_eq!(children[a1_idx + 1], s);
    }

    #[test]
    fn add_sibling_before() {
        let mut tree = sample_tree();
        let a2 = find(&tree, "A2");
        let a = find(&tree, "A");
        let s = tree.add_sibling_before(a2, "S");
        let children = &tree.nodes[a].children;
        let a2_idx = children.iter().position(|&c| c == a2).unwrap();
        assert_eq!(children[a2_idx - 1], s);
    }

    #[test]
    fn add_sibling_inherits_position() {
        let mut tree = sample_tree();
        let a = find(&tree, "A");
        let s = tree.add_sibling(a, "S");
        assert_eq!(tree.nodes[s].position, Some(Side::Right));
    }

    #[test]
    fn delete_subtree_cascades() {
        let mut tree = sample_tree();
        let a = find(&tree, "A");
        let a1 = find(&tree, "A1");
        let a2 = find(&tree, "A2");
        let saved = tree.delete_subtree(a);
        assert!(saved.is_some());
        let saved = saved.unwrap();
        assert_eq!(saved.len(), 3); // A, A1, A2
                                    // All deleted nodes should be cleared
        assert!(tree.nodes[a].text.is_empty());
        assert!(tree.nodes[a1].text.is_empty());
        assert!(tree.nodes[a2].text.is_empty());
        // Root should no longer reference A
        assert!(!tree.nodes[tree.root].children.contains(&a));
    }

    #[test]
    fn delete_root_returns_none() {
        let mut tree = sample_tree();
        assert!(tree.delete_subtree(tree.root).is_none());
    }

    #[test]
    fn is_ancestor() {
        let tree = sample_tree();
        let a = find(&tree, "A");
        let a1 = find(&tree, "A1");
        assert!(tree.is_ancestor(tree.root, a1));
        assert!(tree.is_ancestor(a, a1));
        assert!(!tree.is_ancestor(a1, a)); // not reverse
        assert!(!tree.is_ancestor(a, a)); // not self
    }

    #[test]
    fn reparent_node() {
        let mut tree = sample_tree();
        let a1 = find(&tree, "A1");
        let b = find(&tree, "B");
        let a = find(&tree, "A");
        let result = tree.reparent_node(a1, b);
        assert!(result.is_some());
        let (old_parent, _, _) = result.unwrap();
        assert_eq!(old_parent, a);
        assert_eq!(tree.nodes[a1].parent, Some(b));
        assert!(tree.nodes[b].children.contains(&a1));
        assert!(!tree.nodes[a].children.contains(&a1));
    }

    #[test]
    fn move_sibling_up_down() {
        let mut tree = sample_tree();
        let a = find(&tree, "A");
        let a1 = find(&tree, "A1");
        let a2 = find(&tree, "A2");
        // A1 is already first — move up should return None
        assert!(tree.move_sibling_up(a1).is_none());
        // Move A2 up
        let result = tree.move_sibling_down(a1);
        assert!(result.is_some());
        assert_eq!(tree.nodes[a].children, vec![a2, a1]);
        // Move back
        tree.move_sibling_up(a1);
        assert_eq!(tree.nodes[a].children, vec![a1, a2]);
        // A2 is last — move down should return None
        assert!(tree.move_sibling_down(a2).is_none());
    }

    #[test]
    fn toggle_fold_and_visible_nodes() {
        let mut tree = sample_tree();
        let a = find(&tree, "A");
        let a1 = find(&tree, "A1");
        let a2 = find(&tree, "A2");
        // All visible initially
        let visible = tree.visible_nodes();
        assert!(visible.contains(&a1));
        assert!(visible.contains(&a2));
        // Fold A
        tree.toggle_fold(a);
        assert!(tree.nodes[a].folded);
        let visible = tree.visible_nodes();
        assert!(!visible.contains(&a1));
        assert!(!visible.contains(&a2));
        assert!(visible.contains(&a)); // A itself is still visible
                                       // Unfold
        tree.toggle_fold(a);
        assert!(!tree.nodes[a].folded);
        let visible = tree.visible_nodes();
        assert!(visible.contains(&a1));
    }

    #[test]
    fn depth_and_dfs_order() {
        let tree = sample_tree();
        let a = find(&tree, "A");
        let a1 = find(&tree, "A1");
        assert_eq!(tree.depth(tree.root), 0);
        assert_eq!(tree.depth(a), 1);
        assert_eq!(tree.depth(a1), 2);
        let dfs = tree.dfs_order();
        // Root should be first
        assert_eq!(dfs[0], tree.root);
        // All 5 nodes present
        assert_eq!(dfs.len(), 5);
    }

    #[test]
    fn clone_and_paste_subtree() {
        let mut tree = sample_tree();
        let a = find(&tree, "A");
        let b = find(&tree, "B");
        let blueprint = tree.clone_subtree(a);
        assert_eq!(blueprint.nodes.len(), 3); // A, A1, A2
        let (new_root, all_new) = tree.paste_subtree(&blueprint, b);
        assert_eq!(all_new.len(), 3);
        // New root should be a child of B
        assert_eq!(tree.nodes[new_root].parent, Some(b));
        assert!(tree.nodes[b].children.contains(&new_root));
        // New IDs should be different from originals
        assert_ne!(new_root, a);
        // Text should be preserved
        assert_eq!(tree.nodes[new_root].text, "A");
        // Children should be remapped
        let new_children = &tree.nodes[new_root].children;
        assert_eq!(new_children.len(), 2);
        assert_eq!(tree.nodes[new_children[0]].text, "A1");
        assert_eq!(tree.nodes[new_children[1]].text, "A2");
    }

    #[test]
    fn deduplicate_selection_removes_descendants() {
        let tree = sample_tree();
        let a = find(&tree, "A");
        let a1 = find(&tree, "A1");
        let b = find(&tree, "B");
        let mut selected = HashSet::new();
        selected.insert(a);
        selected.insert(a1); // descendant of A
        selected.insert(b);
        let deduped = tree.deduplicate_selection(&selected);
        assert_eq!(deduped.len(), 2);
        assert!(deduped.contains(&a));
        assert!(deduped.contains(&b));
        assert!(!deduped.contains(&a1));
    }

    #[test]
    fn unfold_path_to() {
        let mut tree = sample_tree();
        let a = find(&tree, "A");
        let a1 = find(&tree, "A1");
        tree.nodes[a].folded = true;
        assert!(tree.unfold_path_to(a1));
        assert!(!tree.nodes[a].folded);
    }
}
