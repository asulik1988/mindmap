use crate::model::{MindmapNode, MindmapTree, Side};
use anyhow::{Context, Result};
use egui::Color32;
use serde::Deserialize;
use std::path::Path;

// Serde model matching FreeMind .mm XML schema

#[derive(Debug, Deserialize)]
#[serde(rename = "map")]
struct FreeMindMap {
    #[serde(rename = "@version")]
    #[allow(dead_code)]
    version: Option<String>,
    node: FreeMindNode,
}

#[derive(Debug, Deserialize)]
struct FreeMindNode {
    #[serde(rename = "@TEXT", default)]
    text: String,
    #[serde(rename = "@ID", default)]
    id: Option<String>,
    #[serde(rename = "@COLOR", default)]
    color: Option<String>,
    #[serde(rename = "@BACKGROUND_COLOR", default)]
    background_color: Option<String>,
    #[serde(rename = "@POSITION", default)]
    position: Option<String>,
    #[serde(rename = "@FOLDED", default)]
    folded: Option<String>,
    #[serde(rename = "@CREATED", default)]
    created: Option<String>,
    #[serde(rename = "@MODIFIED", default)]
    modified: Option<String>,
    #[serde(rename = "@LINK", default)]
    link: Option<String>,
    #[serde(rename = "node", default)]
    children: Vec<FreeMindNode>,
    #[serde(rename = "font", default)]
    font: Option<FreeMindFont>,
    #[serde(rename = "richcontent", default)]
    richcontent: Vec<FreeMindRichcontent>,
}

#[derive(Debug, Deserialize)]
struct FreeMindFont {
    #[serde(rename = "@BOLD", default)]
    bold: Option<String>,
    #[serde(rename = "@NAME", default)]
    name: Option<String>,
    #[serde(rename = "@SIZE", default)]
    size: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FreeMindRichPara {
    #[serde(rename = "$text", default)]
    text: String,
}

#[derive(Debug, Default, Deserialize)]
struct FreeMindRichBody {
    #[serde(rename = "p", default)]
    paragraphs: Vec<FreeMindRichPara>,
}

#[derive(Debug, Default, Deserialize)]
struct FreeMindRichHtml {
    #[serde(rename = "body", default)]
    body: Option<FreeMindRichBody>,
}

#[derive(Debug, Default, Deserialize)]
struct FreeMindRichcontent {
    #[serde(rename = "@TYPE", default)]
    r#type: String,
    #[serde(rename = "html", default)]
    html: Option<FreeMindRichHtml>,
}

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

pub fn load_mm_file(path: &Path) -> Result<MindmapTree> {
    let xml = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    parse_mm_xml(&xml)
}

pub fn parse_mm_xml(xml: &str) -> Result<MindmapTree> {
    let map: FreeMindMap = quick_xml::de::from_str(xml).context("Failed to parse FreeMind XML")?;

    let mut nodes = Vec::new();
    let root_id = convert_node(&map.node, None, &mut nodes);
    Ok(MindmapTree::new(nodes, root_id))
}

fn convert_node(
    fm_node: &FreeMindNode,
    parent: Option<usize>,
    nodes: &mut Vec<MindmapNode>,
) -> usize {
    let id = nodes.len();
    let freemind_id = fm_node.id.clone().unwrap_or_else(|| format!("ID_{}", id));

    let mut node = MindmapNode::new(id, freemind_id, fm_node.text.clone());
    node.parent = parent;
    node.color = fm_node.color.as_deref().and_then(parse_hex_color);
    node.background_color = fm_node
        .background_color
        .as_deref()
        .and_then(parse_hex_color);
    node.position = fm_node.position.as_deref().map(|p| match p {
        "left" => Side::Left,
        _ => Side::Right,
    });
    node.folded = fm_node.folded.as_deref() == Some("true");
    node.created = fm_node
        .created
        .as_deref()
        .and_then(|s| s.parse::<u64>().ok());
    node.modified = fm_node
        .modified
        .as_deref()
        .and_then(|s| s.parse::<u64>().ok());

    if let Some(ref font) = fm_node.font {
        node.bold = font.bold.as_deref() == Some("true");
        node.font_size = font.size.as_deref().and_then(|s| s.parse::<f32>().ok());
        node.font_name = font.name.clone();
    }

    node.link = fm_node.link.clone();

    node.notes = fm_node
        .richcontent
        .iter()
        .find(|rc| rc.r#type == "NOTE")
        .and_then(|rc| rc.html.as_ref())
        .and_then(|h| h.body.as_ref())
        .map(|body| {
            body.paragraphs
                .iter()
                .map(|p| p.text.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    // Push node first (reserves the index), then process children
    nodes.push(node);

    let child_ids: Vec<usize> = fm_node
        .children
        .iter()
        .map(|child| convert_node(child, Some(id), nodes))
        .collect();

    nodes[id].children = child_ids;
    id
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
}
