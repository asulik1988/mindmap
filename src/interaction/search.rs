use crate::model::{MindmapTree, NodeId};

pub struct SearchState {
    pub active: bool,
    pub query: String,
    pub matches: Vec<NodeId>,
    pub current_index: usize,
    pub select_all_pending: bool,
    pub replace_text: String,
    pub replace_active: bool,
    prev_query: String,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            active: false,
            query: String::new(),
            matches: Vec::new(),
            current_index: 0,
            select_all_pending: false,
            replace_text: String::new(),
            replace_active: false,
            prev_query: String::new(),
        }
    }
}

impl SearchState {
    pub fn open(&mut self) {
        self.active = true;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.matches.clear();
        self.current_index = 0;
        self.prev_query.clear();
        self.replace_text.clear();
        self.replace_active = false;
    }

    pub fn toggle_replace(&mut self) {
        self.replace_active = !self.replace_active;
    }

    pub fn update_matches_force(&mut self, tree: &MindmapTree) {
        self.prev_query.clear();
        self.update_matches(tree);
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Re-scan matches when query changes. Case-insensitive substring match.
    pub fn update_matches(&mut self, tree: &MindmapTree) {
        if self.query == self.prev_query {
            return;
        }
        self.prev_query = self.query.clone();

        self.matches.clear();
        if self.query.is_empty() {
            self.current_index = 0;
            return;
        }

        let query_lower = self.query.to_lowercase();
        let dfs = tree.dfs_order();
        for node_id in dfs {
            let node = &tree.nodes[node_id];
            if !node.text.is_empty() && node.text.to_lowercase().contains(&query_lower) {
                self.matches.push(node_id);
            }
        }

        // Try to keep current_index in bounds
        if self.matches.is_empty() {
            self.current_index = 0;
        } else if self.current_index >= self.matches.len() {
            self.current_index = 0;
        }
    }

    pub fn next(&mut self) {
        if !self.matches.is_empty() {
            self.current_index = (self.current_index + 1) % self.matches.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.matches.is_empty() {
            if self.current_index == 0 {
                self.current_index = self.matches.len() - 1;
            } else {
                self.current_index -= 1;
            }
        }
    }

    pub fn current_match(&self) -> Option<NodeId> {
        self.matches.get(self.current_index).copied()
    }

    /// Update current_index when user clicks a match node.
    pub fn jump_to_node(&mut self, node_id: NodeId) {
        if let Some(idx) = self.matches.iter().position(|&id| id == node_id) {
            self.current_index = idx;
        }
    }
}
