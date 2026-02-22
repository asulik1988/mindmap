use egui::Color32;
use std::collections::HashMap;

pub struct NodePalette {
    pub fill: Color32,
    pub stroke: Color32,
    pub text: Color32,
    pub stroke_width: f32,
}

/// Configurable depth-to-color mapping. Stores per-depth overrides into DEPTH_FILLS.
pub struct DepthColorConfig {
    overrides: HashMap<usize, usize>,
    /// Temporary preview override (depth_mod8, palette_index). Takes highest precedence.
    preview: Option<(usize, usize)>,
}

impl DepthColorConfig {
    pub fn new() -> Self {
        Self {
            overrides: HashMap::new(),
            preview: None,
        }
    }

    /// Returns the DEPTH_FILLS index for a given depth.
    pub fn get_fill_index(&self, depth: usize) -> usize {
        // Preview takes highest priority
        if let Some((preview_depth, preview_idx)) = self.preview {
            if depth % 8 == preview_depth {
                return preview_idx % DEPTH_FILL_COUNT;
            }
        }
        if let Some(&idx) = self.overrides.get(&(depth % 8)) {
            idx % DEPTH_FILL_COUNT
        } else {
            depth % DEPTH_FILL_COUNT
        }
    }

    pub fn set_preview(&mut self, preview: Option<(usize, usize)>) {
        self.preview = preview;
    }

    pub fn set_fill_index(&mut self, depth: usize, idx: usize) {
        self.overrides.insert(depth % 8, idx % DEPTH_FILL_COUNT);
    }

    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }

    pub fn has_overrides(&self) -> bool {
        !self.overrides.is_empty()
    }
}

impl Default for DepthColorConfig {
    fn default() -> Self {
        Self::new()
    }
}

pub const DEPTH_FILL_COUNT: usize = 40;

/// Near-black stroke matching Excalidraw's default (#1e1e1e)
const STROKE_COLOR: Color32 = Color32::from_rgb(30, 30, 30);
/// Uniform text color
const TEXT_COLOR: Color32 = Color32::from_rgb(30, 30, 30);

/// 40-color palette for depth-based node coloring.
///
/// Organized as 8 hue families x 5 shades, interleaved so adjacent depths
/// always jump to a contrasting hue. Each color is tuned as a hachure fill
/// (diagonal lines over white background on warm beige canvas #FBFBFA).
///
/// Hue families (in interleave order):
///   Yellow/Gold, Sky Blue, Rose/Pink, Green/Sage,
///   Orange/Peach, Violet/Purple, Teal/Cyan, Lavender/Periwinkle
///
/// Within each cycle of 8, the shade index advances, giving 5 full cycles
/// (40 distinct colors) before recycling.
const DEPTH_FILLS: [(u8, u8, u8); 40] = [
    // ── Cycle 0 (ultra-light, barely tinted) ────────────────────
    (255, 250, 214), //  0  Cream            #FFFAD6
    (214, 236, 255), //  1  Ice Blue         #D6ECFF
    (255, 224, 228), //  2  Blush            #FFE0E4
    (220, 245, 222), //  3  Mint             #DCF5DE
    (255, 234, 210), //  4  Peach Cream      #FFEAD2
    (234, 222, 248), //  5  Pale Lilac       #EADEF8
    (210, 242, 238), //  6  Frost            #D2F2EE
    (226, 228, 248), //  7  Mist             #E2E4F8
    // ── Cycle 1 (light pastel) ──────────────────────────────────
    (255, 244, 168), //  8  Yellow/Gold      #FFF4A8
    (164, 216, 255), //  9  Sky Blue         #A4D8FF
    (255, 186, 194), // 10  Rose/Pink        #FFBAC2
    (176, 232, 181), // 11  Green/Sage       #B0E8B5
    (255, 207, 158), // 12  Orange/Peach     #FFCF9E
    (208, 181, 241), // 13  Violet/Purple    #D0B5F1
    (150, 226, 218), // 14  Teal/Cyan        #96E2DA
    (190, 194, 241), // 15  Lavender         #BEC2F1
    // ── Cycle 2 (slightly deeper) ────────────────────────────────
    (247, 227, 131), // 16  Gold             #F7E383
    (134, 199, 247), // 17  Blue             #86C7F7
    (247, 160, 170), // 18  Pink             #F7A0AA
    (148, 218, 155), // 19  Green            #94DA9B
    (247, 186, 126), // 20  Peach            #F7BA7E
    (187, 155, 227), // 21  Purple           #BB9BE3
    (120, 210, 200), // 22  Teal             #78D2C8
    (168, 173, 227), // 23  Periwinkle       #A8ADE3
    // ── Cycle 3 (medium saturation) ──────────────────────────────
    (240, 212, 100), // 24  Mustard          #F0D464
    (108, 182, 240), // 25  Cornflower       #6CB6F0
    (240, 138, 150), // 26  Coral            #F08A96
    (122, 204, 132), // 27  Fern             #7ACC84
    (240, 168, 100), // 28  Tangerine        #F0A864
    (168, 132, 214), // 29  Iris             #A884D6
    (94, 196, 184),  // 30  Jade             #5EC4B8
    (148, 154, 214), // 31  Wisteria         #949AD6
    // ── Cycle 4 (richest, most saturated) ────────────────────────
    (230, 196, 72),  // 32  Amber            #E6C448
    (82, 164, 230),  // 33  Azure            #52A4E6
    (230, 114, 128), // 34  Raspberry        #E67280
    (96, 190, 108),  // 35  Clover           #60BE6C
    (230, 148, 72),  // 36  Marigold         #E69448
    (148, 108, 200), // 37  Amethyst         #946CC8
    (68, 180, 166),  // 38  Verdigris        #44B4A6
    (128, 134, 200), // 39  Slate Blue       #8086C8
];

pub fn node_palette(depth: usize, config: &DepthColorConfig) -> NodePalette {
    let idx = config.get_fill_index(depth);
    let (r, g, b) = DEPTH_FILLS[idx];
    NodePalette {
        fill: Color32::from_rgb(r, g, b),
        stroke: STROKE_COLOR,
        text: TEXT_COLOR,
        stroke_width: 1.5,
    }
}

/// Get a fill color by palette index (for drawing swatches in the UI).
pub fn depth_fill_color(idx: usize) -> Color32 {
    let (r, g, b) = DEPTH_FILLS[idx % DEPTH_FILL_COUNT];
    Color32::from_rgb(r, g, b)
}

pub fn font_size_for_depth(depth: usize) -> f32 {
    match depth {
        0 => 22.0,
        1 => 18.0,
        2 => 16.0,
        3 => 15.0,
        _ => 14.0,
    }
}

pub fn canvas_bg(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(28, 28, 30)
    } else {
        egui::Color32::from_rgb(251, 251, 250)
    }
}
pub fn grid_dot_color(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgba_premultiplied(100, 100, 110, 40)
    } else {
        egui::Color32::from_rgba_premultiplied(51, 50, 47, 20)
    }
}
pub fn panel_bg(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(38, 38, 42)
    } else {
        egui::Color32::from_rgb(251, 251, 250)
    }
}
pub fn ui_text(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(220, 220, 220)
    } else {
        egui::Color32::from_rgb(30, 30, 30)
    }
}
pub fn ui_text_muted(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(130, 130, 130)
    } else {
        egui::Color32::from_rgb(140, 135, 125)
    }
}
pub fn edge_color(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(160, 155, 145)
    } else {
        egui::Color32::from_rgb(30, 30, 30)
    }
}
pub fn hover_bg(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(55, 55, 62)
    } else {
        egui::Color32::from_rgb(240, 237, 232)
    }
}
pub fn selected_bg(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(50, 50, 58)
    } else {
        egui::Color32::from_rgb(235, 232, 227)
    }
}
pub fn divider_color(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(60, 60, 68)
    } else {
        egui::Color32::from_rgb(224, 221, 216)
    }
}
pub fn border_color(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(80, 80, 90)
    } else {
        egui::Color32::from_rgb(30, 30, 30)
    }
}

// Dark palette: desaturated, darkened versions of the light palette fills
const DEPTH_FILLS_DARK: [(u8, u8, u8); 40] = [
    (55, 52, 38),  //  0  Cream dark
    (30, 50, 75),  //  1  Ice Blue dark
    (75, 40, 45),  //  2  Blush dark
    (35, 65, 38),  //  3  Mint dark
    (75, 55, 35),  //  4  Peach Cream dark
    (55, 40, 80),  //  5  Pale Lilac dark
    (30, 68, 62),  //  6  Frost dark
    (38, 40, 80),  //  7  Mist dark
    (80, 72, 28),  //  8  Yellow dark
    (25, 65, 100), //  9  Sky Blue dark
    (100, 48, 55), // 10  Rose dark
    (40, 80, 45),  // 11  Sage dark
    (100, 70, 32), // 12  Peach dark
    (65, 45, 105), // 13  Purple dark
    (28, 90, 82),  // 14  Teal dark
    (48, 52, 105), // 15  Lavender dark
    (90, 78, 22),  // 16  Gold dark
    (22, 72, 110), // 17  Blue dark
    (110, 42, 52), // 18  Pink dark
    (30, 95, 38),  // 19  Green dark
    (110, 62, 22), // 20  Peach dark
    (68, 32, 112), // 21  Purple dark
    (18, 88, 78),  // 22  Teal dark
    (42, 48, 112), // 23  Periwinkle dark
    (105, 85, 10), // 24  Mustard dark
    (15, 60, 112), // 25  Cornflower dark
    (115, 28, 42), // 26  Coral dark
    (15, 100, 28), // 27  Fern dark
    (115, 48, 10), // 28  Tangerine dark
    (52, 18, 98),  // 29  Iris dark
    (8, 82, 70),   // 30  Jade dark
    (28, 35, 100), // 31  Wisteria dark
    (95, 75, 0),   // 32  Amber dark
    (8, 52, 105),  // 33  Azure dark
    (120, 15, 28), // 34  Raspberry dark
    (10, 85, 22),  // 35  Clover dark
    (120, 38, 0),  // 36  Marigold dark
    (42, 0, 85),   // 37  Amethyst dark
    (0, 68, 58),   // 38  Verdigris dark
    (18, 22, 88),  // 39  Slate Blue dark
];

pub fn node_palette_dark(depth: usize, config: &DepthColorConfig) -> NodePalette {
    let idx = config.get_fill_index(depth);
    let (r, g, b) = DEPTH_FILLS_DARK[idx];
    NodePalette {
        fill: egui::Color32::from_rgb(r, g, b),
        stroke: egui::Color32::from_rgb(120, 118, 115),
        text: egui::Color32::from_rgb(210, 210, 210),
        stroke_width: 1.5,
    }
}

pub fn node_palette_themed(
    depth: usize,
    dark_mode: bool,
    config: &DepthColorConfig,
) -> NodePalette {
    if dark_mode {
        node_palette_dark(depth, config)
    } else {
        node_palette(depth, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cycling() {
        let config = DepthColorConfig::new();
        assert_eq!(config.get_fill_index(0), 0);
        assert_eq!(config.get_fill_index(5), 5);
        assert_eq!(config.get_fill_index(40), 0); // wraps at 40
        assert_eq!(config.get_fill_index(41), 1);
    }

    #[test]
    fn override_takes_precedence() {
        let mut config = DepthColorConfig::new();
        config.set_fill_index(0, 10); // depth%8==0 maps to index 10
        assert_eq!(config.get_fill_index(0), 10);
        assert_eq!(config.get_fill_index(8), 10); // same depth%8
        assert_eq!(config.get_fill_index(1), 1); // unaffected
    }

    #[test]
    fn preview_takes_precedence_over_override() {
        let mut config = DepthColorConfig::new();
        config.set_fill_index(0, 10);
        config.set_preview(Some((0, 20)));
        assert_eq!(config.get_fill_index(0), 20);
        assert_eq!(config.get_fill_index(8), 20); // same depth%8
        config.set_preview(None);
        assert_eq!(config.get_fill_index(0), 10); // back to override
    }

    #[test]
    fn font_size_tiers() {
        assert_eq!(font_size_for_depth(0), 22.0);
        assert_eq!(font_size_for_depth(1), 18.0);
        assert_eq!(font_size_for_depth(2), 16.0);
        assert_eq!(font_size_for_depth(3), 15.0);
        assert_eq!(font_size_for_depth(4), 14.0);
        assert_eq!(font_size_for_depth(100), 14.0); // deep depths use 14
    }

    #[test]
    fn reset_clears_overrides() {
        let mut config = DepthColorConfig::new();
        config.set_fill_index(0, 10);
        assert!(config.has_overrides());
        config.reset_all();
        assert!(!config.has_overrides());
        assert_eq!(config.get_fill_index(0), 0);
    }
}
