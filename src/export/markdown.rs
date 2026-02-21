use crate::model::{MindmapTree, NodeId};

pub fn export_markdown(tree: &MindmapTree) -> String {
    let mut out = String::new();
    write_node_md(tree, tree.root, 0, &mut out);
    out
}

fn write_node_md(tree: &MindmapTree, id: NodeId, depth: usize, out: &mut String) {
    let node = &tree.nodes[id];
    if node.text.is_empty() && id != tree.root {
        return; // skip deleted nodes
    }

    // Heading level: depth 0-5 → # to ######, deeper → indented bullet
    if depth <= 5 {
        let hashes = "#".repeat(depth + 1);
        out.push_str(&format!("{} {}\n", hashes, node.text));
    } else {
        let indent = "  ".repeat(depth - 6);
        out.push_str(&format!("{}- {}\n", indent, node.text));
    }

    // Notes as blockquote
    if !node.notes.is_empty() {
        for line in node.notes.lines() {
            out.push_str(&format!("> {}\n", line));
        }
        out.push('\n');
    }

    for &child_id in &node.children {
        write_node_md(tree, child_id, depth + 1, out);
    }
}
