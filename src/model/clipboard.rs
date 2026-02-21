use super::node::MindmapNode;

/// A deep-cloned subtree ready for pasting. Nodes retain their original
/// structure (parent/children refs) but will get new arena IDs on paste.
#[derive(Clone, Debug)]
pub struct SubtreeBlueprint {
    /// Nodes in the subtree, root first. Original arena IDs preserved for
    /// internal reference only — paste allocates fresh slots.
    pub nodes: Vec<MindmapNode>,
}

/// Internal clipboard holding copied subtrees.
#[derive(Clone, Debug, Default)]
pub struct Clipboard {
    pub blueprints: Vec<SubtreeBlueprint>,
}

impl Clipboard {
    pub fn is_empty(&self) -> bool {
        self.blueprints.is_empty()
    }

    pub fn clear(&mut self) {
        self.blueprints.clear();
    }
}
