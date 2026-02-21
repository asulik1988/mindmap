use super::NodeId;
use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub selected: HashSet<NodeId>,
    pub hovered: Option<NodeId>,
}

impl Selection {
    pub fn select_single(&mut self, id: NodeId) {
        self.selected.clear();
        self.selected.insert(id);
    }

    pub fn toggle(&mut self, id: NodeId) {
        if !self.selected.remove(&id) {
            self.selected.insert(id);
        }
    }

    pub fn clear(&mut self) {
        self.selected.clear();
    }

    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selected.contains(&id)
    }

    pub fn primary(&self) -> Option<NodeId> {
        self.selected.iter().next().copied()
    }
}
