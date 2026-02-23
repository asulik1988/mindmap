use crate::model::{MindmapNode, MindmapTree, Side};
use anyhow::{Context, Result};
use egui::Color32;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::borrow::Cow;
use std::path::Path;

fn parse_hex_color(s: &str) -> Option<Color32> {
    let s = s.trim_start_matches('#');
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(Color32::from_rgb(r, g, b))
    } else {
        None
    }
}

/// Helper: decode an XML attribute value into a Cow<str>, unescaping entities.
/// Returns None if the attribute is not present.
#[inline]
fn attr_str<'a>(e: &'a quick_xml::events::BytesStart<'a>, name: &[u8]) -> Option<Cow<'a, str>> {
    e.try_get_attribute(name)
        .ok()
        .flatten()
        .and_then(|a| a.unescape_value().ok())
}

/// Load a FreeMind .mm file from disk. The SAX parser is iterative (not
/// recursive), so no large stack is needed -- unlike the old serde approach
/// which could overflow at ~12K nesting depth.
#[allow(dead_code)]
pub fn load_mm_file(path: &Path) -> Result<MindmapTree> {
    let xml = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    parse_mm_xml(&xml)
}

/// Parse FreeMind .mm XML into a MindmapTree using SAX-style event parsing.
///
/// This iterates XML events with quick_xml::Reader, pushing nodes directly
/// into the arena Vec<MindmapNode> without building an intermediate tree.
/// For a 1M-node file this is ~5-10x faster than the serde deserialization
/// approach because it avoids millions of intermediate struct allocations.
pub fn parse_mm_xml(xml: &str) -> Result<MindmapTree> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // Pre-allocate with a rough estimate. FreeMind nodes average ~80 bytes of
    // XML each, so byte_len / 80 is a reasonable upper-bound guess.
    let estimated_nodes = xml.len() / 80;
    let mut nodes: Vec<MindmapNode> = Vec::with_capacity(estimated_nodes);

    // Stack of arena indices tracking the current ancestor chain.
    // When we see <node>, we push the new node's index.
    // When we see </node>, we pop.
    let mut ancestor_stack: Vec<usize> = Vec::new();
    let mut root_id: Option<usize> = None;

    // State for richcontent/note parsing.
    // FreeMind notes look like:
    //   <richcontent TYPE="NOTE"><html><head/><body>
    //     <p>Line one</p>
    //     <p>Line two</p>
    //   </body></html></richcontent>
    let mut in_richcontent_note = false;
    let mut in_body = false;
    let mut in_p = false;
    let mut note_paragraphs: Vec<String> = Vec::new();
    // The arena index of the node that owns the current richcontent.
    let mut note_target_node: Option<usize> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag_name = e.name();
                let local = tag_name.as_ref();

                if in_richcontent_note {
                    // Inside a NOTE richcontent, track nested tags.
                    match local {
                        b"body" => in_body = true,
                        b"p" if in_body => in_p = true,
                        b"richcontent" => {} // nested richcontent (unlikely but safe)
                        _ => {}
                    }
                    continue;
                }

                match local {
                    b"node" => {
                        let parent = ancestor_stack.last().copied();
                        let node_id = nodes.len();

                        let node = parse_node_from_attrs(e, node_id, parent);
                        nodes.push(node);

                        // Register as child of parent.
                        if let Some(parent_idx) = parent {
                            nodes[parent_idx].children.push(node_id);
                        }

                        if root_id.is_none() {
                            root_id = Some(node_id);
                        }

                        ancestor_stack.push(node_id);
                    }
                    b"font" => {
                        // <font> as a start tag (rare, usually empty).
                        // Apply font attributes to the current node.
                        if let Some(&node_idx) = ancestor_stack.last() {
                            apply_font_attrs(e, &mut nodes[node_idx]);
                        }
                    }
                    b"richcontent" => {
                        let rc_type = attr_str(e, b"TYPE");
                        if rc_type.as_deref() == Some("NOTE") {
                            in_richcontent_note = true;
                            in_body = false;
                            in_p = false;
                            note_paragraphs.clear();
                            note_target_node = ancestor_stack.last().copied();
                        }
                    }
                    _ => {} // <map>, <html>, <head>, etc. -- skip
                }
            }

            Ok(Event::Empty(ref e)) => {
                let tag_name = e.name();
                let local = tag_name.as_ref();

                if in_richcontent_note {
                    // Self-closing tags inside richcontent (e.g. <head/>).
                    continue;
                }

                match local {
                    b"node" => {
                        // Self-closing <node ... /> -- a leaf node.
                        let parent = ancestor_stack.last().copied();
                        let node_id = nodes.len();

                        let node = parse_node_from_attrs(e, node_id, parent);
                        nodes.push(node);

                        if let Some(parent_idx) = parent {
                            nodes[parent_idx].children.push(node_id);
                        }

                        if root_id.is_none() {
                            root_id = Some(node_id);
                        }
                        // Do NOT push to ancestor_stack -- it's self-closing.
                    }
                    b"font" => {
                        if let Some(&node_idx) = ancestor_stack.last() {
                            apply_font_attrs(e, &mut nodes[node_idx]);
                        }
                    }
                    _ => {}
                }
            }

            Ok(Event::End(ref e)) => {
                let tag_name = e.name();
                let local = tag_name.as_ref();

                if in_richcontent_note {
                    match local {
                        b"p" => {
                            in_p = false;
                        }
                        b"body" => {
                            in_body = false;
                        }
                        b"richcontent" => {
                            // This is the closing </richcontent> for our NOTE block.
                            // Assemble note text and attach to the node.
                            if let Some(node_idx) = note_target_node.take() {
                                nodes[node_idx].notes = note_paragraphs
                                    .iter()
                                    .filter(|s| !s.is_empty())
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join("\n");
                            }
                            in_richcontent_note = false;
                            in_body = false;
                            in_p = false;
                            note_paragraphs.clear();
                        }
                        _ => {}
                    }
                    continue;
                }

                if local == b"node" {
                    ancestor_stack.pop();
                }
            }

            Ok(Event::Text(ref e)) => {
                if in_richcontent_note && in_body && in_p {
                    // Text inside <p>...</p> within a NOTE richcontent.
                    if let Ok(text) = e.unescape() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            note_paragraphs.push(trimmed.to_string());
                        }
                    }
                }
            }

            Ok(Event::Eof) => break,

            Ok(_) => {} // Comments, CData, PI, Decl -- skip

            Err(e) => {
                return Err(anyhow::anyhow!(
                    "XML parse error at position {}: {}",
                    reader.error_position(),
                    e
                ));
            }
        }
    }

    let root = root_id.context("No root <node> found in FreeMind XML")?;
    Ok(MindmapTree::new(nodes, root))
}

/// Extract all relevant attributes from a <node> start/empty tag and build
/// a MindmapNode directly. This avoids any intermediate struct.
#[inline]
fn parse_node_from_attrs(
    e: &quick_xml::events::BytesStart<'_>,
    node_id: usize,
    parent: Option<usize>,
) -> MindmapNode {
    // Collect attributes in a single pass over the attribute bytes.
    let mut text: Option<Cow<'_, str>> = None;
    let mut id: Option<Cow<'_, str>> = None;
    let mut color: Option<Color32> = None;
    let mut background_color: Option<Color32> = None;
    let mut position: Option<Side> = None;
    let mut folded = false;
    let mut created: Option<u64> = None;
    let mut modified: Option<u64> = None;
    let mut link: Option<String> = None;

    for attr_result in e.attributes().with_checks(false) {
        if let Ok(attr) = attr_result {
            match attr.key.as_ref() {
                b"TEXT" => {
                    text = attr.unescape_value().ok();
                }
                b"ID" => {
                    id = attr.unescape_value().ok();
                }
                b"COLOR" => {
                    if let Ok(val) = attr.unescape_value() {
                        color = parse_hex_color(&val);
                    }
                }
                b"BACKGROUND_COLOR" => {
                    if let Ok(val) = attr.unescape_value() {
                        background_color = parse_hex_color(&val);
                    }
                }
                b"POSITION" => {
                    if let Ok(val) = attr.unescape_value() {
                        position = Some(if val.as_ref() == "left" {
                            Side::Left
                        } else {
                            Side::Right
                        });
                    }
                }
                b"FOLDED" => {
                    if let Ok(val) = attr.unescape_value() {
                        folded = val.as_ref() == "true";
                    }
                }
                b"CREATED" => {
                    if let Ok(val) = attr.unescape_value() {
                        created = val.parse::<u64>().ok();
                    }
                }
                b"MODIFIED" => {
                    if let Ok(val) = attr.unescape_value() {
                        modified = val.parse::<u64>().ok();
                    }
                }
                b"LINK" => {
                    if let Ok(val) = attr.unescape_value() {
                        link = Some(val.into_owned());
                    }
                }
                _ => {} // Ignore unknown attributes
            }
        }
    }

    let freemind_id = id
        .map(|c| c.into_owned())
        .unwrap_or_else(|| format!("ID_{}", node_id));
    let node_text = text.map(|c| c.into_owned()).unwrap_or_default();

    let mut node = MindmapNode::new(node_id, freemind_id, node_text);
    node.parent = parent;
    node.color = color;
    node.background_color = background_color;
    node.position = position;
    node.folded = folded;
    node.created = created;
    node.modified = modified;
    node.link = link;

    node
}

/// Apply <font> attributes to an existing node.
#[inline]
fn apply_font_attrs(e: &quick_xml::events::BytesStart<'_>, node: &mut MindmapNode) {
    for attr_result in e.attributes().with_checks(false) {
        if let Ok(attr) = attr_result {
            match attr.key.as_ref() {
                b"BOLD" => {
                    if let Ok(val) = attr.unescape_value() {
                        node.bold = val.as_ref() == "true";
                    }
                }
                b"SIZE" => {
                    if let Ok(val) = attr.unescape_value() {
                        node.font_size = val.parse::<f32>().ok();
                    }
                }
                b"NAME" => {
                    if let Ok(val) = attr.unescape_value() {
                        node.font_name = Some(val.into_owned());
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_mm() {
        let xml = r#"<map version="1.0.1">
            <node TEXT="Root" ID="ID_1">
                <node TEXT="Child1" ID="ID_2" POSITION="right"/>
                <node TEXT="Child2" ID="ID_3" POSITION="left"/>
            </node>
        </map>"#;
        let tree = parse_mm_xml(xml).unwrap();
        assert_eq!(tree.nodes[tree.root].text, "Root");
        assert_eq!(tree.nodes[tree.root].children.len(), 2);
        let c1 = tree.nodes[tree.root].children[0];
        let c2 = tree.nodes[tree.root].children[1];
        assert_eq!(tree.nodes[c1].text, "Child1");
        assert_eq!(tree.nodes[c1].position, Some(Side::Right));
        assert_eq!(tree.nodes[c2].text, "Child2");
        assert_eq!(tree.nodes[c2].position, Some(Side::Left));
    }

    #[test]
    fn parse_attributes() {
        let xml = r##"<map version="1.0.1">
            <node TEXT="Root" ID="ID_1" COLOR="#ff0000" BACKGROUND_COLOR="#00ff00"
                  FOLDED="true" CREATED="1000" MODIFIED="2000" LINK="https://example.com">
                <font BOLD="true" NAME="Arial" SIZE="18"/>
            </node>
        </map>"##;
        let tree = parse_mm_xml(xml).unwrap();
        let root = &tree.nodes[tree.root];
        assert_eq!(root.color, Some(Color32::from_rgb(255, 0, 0)));
        assert_eq!(root.background_color, Some(Color32::from_rgb(0, 255, 0)));
        assert!(root.folded);
        assert_eq!(root.created, Some(1000));
        assert_eq!(root.modified, Some(2000));
        assert_eq!(root.link, Some("https://example.com".to_string()));
        assert!(root.bold);
        assert_eq!(root.font_size, Some(18.0));
        assert_eq!(root.font_name, Some("Arial".to_string()));
    }

    #[test]
    fn parse_notes() {
        let xml = r#"<map version="1.0.1">
            <node TEXT="Root" ID="ID_1">
                <richcontent TYPE="NOTE"><html><head></head><body>
                    <p>Line one</p>
                    <p>Line two</p>
                </body></html></richcontent>
            </node>
        </map>"#;
        let tree = parse_mm_xml(xml).unwrap();
        assert_eq!(tree.nodes[tree.root].notes, "Line one\nLine two");
    }

    #[test]
    fn parse_xml_entities_in_text() {
        let xml = r#"<map version="1.0.1">
            <node TEXT="A &amp; B &lt; C &gt; D" ID="ID_1"/>
        </map>"#;
        let tree = parse_mm_xml(xml).unwrap();
        assert_eq!(tree.nodes[tree.root].text, "A & B < C > D");
    }

    /// Ad-hoc stress test: parse the 1M-node file if it exists and print timing.
    /// Run with: cargo test --release bench_1m_parse -- --nocapture --ignored
    #[test]
    #[ignore]
    fn bench_1m_parse() {
        let path = std::path::Path::new("stress-test-1m.mm");
        if !path.exists() {
            eprintln!("Skipping: stress-test-1m.mm not found");
            return;
        }

        let t0 = std::time::Instant::now();
        let xml = std::fs::read_to_string(path).unwrap();
        let t_read = t0.elapsed();
        eprintln!(
            "  read_to_string: {:.3}s ({:.1} MB)",
            t_read.as_secs_f64(),
            xml.len() as f64 / 1_048_576.0
        );

        let t1 = std::time::Instant::now();
        let tree = parse_mm_xml(&xml).unwrap();
        let t_parse = t1.elapsed();
        eprintln!(
            "  parse_mm_xml (SAX + MindmapTree::new): {:.3}s ({} nodes)",
            t_parse.as_secs_f64(),
            tree.nodes.len()
        );

        eprintln!("  total: {:.3}s", (t_read + t_parse).as_secs_f64());

        // Sanity checks
        assert!(tree.nodes.len() > 900_000, "Expected ~1M nodes");
        assert_eq!(tree.nodes[tree.root].text, "StressRoot");
    }
}
