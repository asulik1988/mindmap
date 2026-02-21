use egui::Color32;

pub struct NodePalette {
    pub fill: Color32,
    pub stroke: Color32,
    pub text: Color32,
    pub stroke_width: f32,
}

pub const CANVAS_BG: Color32 = Color32::from_rgb(251, 251, 250); // #FBFBFA

pub fn node_palette(depth: usize) -> NodePalette {
    match depth {
        0 => NodePalette {
            fill: Color32::from_rgb(255, 243, 224),   // #FFF3E0
            stroke: Color32::from_rgb(230, 81, 0),     // #E65100
            text: Color32::from_rgb(191, 54, 12),      // #BF360C
            stroke_width: 2.5,
        },
        1 => NodePalette {
            fill: Color32::from_rgb(232, 245, 233),   // #E8F5E9
            stroke: Color32::from_rgb(46, 125, 50),    // #2E7D32
            text: Color32::from_rgb(27, 94, 32),       // #1B5E20
            stroke_width: 2.0,
        },
        2 => NodePalette {
            fill: Color32::from_rgb(227, 242, 253),   // #E3F2FD
            stroke: Color32::from_rgb(21, 101, 192),   // #1565C0
            text: Color32::from_rgb(13, 71, 161),      // #0D47A1
            stroke_width: 1.5,
        },
        3 => NodePalette {
            fill: Color32::from_rgb(243, 229, 245),   // #F3E5F5
            stroke: Color32::from_rgb(106, 27, 154),   // #6A1B9A
            text: Color32::from_rgb(74, 20, 140),      // #4A148C
            stroke_width: 1.5,
        },
        _ => NodePalette {
            fill: Color32::from_rgb(250, 250, 250),   // #FAFAFA
            stroke: Color32::from_rgb(117, 117, 117),  // #757575
            text: Color32::from_rgb(66, 66, 66),       // #424242
            stroke_width: 1.0,
        },
    }
}

pub fn font_size_for_depth(depth: usize) -> f32 {
    match depth {
        0 => 20.0,
        1 => 16.0,
        2 => 14.0,
        3 => 13.0,
        _ => 12.0,
    }
}
