use super::clipboard::SubtreeBlueprint;
use super::node::{MindmapNode, NodeId, Side};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct MindmapTree {
    pub nodes: Vec<MindmapNode>,
    pub root: NodeId,
    id_map: HashMap<String, NodeId>,
    next_freemind_id: u64,
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
        }
    }

    fn alloc_freemind_id(&mut self) -> String {
        let id = format!("ID_{}", self.next_freemind_id);
        self.next_freemind_id += 1;
        id
    }

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
    pub fn visible_nodes(&self) -> Vec<NodeId> {
        let mut result = Vec::new();
        self.collect_visible(self.root, &mut result);
        result
    }

    fn collect_visible(&self, node_id: NodeId, result: &mut Vec<NodeId>) {
        result.push(node_id);
        let node = &self.nodes[node_id];
        if !node.folded {
            for &child_id in &node.children.clone() {
                self.collect_visible(child_id, result);
            }
        }
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

        Some(subtree)
    }

    fn collect_subtree(&self, node_id: NodeId, result: &mut Vec<NodeId>) {
        result.push(node_id);
        for &child_id in &self.nodes[node_id].children {
            self.collect_subtree(child_id, result);
        }
    }

    pub fn toggle_fold(&mut self, node_id: NodeId) {
        if !self.nodes[node_id].children.is_empty() {
            self.nodes[node_id].folded = !self.nodes[node_id].folded;
        }
    }

    /// First visible (not folded-away) child of a node.
    pub fn first_visible_child(&self, node_id: NodeId) -> Option<NodeId> {
        let node = &self.nodes[node_id];
        if node.folded {
            return None;
        }
        node.children.first().copied()
    }

    /// Previous sibling in parent's children list.
    pub fn prev_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let parent_id = self.nodes[node_id].parent?;
        let children = &self.nodes[parent_id].children;
        let idx = children.iter().position(|&c| c == node_id)?;
        if idx > 0 {
            Some(children[idx - 1])
        } else {
            None
        }
    }

    /// Next sibling in parent's children list.
    pub fn next_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let parent_id = self.nodes[node_id].parent?;
        let children = &self.nodes[parent_id].children;
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
    /// Returns true if any ancestor was unfolded.
    pub fn unfold_path_to(&mut self, node_id: NodeId) -> bool {
        let mut changed = false;
        let mut current = self.nodes[node_id].parent;
        while let Some(pid) = current {
            if self.nodes[pid].folded {
                self.nodes[pid].folded = false;
                changed = true;
            }
            current = self.nodes[pid].parent;
        }
        changed
    }

    /// Full DFS traversal of the tree, returning node IDs in order.
    pub fn dfs_order(&self) -> Vec<NodeId> {
        let mut result = Vec::new();
        self.collect_dfs(self.root, &mut result);
        result
    }

    fn collect_dfs(&self, node_id: NodeId, result: &mut Vec<NodeId>) {
        if self.nodes[node_id].text.is_empty() {
            return; // skip deleted nodes
        }
        result.push(node_id);
        for &child_id in &self.nodes[node_id].children {
            self.collect_dfs(child_id, result);
        }
    }

    /// Deep-clone a subtree into a blueprint for clipboard storage.
    pub fn clone_subtree(&self, node_id: NodeId) -> SubtreeBlueprint {
        let mut ids = Vec::new();
        self.collect_subtree(node_id, &mut ids);
        let nodes: Vec<MindmapNode> = ids.iter().map(|&id| self.nodes[id].clone()).collect();
        SubtreeBlueprint { nodes }
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

        (new_root_id, all_new_ids)
    }
}
