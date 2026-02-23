use crate::model::{MindmapTree, NodeId};

pub fn export_markdown(tree: &MindmapTree) -> String {
    let mut out = String::new();
    write_node_md(tree, tree.root, 0, &mut out);
    out
}

fn write_node_md(tree: &MindmapTree, root: NodeId, root_depth: usize, out: &mut String) {
    let mut stack: Vec<(NodeId, usize)> = vec![(root, root_depth)];
    while let Some((id, depth)) = stack.pop() {
        let node = &tree.nodes[id];
        if node.text.is_empty() && id != tree.root {
            continue; // skip deleted nodes
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

        for &child_id in node.children.iter().rev() {
            stack.push((child_id, depth + 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MindmapTree;

    #[test]
    fn heading_depths() {
        let mut tree = MindmapTree::new_empty("Root");
        let d1 = tree.add_child(tree.root, "D1");
        let d2 = tree.add_child(d1, "D2");
        let d3 = tree.add_child(d2, "D3");
        let d4 = tree.add_child(d3, "D4");
        let d5 = tree.add_child(d4, "D5");
        tree.add_child(d5, "D6");
        let md = export_markdown(&tree);
        assert!(md.contains("# Root\n"));
        assert!(md.contains("## D1\n"));
        assert!(md.contains("### D2\n"));
        assert!(md.contains("#### D3\n"));
        assert!(md.contains("##### D4\n"));
        assert!(md.contains("###### D5\n"));
        assert!(md.contains("- D6\n")); // depth 6+ uses bullets
    }

    #[test]
    fn notes_as_blockquotes() {
        let mut tree = MindmapTree::new_empty("Root");
        tree.nodes[tree.root].notes = "A note\nSecond line".to_string();
        let md = export_markdown(&tree);
        assert!(md.contains("> A note\n"));
        assert!(md.contains("> Second line\n"));
    }
}
