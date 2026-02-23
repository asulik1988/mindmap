use crate::model::{MindmapTree, NodeId};
use std::fmt::Write as FmtWrite;

pub fn export_opml(tree: &MindmapTree) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(out, "<opml version=\"2.0\">");
    let _ = writeln!(
        out,
        "  <head><title>{}</title></head>",
        xml_escape(&tree.nodes[tree.root].text)
    );
    let _ = writeln!(out, "  <body>");
    write_node_opml(tree, tree.root, 2, &mut out);
    let _ = writeln!(out, "  </body>");
    let _ = writeln!(out, "</opml>");
    out
}

enum OpmlPhase {
    Enter(NodeId, usize),
    Leave(usize),
}

fn write_node_opml(tree: &MindmapTree, root: NodeId, root_indent: usize, out: &mut String) {
    let mut stack: Vec<OpmlPhase> = vec![OpmlPhase::Enter(root, root_indent)];
    while let Some(phase) = stack.pop() {
        match phase {
            OpmlPhase::Enter(id, indent) => {
                let node = &tree.nodes[id];
                if node.text.is_empty() && id != tree.root {
                    continue; // skip deleted nodes
                }

                let pad = "  ".repeat(indent);
                let text_attr = xml_escape(&node.text);
                let has_children = !node.children.is_empty();

                if has_children {
                    if node.notes.is_empty() {
                        let _ = writeln!(out, "{}<outline text=\"{}\">", pad, text_attr);
                    } else {
                        let note_attr = xml_escape(&node.notes);
                        let _ = writeln!(
                            out,
                            "{}<outline text=\"{}\" _note=\"{}\">",
                            pad, text_attr, note_attr
                        );
                    }
                    // Push close tag first, then children in reverse
                    stack.push(OpmlPhase::Leave(indent));
                    for &child_id in node.children.iter().rev() {
                        stack.push(OpmlPhase::Enter(child_id, indent + 1));
                    }
                } else if node.notes.is_empty() {
                    let _ = writeln!(out, "{}<outline text=\"{}\"/>", pad, text_attr);
                } else {
                    let note_attr = xml_escape(&node.notes);
                    let _ = writeln!(
                        out,
                        "{}<outline text=\"{}\" _note=\"{}\"/>",
                        pad, text_attr, note_attr
                    );
                }
            }
            OpmlPhase::Leave(indent) => {
                let pad = "  ".repeat(indent);
                let _ = writeln!(out, "{}</outline>", pad);
            }
        }
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MindmapTree;

    #[test]
    fn valid_opml_structure() {
        let mut tree = MindmapTree::new_empty("My Map");
        tree.add_child(tree.root, "Child");
        let opml = export_opml(&tree);
        assert!(opml.contains("<?xml version=\"1.0\""));
        assert!(opml.contains("<opml version=\"2.0\">"));
        assert!(opml.contains("<title>My Map</title>"));
        assert!(opml.contains("<body>"));
        assert!(opml.contains("</body>"));
        assert!(opml.contains("</opml>"));
        assert!(opml.contains("text=\"Child\""));
    }

    #[test]
    fn notes_as_attribute() {
        let mut tree = MindmapTree::new_empty("Root");
        let c = tree.add_child(tree.root, "WithNote");
        tree.nodes[c].notes = "A note".to_string();
        let opml = export_opml(&tree);
        assert!(opml.contains("_note=\"A note\""));
    }

    #[test]
    fn xml_escaping_in_output() {
        let mut tree = MindmapTree::new_empty("A & B < C");
        let opml = export_opml(&tree);
        assert!(opml.contains("A &amp; B &lt; C"));
    }
}
