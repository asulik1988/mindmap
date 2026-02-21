use super::node::{MindmapNode, NodeId, Side};
use std::collections::HashMap;

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
            self.nodes[parent_id]
                .children
                .retain(|&c| c != node_id);
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
}
