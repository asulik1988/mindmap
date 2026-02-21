use super::svg::export_svg;
use crate::model::MindmapTree;
use crate::style::colors::DepthColorConfig;

pub fn export_png(tree: &MindmapTree, color_config: &DepthColorConfig) -> Option<Vec<u8>> {
    let svg_str = export_svg(tree, color_config);
    let opt = resvg::usvg::Options::default();
    let fontdb = resvg::usvg::fontdb::Database::new();
    let rtree = resvg::usvg::Tree::from_str(&svg_str, &opt, &fontdb).ok()?;
    let size = rtree.size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width() as u32, size.height() as u32)?;
    resvg::render(
        &rtree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().ok()
}
