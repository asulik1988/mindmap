use crate::model::{MindmapTree, Side};
use anyhow::Result;
use std::fmt::Write as FmtWrite;
use std::path::Path;

pub fn save_mm_file(tree: &MindmapTree, path: &Path) -> Result<()> {
    let xml = serialize_tree(tree)?;
    std::fs::write(path, xml)?;
    Ok(())
}

pub fn serialize_tree(tree: &MindmapTree) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "<map version=\"1.0.1\">")?;
    writeln!(
        out,
        "<!-- To view this file, download free mind mapping software FreeMind from http://freemind.sourceforge.net -->"
    )?;
    write_node(tree, tree.root, &mut out, 0)?;
    writeln!(out, "</map>")?;
    Ok(out)
}

fn write_node(tree: &MindmapTree, node_id: usize, out: &mut String, indent: usize) -> Result<()> {
    let node = &tree.nodes[node_id];

    // Skip deleted nodes (empty text, no parent, no children)
    if node.text.is_empty() && node.parent.is_none() && node_id != tree.root {
        return Ok(());
    }

    let pad = "  ".repeat(indent);

    write!(out, "{pad}<node")?;

    if let Some(ref bg) = node.background_color {
        write!(
            out,
            " BACKGROUND_COLOR=\"#{:02x}{:02x}{:02x}\"",
            bg.r(),
            bg.g(),
            bg.b()
        )?;
    }

    if let Some(ref c) = node.color {
        write!(
            out,
            " COLOR=\"#{:02x}{:02x}{:02x}\"",
            c.r(),
            c.g(),
            c.b()
        )?;
    }

    if let Some(ts) = node.created {
        write!(out, " CREATED=\"{ts}\"")?;
    }

    if node.folded {
        write!(out, " FOLDED=\"true\"")?;
    }

    write!(out, " ID=\"{}\"", node.freemind_id)?;

    if let Some(ts) = node.modified {
        write!(out, " MODIFIED=\"{ts}\"")?;
    }

    if let Some(ref pos) = node.position {
        let pos_str = match pos {
            Side::Left => "left",
            Side::Right => "right",
        };
        write!(out, " POSITION=\"{pos_str}\"")?;
    }

    // Escape XML special chars in text
    let escaped_text = xml_escape(&node.text);
    write!(out, " TEXT=\"{escaped_text}\"")?;

    let has_children = !node.children.is_empty();
    let has_font = node.bold || node.font_size.is_some() || node.font_name.is_some();

    if !has_children && !has_font {
        writeln!(out, "/>")?;
    } else {
        writeln!(out, ">")?;

        if has_font {
            write!(out, "{pad}  <font")?;
            if node.bold {
                write!(out, " BOLD=\"true\"")?;
            }
            if let Some(ref name) = node.font_name {
                write!(out, " NAME=\"{name}\"")?;
            }
            if let Some(size) = node.font_size {
                write!(out, " SIZE=\"{size}\"")?;
            }
            writeln!(out, "/>")?;
        }

        for &child_id in &node.children {
            write_node(tree, child_id, out, indent + 1)?;
        }

        writeln!(out, "{pad}</node>")?;
    }

    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
