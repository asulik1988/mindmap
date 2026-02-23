//! Faithful Rust port of roughjs drawing algorithms for use with egui.
//!
//! Ports the PRNG from `math.ts` and the core rendering functions from
//! `renderer.ts`: `_line`, `_doubleLine`, `_bezierTo`, `_curve`,
//! `_curveWithOffset`, and `svgPath` segment handling.
//!
//! All bezier ops are sampled at high resolution (20 points per cubic
//! segment) so the result looks smooth when rendered as egui `PathShape::line`.

use egui::{Pos2, Rect};

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Configuration matching roughjs `ResolvedOptions` (subset we need).
pub struct RoughOptions {
    pub roughness: f32,
    pub bowing: f32,
    pub max_randomness_offset: f32,
    /// Catmull-Rom tightness for `_curve`. 0.0 = standard Catmull-Rom.
    pub curve_tightness: f32,
    pub disable_multi_stroke: bool,
    pub preserve_vertices: bool,
}

impl Default for RoughOptions {
    fn default() -> Self {
        Self {
            roughness: 1.0,
            bowing: 1.0,
            max_randomness_offset: 2.0,
            curve_tightness: 0.0,
            disable_multi_stroke: false,
            preserve_vertices: false,
        }
    }
}

// ---------------------------------------------------------------------------
// PRNG — faithful port of roughjs math.ts Random class
// ---------------------------------------------------------------------------

/// roughjs LCG PRNG: `(2^31 - 1) & (seed = imul(48271, seed)) / 2^31`
struct Rng {
    seed: i32,
}

impl Rng {
    fn new(seed: u32) -> Self {
        // roughjs stores seed as a JS number; we use i32 to match Math.imul
        // semantics (signed 32-bit multiply then mask with 2^31-1).
        Self { seed: seed as i32 }
    }

    /// Returns a value in (0, 1) matching roughjs `Random.next()`.
    fn next(&mut self) -> f32 {
        // Math.imul(48271, seed) — wrapping signed 32-bit multiply
        self.seed = (48271_i32).wrapping_mul(self.seed);
        // (2^31 - 1) & seed
        let masked = self.seed & 0x7FFF_FFFF;
        masked as f32 / 2_147_483_648.0 // 2^31
    }
}

// ---------------------------------------------------------------------------
// Drawing op intermediate representation
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum Op {
    Move(f32, f32),
    LineTo(f32, f32),
    BcurveTo(f32, f32, f32, f32, f32, f32), // cp1x,cp1y, cp2x,cp2y, x,y
}

// ---------------------------------------------------------------------------
// Private helpers — direct ports from renderer.ts
// ---------------------------------------------------------------------------

/// `_offset(min, max, ops, roughnessGain)` from renderer.ts
fn _offset(min: f32, max: f32, rng: &mut Rng, roughness: f32, roughness_gain: f32) -> f32 {
    roughness * roughness_gain * (rng.next() * (max - min) + min)
}

/// `_offsetOpt(x, ops, roughnessGain)` — random in [-x, x] scaled by roughness
fn _offset_opt(x: f32, rng: &mut Rng, roughness: f32, roughness_gain: f32) -> f32 {
    _offset(-x, x, rng, roughness, roughness_gain)
}

/// `_line()` from renderer.ts — converts a line segment into ops (move + bcurveTo).
#[allow(clippy::too_many_arguments)]
fn _line(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    rng: &mut Rng,
    o: &RoughOptions,
    do_move: bool,
    overlay: bool,
) -> Vec<Op> {
    let length_sq = (x1 - x2).powi(2) + (y1 - y2).powi(2);
    let length = length_sq.sqrt();

    let roughness_gain = if length < 200.0 {
        1.0
    } else if length > 500.0 {
        0.4
    } else {
        (-0.001_666_8) * length + 1.233_334
    };

    let mut offset = o.max_randomness_offset;
    if offset * offset * 100.0 > length_sq {
        offset = length / 10.0;
    }
    let half_offset = offset / 2.0;

    let diverge_point = 0.2 + rng.next() * 0.2;

    let mut mid_disp_x = o.bowing * o.max_randomness_offset * (y2 - y1) / 200.0;
    let mut mid_disp_y = o.bowing * o.max_randomness_offset * (x1 - x2) / 200.0;
    mid_disp_x = _offset_opt(mid_disp_x, rng, o.roughness, roughness_gain);
    mid_disp_y = _offset_opt(mid_disp_y, rng, o.roughness, roughness_gain);

    let mut ops = Vec::new();

    let random_half = |rng: &mut Rng| _offset_opt(half_offset, rng, o.roughness, roughness_gain);
    let random_full = |rng: &mut Rng| _offset_opt(offset, rng, o.roughness, roughness_gain);

    if do_move {
        if overlay {
            ops.push(Op::Move(
                x1 + if o.preserve_vertices {
                    0.0
                } else {
                    random_half(rng)
                },
                y1 + if o.preserve_vertices {
                    0.0
                } else {
                    random_half(rng)
                },
            ));
        } else {
            ops.push(Op::Move(
                x1 + if o.preserve_vertices {
                    0.0
                } else {
                    _offset_opt(offset, rng, o.roughness, roughness_gain)
                },
                y1 + if o.preserve_vertices {
                    0.0
                } else {
                    _offset_opt(offset, rng, o.roughness, roughness_gain)
                },
            ));
        }
    }

    if overlay {
        ops.push(Op::BcurveTo(
            mid_disp_x + x1 + (x2 - x1) * diverge_point + random_half(rng),
            mid_disp_y + y1 + (y2 - y1) * diverge_point + random_half(rng),
            mid_disp_x + x1 + 2.0 * (x2 - x1) * diverge_point + random_half(rng),
            mid_disp_y + y1 + 2.0 * (y2 - y1) * diverge_point + random_half(rng),
            x2 + if o.preserve_vertices {
                0.0
            } else {
                random_half(rng)
            },
            y2 + if o.preserve_vertices {
                0.0
            } else {
                random_half(rng)
            },
        ));
    } else {
        ops.push(Op::BcurveTo(
            mid_disp_x + x1 + (x2 - x1) * diverge_point + random_full(rng),
            mid_disp_y + y1 + (y2 - y1) * diverge_point + random_full(rng),
            mid_disp_x + x1 + 2.0 * (x2 - x1) * diverge_point + random_full(rng),
            mid_disp_y + y1 + 2.0 * (y2 - y1) * diverge_point + random_full(rng),
            x2 + if o.preserve_vertices {
                0.0
            } else {
                random_full(rng)
            },
            y2 + if o.preserve_vertices {
                0.0
            } else {
                random_full(rng)
            },
        ));
    }

    ops
}

/// `_doubleLine()` from renderer.ts — primary stroke + optional overlay.
fn _double_line(x1: f32, y1: f32, x2: f32, y2: f32, rng: &mut Rng, o: &RoughOptions) -> Vec<Op> {
    let o1 = _line(x1, y1, x2, y2, rng, o, true, false);
    if o.disable_multi_stroke {
        return o1;
    }
    let o2 = _line(x1, y1, x2, y2, rng, o, true, true);
    [o1, o2].concat()
}

/// `_bezierTo()` from renderer.ts — wobble a cubic bezier.
#[allow(clippy::too_many_arguments)]
fn _bezier_to(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    x: f32,
    y: f32,
    current: (f32, f32),
    rng: &mut Rng,
    o: &RoughOptions,
) -> Vec<Op> {
    let mut ops = Vec::new();
    let ros = [
        o.max_randomness_offset.max(1.0),
        o.max_randomness_offset.max(1.0) + 0.3,
    ];
    let iterations = if o.disable_multi_stroke { 1 } else { 2 };

    for i in 0..iterations {
        if i == 0 {
            ops.push(Op::Move(current.0, current.1));
        } else {
            ops.push(Op::Move(
                current.0
                    + if o.preserve_vertices {
                        0.0
                    } else {
                        _offset_opt(ros[0], rng, o.roughness, 1.0)
                    },
                current.1
                    + if o.preserve_vertices {
                        0.0
                    } else {
                        _offset_opt(ros[0], rng, o.roughness, 1.0)
                    },
            ));
        }

        let f = if o.preserve_vertices {
            (x, y)
        } else {
            (
                x + _offset_opt(ros[i], rng, o.roughness, 1.0),
                y + _offset_opt(ros[i], rng, o.roughness, 1.0),
            )
        };

        ops.push(Op::BcurveTo(
            x1 + _offset_opt(ros[i], rng, o.roughness, 1.0),
            y1 + _offset_opt(ros[i], rng, o.roughness, 1.0),
            x2 + _offset_opt(ros[i], rng, o.roughness, 1.0),
            y2 + _offset_opt(ros[i], rng, o.roughness, 1.0),
            f.0,
            f.1,
        ));
    }

    ops
}

/// `_curve()` from renderer.ts — Catmull-Rom to cubic bezier conversion.
fn _curve(points: &[(f32, f32)], rng: &mut Rng, o: &RoughOptions) -> Vec<Op> {
    let len = points.len();
    let mut ops = Vec::new();

    if len > 3 {
        let s = 1.0 - o.curve_tightness;
        ops.push(Op::Move(points[1].0, points[1].1));

        let mut i = 1;
        while i + 2 < len {
            let cached = points[i];
            let b1 = (
                cached.0 + (s * points[i + 1].0 - s * points[i - 1].0) / 6.0,
                cached.1 + (s * points[i + 1].1 - s * points[i - 1].1) / 6.0,
            );
            let b2 = (
                points[i + 1].0 + (s * points[i].0 - s * points[i + 2].0) / 6.0,
                points[i + 1].1 + (s * points[i].1 - s * points[i + 2].1) / 6.0,
            );
            let b3 = (points[i + 1].0, points[i + 1].1);
            ops.push(Op::BcurveTo(b1.0, b1.1, b2.0, b2.1, b3.0, b3.1));
            i += 1;
        }
    } else if len == 3 {
        ops.push(Op::Move(points[1].0, points[1].1));
        ops.push(Op::BcurveTo(
            points[1].0,
            points[1].1,
            points[2].0,
            points[2].1,
            points[2].0,
            points[2].1,
        ));
    } else if len == 2 {
        ops.extend(_line(
            points[0].0,
            points[0].1,
            points[1].0,
            points[1].1,
            rng,
            o,
            true,
            true,
        ));
    }

    ops
}

/// `_curveWithOffset()` from renderer.ts — adds random offsets then calls `_curve`.
fn _curve_with_offset(
    points: &[(f32, f32)],
    offset: f32,
    rng: &mut Rng,
    o: &RoughOptions,
) -> Vec<Op> {
    if points.is_empty() {
        return Vec::new();
    }

    let mut ps: Vec<(f32, f32)> = Vec::new();

    // Duplicate first point (two copies at the front, each with independent offsets)
    ps.push((
        points[0].0 + _offset_opt(offset, rng, o.roughness, 1.0),
        points[0].1 + _offset_opt(offset, rng, o.roughness, 1.0),
    ));
    ps.push((
        points[0].0 + _offset_opt(offset, rng, o.roughness, 1.0),
        points[0].1 + _offset_opt(offset, rng, o.roughness, 1.0),
    ));

    for i in 1..points.len() {
        ps.push((
            points[i].0 + _offset_opt(offset, rng, o.roughness, 1.0),
            points[i].1 + _offset_opt(offset, rng, o.roughness, 1.0),
        ));
        // Duplicate last point
        if i == points.len() - 1 {
            ps.push((
                points[i].0 + _offset_opt(offset, rng, o.roughness, 1.0),
                points[i].1 + _offset_opt(offset, rng, o.roughness, 1.0),
            ));
        }
    }

    _curve(&ps, rng, o)
}

// ---------------------------------------------------------------------------
// SVG-path-like rendering for rounded rectangles
// ---------------------------------------------------------------------------

/// Segment types parsed from a rounded-rect SVG path (M, L, C, Q, Z).
/// We convert quadratic beziers (Q) to cubic (C) immediately.
enum PathSeg {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

/// Build the SVG-like path segments for a rounded rectangle, matching the
/// Excalidraw approach: `M ... L ... Q ... L ... Q ...` etc.
///
/// Quadratic corners are promoted to cubic beziers so `_bezierTo` can handle
/// them uniformly.
fn rounded_rect_segments(rect: Rect, rounding: f32) -> Vec<PathSeg> {
    let r = rounding
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);

    let l = rect.min.x;
    let t = rect.min.y;
    let ri = rect.max.x;
    let b = rect.max.y;

    if r <= 0.0 {
        // Sharp corners — simple rectangle
        return vec![
            PathSeg::Move(l, t),
            PathSeg::Line(ri, t),
            PathSeg::Line(ri, b),
            PathSeg::Line(l, b),
            PathSeg::Close,
        ];
    }

    // Excalidraw rounded rect path:
    //   M (l+r, t)
    //   L (ri-r, t)   Q (ri, t, ri, t+r)
    //   L (ri, b-r)   Q (ri, b, ri-r, b)
    //   L (l+r, b)    Q (l, b, l, b-r)
    //   L (l, t+r)    Q (l, t, l+r, t)
    //   Z
    //
    // Quadratic bezier Q(cpx,cpy, x,y) from current point is promoted to
    // cubic: C(cp1x,cp1y, cp2x,cp2y, x,y) where
    //   cp1 = current + 2/3 * (Q_cp - current)
    //   cp2 = end     + 2/3 * (Q_cp - end)

    vec![
        PathSeg::Move(l + r, t),
        // Top edge -> top-right corner
        PathSeg::Line(ri - r, t),
        quad_to_cubic(ri - r, t, ri, t, ri, t + r),
        // Right edge -> bottom-right corner
        PathSeg::Line(ri, b - r),
        quad_to_cubic(ri, b - r, ri, b, ri - r, b),
        // Bottom edge -> bottom-left corner
        PathSeg::Line(l + r, b),
        quad_to_cubic(l + r, b, l, b, l, b - r),
        // Left edge -> top-left corner
        PathSeg::Line(l, t + r),
        quad_to_cubic(l, t + r, l, t, l + r, t),
        PathSeg::Close,
    ]
}

/// Convert quadratic bezier (from, Q_cp, to) into a cubic PathSeg.
fn quad_to_cubic(from_x: f32, from_y: f32, cpx: f32, cpy: f32, to_x: f32, to_y: f32) -> PathSeg {
    let cp1x = from_x + 2.0 / 3.0 * (cpx - from_x);
    let cp1y = from_y + 2.0 / 3.0 * (cpy - from_y);
    let cp2x = to_x + 2.0 / 3.0 * (cpx - to_x);
    let cp2y = to_y + 2.0 / 3.0 * (cpy - to_y);
    PathSeg::Cubic(cp1x, cp1y, cp2x, cp2y, to_x, to_y)
}

/// Render SVG-like path segments through roughjs algorithms, producing ops.
/// This is the direct port of `svgPath()` from renderer.ts.
fn render_path_segments(segs: &[PathSeg], rng: &mut Rng, o: &RoughOptions) -> Vec<Op> {
    let mut ops = Vec::new();
    let mut first = (0.0_f32, 0.0_f32);
    let mut current = (0.0_f32, 0.0_f32);

    for seg in segs {
        match seg {
            PathSeg::Move(x, y) => {
                current = (*x, *y);
                first = (*x, *y);
            }
            PathSeg::Line(x, y) => {
                ops.extend(_double_line(current.0, current.1, *x, *y, rng, o));
                current = (*x, *y);
            }
            PathSeg::Cubic(x1, y1, x2, y2, x, y) => {
                ops.extend(_bezier_to(*x1, *y1, *x2, *y2, *x, *y, current, rng, o));
                current = (*x, *y);
            }
            PathSeg::Close => {
                ops.extend(_double_line(current.0, current.1, first.0, first.1, rng, o));
                current = first;
            }
        }
    }

    ops
}

// ---------------------------------------------------------------------------
// Ops -> egui Pos2 conversion with high-resolution bezier sampling
// ---------------------------------------------------------------------------

const BEZIER_SAMPLES: usize = 20;

/// Evaluate a cubic bezier at parameter `t`.
#[allow(clippy::too_many_arguments)]
fn cubic_bezier_point(
    p0x: f32,
    p0y: f32,
    p1x: f32,
    p1y: f32,
    p2x: f32,
    p2y: f32,
    p3x: f32,
    p3y: f32,
    t: f32,
) -> (f32, f32) {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;
    let t2 = t * t;
    let t3 = t2 * t;
    (
        mt3 * p0x + 3.0 * mt2 * t * p1x + 3.0 * mt * t2 * p2x + t3 * p3x,
        mt3 * p0y + 3.0 * mt2 * t * p1y + 3.0 * mt * t2 * p2y + t3 * p3y,
    )
}

/// Convert a sequence of `Op`s into separate polyline paths (each `Vec<Pos2>`).
///
/// Every `Move` op starts a new path. `BcurveTo` ops are sampled at
/// `BEZIER_SAMPLES` points per segment. `LineTo` is a single point.
fn ops_to_paths(ops: &[Op]) -> Vec<Vec<Pos2>> {
    let mut paths: Vec<Vec<Pos2>> = Vec::new();
    let mut current_path: Vec<Pos2> = Vec::new();
    let mut cx = 0.0_f32;
    let mut cy = 0.0_f32;

    for op in ops {
        match op {
            Op::Move(x, y) => {
                // Flush previous path if non-empty
                if !current_path.is_empty() {
                    paths.push(std::mem::take(&mut current_path));
                }
                current_path.push(Pos2::new(*x, *y));
                cx = *x;
                cy = *y;
            }
            Op::LineTo(x, y) => {
                current_path.push(Pos2::new(*x, *y));
                cx = *x;
                cy = *y;
            }
            Op::BcurveTo(cp1x, cp1y, cp2x, cp2y, x, y) => {
                // Sample the cubic bezier at high resolution, skipping t=0
                // (already in the path as the previous point).
                for i in 1..=BEZIER_SAMPLES {
                    let t = i as f32 / BEZIER_SAMPLES as f32;
                    let (px, py) =
                        cubic_bezier_point(cx, cy, *cp1x, *cp1y, *cp2x, *cp2y, *x, *y, t);
                    current_path.push(Pos2::new(px, py));
                }
                cx = *x;
                cy = *y;
            }
        }
    }

    if !current_path.is_empty() {
        paths.push(current_path);
    }

    paths
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate a roughjs-style hand-drawn line between two points.
///
/// Returns multiple paths (polylines) suitable for `PathShape::line`.
pub fn rough_line(from: Pos2, to: Pos2, seed: u32, options: &RoughOptions) -> Vec<Vec<Pos2>> {
    let mut rng = Rng::new(seed);
    let ops = _double_line(from.x, from.y, to.x, to.y, &mut rng, options);
    ops_to_paths(&ops)
}

/// Generate a roughjs-style hand-drawn rounded rectangle.
///
/// Returns multiple paths (primary stroke + overlay stroke when multi-stroke
/// is enabled). Each inner `Vec<Pos2>` is a polyline suitable for
/// `PathShape::line`.
///
/// The path is constructed as an SVG-like rounded-rect (M, L, Q corners
/// promoted to C) and then rendered through roughjs's `_doubleLine` /
/// `_bezierTo` algorithms, exactly matching roughjs `svgPath()` behavior.
pub fn rough_rounded_rect(
    rect: Rect,
    rounding: f32,
    seed: u32,
    options: &RoughOptions,
) -> Vec<Vec<Pos2>> {
    let segs = rounded_rect_segments(rect, rounding);

    // --- Primary stroke ---
    let mut rng = Rng::new(seed);
    let primary_ops = render_path_segments(&segs, &mut rng, options);

    // --- Overlay stroke (seed + 1, matching roughjs cloneOptionsAlterSeed) ---
    if options.disable_multi_stroke {
        // Even with multi-stroke disabled, _doubleLine and _bezierTo already
        // produce single strokes internally, so we just return the one set.
        return ops_to_paths(&primary_ops);
    }

    // The primary ops already include both passes from _doubleLine / _bezierTo
    // (they internally do 2 iterations). The roughjs svgPath function returns
    // all ops concatenated, so ops_to_paths naturally separates them at each
    // Move op into distinct polylines.
    ops_to_paths(&primary_ops)
}

/// Generate a roughjs-style hand-drawn bezier edge (for parent-child connections).
///
/// Uses `_curveWithOffset` (Catmull-Rom with random displacement), matching
/// roughjs `curve()` behavior. Returns multiple paths (primary + overlay).
pub fn rough_bezier_edge(
    src: Pos2,
    cp1: Pos2,
    cp2: Pos2,
    tgt: Pos2,
    seed: u32,
    options: &RoughOptions,
) -> Vec<Vec<Pos2>> {
    // Build the input point list. roughjs `curve()` takes a flat list of
    // points. For a cubic bezier we sample the *clean* curve into enough
    // intermediate points so Catmull-Rom has material to work with.
    let n = 10; // intermediate sample count for the base curve
    let points: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            cubic_bezier_point(src.x, src.y, cp1.x, cp1.y, cp2.x, cp2.y, tgt.x, tgt.y, t)
        })
        .collect();

    let mut rng1 = Rng::new(seed);

    // Primary pass: offset = 1 * (1 + roughness * 0.2)
    let offset1 = 1.0 * (1.0 + options.roughness * 0.2);
    let o1 = _curve_with_offset(&points, offset1, &mut rng1, options);

    // Overlay pass: offset = 1.5 * (1 + roughness * 0.22), seed + 1
    let mut paths = ops_to_paths(&o1);

    if !options.disable_multi_stroke {
        let mut rng2 = Rng::new(seed + 1);
        let offset2 = 1.5 * (1.0 + options.roughness * 0.22);
        let o2 = _curve_with_offset(&points, offset2, &mut rng2, options);
        paths.extend(ops_to_paths(&o2));
    }

    paths
}

// ---------------------------------------------------------------------------
// Hachure (hatch) fill — ported from roughjs hachure-fill package
// ---------------------------------------------------------------------------

/// Rotate a point around the origin by `angle` radians.
fn rotate_point(x: f32, y: f32, angle: f32) -> (f32, f32) {
    let cos = angle.cos();
    let sin = angle.sin();
    (x * cos - y * sin, x * sin + y * cos)
}

/// Generate hachure (parallel diagonal) fill lines for a rectangle.
///
/// Uses the roughjs approach: rotate polygon so hatch direction becomes
/// horizontal, run scan-line intersection, rotate back, then draw each
/// line through `_double_line` for hand-drawn wobble.
pub fn hachure_fill_rect(
    rect: Rect,
    angle_degrees: f32,
    gap: f32,
    seed: u32,
    options: &RoughOptions,
) -> Vec<Vec<Pos2>> {
    if gap < 1.0 {
        return Vec::new();
    }

    // Skip rects with non-finite coordinates (can happen with extreme layout sizes)
    if !rect.min.x.is_finite()
        || !rect.min.y.is_finite()
        || !rect.max.x.is_finite()
        || !rect.max.y.is_finite()
    {
        return Vec::new();
    }

    // Rectangle corners
    let corners = [
        (rect.min.x, rect.min.y),
        (rect.max.x, rect.min.y),
        (rect.max.x, rect.max.y),
        (rect.min.x, rect.max.y),
    ];

    // Rotation angle: roughjs adds 90° to convert user angle to scan direction
    let rot = (angle_degrees + 90.0).to_radians();

    // Rotate corners
    let rotated: Vec<(f32, f32)> = corners
        .iter()
        .map(|&(x, y)| rotate_point(x, y, rot))
        .collect();

    // Find bounding box of rotated polygon
    let ymin = rotated.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    let ymax = rotated
        .iter()
        .map(|p| p.1)
        .fold(f32::NEG_INFINITY, f32::max);

    // Build edge table from rotated polygon (4 edges, closing the loop)
    struct Edge {
        ymin: f32,
        ymax: f32,
        x_at_ymin: f32,
        islope: f32, // dx/dy
    }

    let mut edges = Vec::new();
    for i in 0..4 {
        let p1 = rotated[i];
        let p2 = rotated[(i + 1) % 4];
        if (p1.1 - p2.1).abs() < 0.001 {
            continue; // skip horizontal edges
        }
        let (ymin_e, ymax_e, x_at_ymin) = if p1.1 < p2.1 {
            (p1.1, p2.1, p1.0)
        } else {
            (p2.1, p1.1, p2.0)
        };
        let islope = (p2.0 - p1.0) / (p2.1 - p1.1);
        edges.push(Edge {
            ymin: ymin_e,
            ymax: ymax_e,
            x_at_ymin,
            islope,
        });
    }
    edges.sort_by(|a, b| a.ymin.total_cmp(&b.ymin));

    // Scan-line: collect horizontal line segments
    // Use a separate rng for gap jitter so it doesn't affect line drawing rng
    let mut gap_rng = Rng::new(seed.wrapping_add(33333));
    let mut line_segments: Vec<((f32, f32), (f32, f32))> = Vec::new();
    let mut y = ymin + gap / 2.0; // offset slightly so lines don't sit on edges

    while y < ymax {
        // Jitter the scan-line Y position for organic spacing (±35% of gap)
        let jitter = gap_rng.next() * 0.7 - 0.35; // [-0.35, +0.35]
        let scan_y = y + jitter * gap;

        // Find x-intersections with active edges
        let mut xs = Vec::new();
        for edge in &edges {
            if scan_y >= edge.ymin && scan_y < edge.ymax {
                let x = edge.x_at_ymin + (scan_y - edge.ymin) * edge.islope;
                xs.push(x);
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Emit lines between pairs
        let mut i = 0;
        while i + 1 < xs.len() {
            line_segments.push(((xs[i], scan_y), (xs[i + 1], scan_y)));
            i += 2;
        }

        y += gap;
    }

    // Rotate line endpoints back to original coordinate space
    let neg_rot = -rot;
    let unrotated_lines: Vec<((f32, f32), (f32, f32))> = line_segments
        .iter()
        .map(|&(p1, p2)| {
            (
                rotate_point(p1.0, p1.1, neg_rot),
                rotate_point(p2.0, p2.1, neg_rot),
            )
        })
        .collect();

    // Draw each line through roughjs _double_line for hand-drawn wobble
    let mut rng = Rng::new(seed);
    let mut all_ops = Vec::new();
    for (p1, p2) in &unrotated_lines {
        let ops = if options.disable_multi_stroke {
            _line(p1.0, p1.1, p2.0, p2.1, &mut rng, options, true, false)
        } else {
            _double_line(p1.0, p1.1, p2.0, p2.1, &mut rng, options)
        };
        all_ops.extend(ops);
    }

    ops_to_paths(&all_ops)
}
