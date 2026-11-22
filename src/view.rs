// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! Drawing the page: ANSI backgrounds, the glow (baked once into small images), the blocks as exact rectangles with
//! their colour ramp, selection and search matches, the text, and the CRT look over it all.
use crate::core::{self, Cfg, Rgb, Theme};
use crate::page::{Cell, Page};
use crate::smooth;
use eframe::egui::{self, text::LayoutJob, Color32, FontId, Mesh, Pos2, Rect, Stroke, TextFormat, TextureHandle, TextureOptions, Vec2};

pub fn rgb(c: Rgb) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Where the grid lies on the screen.
#[derive(Clone, Copy)]
pub struct Grid {
    pub origin: Pos2, // top left of cell (0, 0)
    pub cw: f32,
    pub ch: f32,
    pub ppp: f32, // pixels per point: edges snap to whole pixels, so neighbouring blocks leave no seam
}

impl Grid {
    fn snap(&self, v: f32) -> f32 {
        (v * self.ppp).round() / self.ppp
    }
    pub fn x(&self, c: f32) -> f32 {
        self.snap(self.origin.x + c * self.cw)
    }
    pub fn y(&self, r: f32) -> f32 {
        self.snap(self.origin.y + r * self.ch)
    }
    pub fn cell_at(&self, pos: Pos2) -> Cell {
        (((pos.y - self.origin.y) / self.ch).max(0.) as usize, ((pos.x - self.origin.x) / self.cw).max(0.) as usize)
    }
    fn cells(&self, r: usize, a: usize, b: usize) -> Rect {
        Rect::from_min_max(Pos2::new(self.x(a as f32), self.y(r as f32)), Pos2::new(self.x(b as f32), self.y(r as f32 + 1.)))
    }
}

/// The images made once and reused every frame.
#[derive(Default)]
pub struct Cache {
    key: String,
    glow: Vec<(TextureHandle, f32, f32)>, // image, k (page points per image pixel), margin
    smooth: Option<(String, TextureHandle)>, // the smoothed blocks as the alpha of the whole page, at screen resolution
    text_key: String,
    text_glow: Vec<(TextureHandle, f32, f32, f32)>, // as glow, and how strong
    scanlines: Option<TextureHandle>,
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

/// The colour of a block at y (in rows from the top of the page): the ramp runs from the top to the bottom of its
/// group of art rows.
pub fn block_color(page: &Page, r: usize, y: f32, ramp: Option<&[Rgb; 3]>, art: Rgb) -> Rgb {
    match (ramp, page.group[r]) {
        (Some(ramp), Some((top, end))) => ramp_at(ramp, top, end, y),
        _ => art,
    }
}

fn ramp_at(ramp: &[Rgb; 3], top: usize, end: usize, y: f32) -> Rgb {
    let t = ((y - top as f32) / (end - top) as f32).clamp(0., 1.) * 2.;
    if t < 1. { mix(ramp[0], ramp[1], t) } else { mix(ramp[1], ramp[2], t - 1.) }
}

/// The blocks at 1/k size, every image pixel the average of what it covers: scaled back up smoothly, that is the
/// blur. A margin keeps a halo from being cut off at the edge of the art. None for a page too big for an image.
pub fn bake(page: &Page, cw: f32, ch: f32, k: f32, ramp: Option<&[Rgb; 3]>, art: Rgb) -> Option<(egui::ColorImage, f32)> {
    let m = (2. * k).ceil();
    let (w, h) = (((page.cols as f32 * cw + 2. * m) / k).ceil() as usize, ((page.lines.len() as f32 * ch + 2. * m) / k).ceil() as usize);
    if w == 0 || h == 0 || w * h > 8_000_000 || w.max(h) > 8192 {
        return None;
    }
    let mut sum = vec![[0f32; 4]; w * h];
    for (r, line) in page.lines.iter().enumerate() {
        for (c, &chr) in line.iter().enumerate() {
            let Some((rects, alpha)) = core::block(chr) else { continue };
            for q in rects {
                let col = block_color(page, r, r as f32 + (q[1] + q[3]) / 2., ramp, art);
                let (x0, x1) = ((m + (c as f32 + q[0]) * cw) / k, (m + (c as f32 + q[2]) * cw) / k);
                let (y0, y1) = ((m + (r as f32 + q[1]) * ch) / k, (m + (r as f32 + q[3]) * ch) / k);
                for py in y0 as usize..(y1.ceil() as usize).min(h) {
                    let cover_y = (y1.min(py as f32 + 1.) - y0.max(py as f32)).max(0.);
                    for px in x0 as usize..(x1.ceil() as usize).min(w) {
                        let a = cover_y * (x1.min(px as f32 + 1.) - x0.max(px as f32)).max(0.) * alpha;
                        let cell = &mut sum[py * w + px];
                        for i in 0..3 {
                            cell[i] += col[i] as f32 * a;
                        }
                        cell[3] += a * 255.;
                    }
                }
            }
        }
    }
    let pixels = sum.iter().map(|p| Color32::from_rgba_premultiplied(p[0].min(255.) as u8, p[1].min(255.) as u8, p[2].min(255.) as u8, p[3].min(255.) as u8));
    Some((egui::ColorImage::new([w, h], pixels.collect()), m))
}

/// The text of the whole page at 1/k size (k in points), every image pixel the average of what it covers, as `bake`
/// does for the blocks: the glyphs are the very ones the window shows, from egui's font image. Scaled up smoothly and
/// softened once more, that is the halo of the text glow. None for a page too big for an image.
pub fn bake_text(ctx: &egui::Context, page: &Page, theme: &Theme, font: &FontId, cw: f32, ch: f32, k: f32) -> Option<(egui::ColorImage, f32)> {
    let ppp = ctx.pixels_per_point();
    let m = (3. * k).ceil();
    let (w, h) = (((page.cols as f32 * cw + 2. * m) / k).ceil() as usize, ((page.lines.len() as f32 * ch + 2. * m) / k).ceil() as usize);
    if w == 0 || h == 0 || w * h > 8_000_000 || w.max(h) > 8192 {
        return None;
    }
    // all rows laid out first: a glyph is in the font image once it was laid out
    let galleys: Vec<_> = (0..page.lines.len()).map(|r| ctx.fonts_mut(|f| f.layout_job(row_text(page, r, theme, font)))).collect();
    let atlas = ctx.fonts(|f| f.image());
    let mut sum = vec![[0f32; 4]; w * h];
    let share = 1. / (k * ppp).powi(2); // of a small pixel, one pixel of the font image
    for (r, galley) in galleys.iter().enumerate() {
        for placed in &galley.rows {
            let known = placed.row.glyphs.len() == page.lines[r].len(); // one glyph per character: its colour is known
            for (i, glyph) in placed.row.glyphs.iter().enumerate().filter(|(_, g)| !g.uv_rect.is_nothing()) {
                let at = placed.pos + glyph.pos.to_vec2() + glyph.uv_rect.offset + Vec2::new(m, m + r as f32 * ch);
                let color = if known { ink(page, r, i, theme) } else { theme.text };
                let uv = glyph.uv_rect;
                for (j, ay) in (uv.min[1]..uv.max[1]).enumerate() {
                    let py = ((at.y + j as f32 / ppp) / k) as usize;
                    for (i, ax) in (uv.min[0]..uv.max[0]).enumerate() {
                        let px = ((at.x + i as f32 / ppp) / k) as usize;
                        let a = atlas.pixels[ay as usize * atlas.size[0] + ax as usize].a() as f32 / 255. * share;
                        if a > 0. && px < w && py < h {
                            let cell = &mut sum[py * w + px];
                            for c in 0..3 {
                                cell[c] += color[c] as f32 * a;
                            }
                            cell[3] += a * 255.;
                        }
                    }
                }
            }
        }
        for (a, b, _) in page.links.get(r).into_iter().flatten() {
            // the underline of a link glows too
            let (y, x0, x1) = ((m + (r as f32 + 0.92) * ch) / k, (m + *a as f32 * cw) / k, (m + *b as f32 * cw) / k);
            for px in x0 as usize..(x1 as usize).min(w) {
                if let Some(cell) = sum.get_mut(y as usize * w + px) {
                    for c in 0..3 {
                        cell[c] += theme.link[c] as f32 / k;
                    }
                    cell[3] += 255. / k;
                }
            }
        }
    }
    // softened once more with a 1 2 1 kernel each way: a halo closer to a real blur than the plain small image
    for (dx, dy) in [(1, 0), (0, w)] {
        let before = sum.clone();
        for (i, cell) in sum.iter_mut().enumerate() {
            let (x, y) = (i % w, i / w);
            let side = |d: isize| {
                let ok = if dx == 1 { (x as isize + d) >= 0 && ((x as isize + d) as usize) < w } else { (y as isize + d) >= 0 && ((y as isize + d) as usize) < h };
                if ok { before[(i as isize + d * (dx + dy) as isize) as usize] } else { [0.; 4] }
            };
            let (a, b) = (side(-1), side(1));
            for c in 0..4 {
                cell[c] = (a[c] + 2. * before[i][c] + b[c]) / 4.;
            }
        }
    }
    let pixels = sum.iter().map(|p| Color32::from_rgba_premultiplied(p[0].min(255.) as u8, p[1].min(255.) as u8, p[2].min(255.) as u8, p[3].min(255.) as u8));
    Some((egui::ColorImage::new([w, h], pixels.collect()), m))
}

/// The colour of the character at (r, c): ANSI art brings its own, links and art have the theme's.
pub fn ink(page: &Page, r: usize, c: usize, theme: &Theme) -> Rgb {
    match &page.colors {
        Some(colors) => colors[r][c].0,
        None if page.link_at(r, c).is_some() => theme.link,
        None if core::is_art(page.lines[r][c]) => theme.art,
        None => theme.text,
    }
}

/// The text of a row, ready to lay out: the blocks left out (they are drawn as rectangles), links underlined.
pub fn row_text(page: &Page, r: usize, theme: &Theme, font: &FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    let (mut run, mut format) = (String::new(), TextFormat::default());
    for (c, &chr) in page.lines[r].iter().enumerate() {
        let (fg, link) = (rgb(ink(page, r, c, theme)), page.colors.is_none() && page.link_at(r, c).is_some());
        let next = TextFormat { font_id: font.clone(), color: fg, underline: if link { Stroke::new(1., fg) } else { Stroke::NONE }, ..Default::default() };
        if (next.color != format.color || next.underline != format.underline) && !run.is_empty() {
            job.append(&std::mem::take(&mut run), 0., format);
        }
        format = next;
        run.push(if core::block(chr).is_some() { ' ' } else { chr });
    }
    job.append(&run, 0., format);
    job
}

/// What moves, for one frame.
pub struct Fx {
    pub t: f32,             // the animation clock, in seconds
    pub level: Option<f32>, // how loud the music is, 0..1 (None: no music)
    pub neon: Option<f32>,  // seconds into the neon flicker-on
    pub view: Rect,         // the visible part of the page on the screen
}

/// A steady pseudo-random number 0..1 from a few numbers (the same every frame).
pub fn hash(n: &[f64]) -> f32 {
    let sum: f64 = n.iter().zip([12.9898, 78.233, 37.719]).map(|(v, k)| v * k).sum();
    (sum.sin() * 43758.5453).rem_euclid(1.) as f32
}

/// How bright the neon sign is at t seconds (1 = fully): mostly steady, now and then a short stutter.
pub fn flicker(t: f32) -> f32 {
    let period = 3.3;
    let k = (t / period).floor() as f64;
    if hash(&[k]) < 0.4 {
        return 1.; // no stutter in this stretch
    }
    let local = t - k as f32 * period - hash(&[0., k]) * (period - 0.6);
    if (0. ..0.5).contains(&local) { [0.15, 0.6, 0.25, 0.85, 0.4, 0.7][(local * 12.) as usize] } else { 1. }
}

/// How bright one slice of the art is during the neon flicker-on: each has its own start; from there it stutters
/// between dark, dim and bright, more and more often bright, until it stays on.
pub fn neon_slice(i: usize, t: f32) -> f32 {
    let u = (t - (0.12 + 0.85 * hash(&[i as f64]))) / 0.55;
    let phase = (u * 9.).rem_euclid(1.);
    if u >= 1. || (u > 0. && phase < 0.25 + 0.7 * u) { 1. } else if u > 0. && phase < 0.6 + 0.4 * u { 0.35 } else { 0. }
}

/// The colour cycling: bands that move up (or down) through the art, endlessly.
pub struct Bands {
    colors: Vec<Rgb>,
    band: f32,  // height of one colour, in points
    hard: bool, // thin stripes with hard edges, like on the C64
    shift: f32,
    pad: f32, // the empty margin above the first row: as on Linux, the bands start at the top of the page
}

impl Bands {
    pub fn new(cfg: &Cfg, ramp: Option<&[Rgb; 3]>, ch: f32, t: f32) -> Self {
        let (colors, band, hard) = if cfg["cycle_palette"] == "ramp" {
            let d = core::defaults();
            let r = ramp.copied().unwrap_or([0, 1, 2].map(|i| core::hex(d["ramp_colors"][i].as_str().unwrap())));
            (vec![r[0], r[1], r[2], r[1]], ch * 2., false) // there and back: no hard edge where the pattern repeats
        } else {
            (core::C64_CYCLE.to_vec(), ch / 3., true)
        };
        let shift = t * ch * 2.5; // 2.5 rows a second
        let period = band * colors.len() as f32;
        Bands { shift: (if cfg["cycle_dir"] == "up" { shift } else { -shift }).rem_euclid(period), colors, band, hard, pad: 0. }
    }
    /// The bands run from the top of the page, not of the first row (NfoGlow for Linux): `ch` is the row height, the
    /// margin above the first row 1.5 rows.
    pub fn on_page(mut self, ch: f32) -> Self {
        self.pad = (ch * 1.5).round_ties_even();
        self
    }
    /// The colour at y (points from the first row).
    pub fn at(&self, y: f32) -> Rgb {
        let n = self.colors.len();
        let i = ((y + self.pad + self.shift) / self.band).rem_euclid(n as f32);
        let a = self.colors[i as usize % n];
        if self.hard { a } else { mix(a, self.colors[(i as usize + 1) % n], i.fract()) }
    }
    /// Where a colour ends between a and b (points): a..b cut into pieces of one band each.
    fn cuts(&self, a: f32, b: f32) -> Vec<f32> {
        let first = ((a + self.pad + self.shift) / self.band).floor() as i64 + 1;
        (first..).map(|k| k as f32 * self.band - self.shift - self.pad).take_while(|y| *y < b - 1e-3).filter(|y| *y > a + 1e-3).collect()
    }
}

#[cfg(test)]
impl Bands {
    /// (colours, period, hard, shift) as NfoGlow for Linux reports them (for the conformance test).
    pub(crate) fn parts(&self) -> (Vec<Rgb>, f32, bool, f32) {
        (self.colors.clone(), self.band * self.colors.len() as f32, self.hard, self.shift)
    }
}

/// What the glow draws, as (soft, [(k, alpha)]): for neon a wide and a tight blurred copy behind sharp blocks, for
/// soft edges one blurred copy instead of the blocks, or nothing. ANSI art is pixel art made for exact cells: it
/// never glows. k is the blur (page points per image pixel); the alpha moves with the pulse and the neon flicker.
pub fn glow_parts(page: &Page, cfg: &Cfg, cw: f32, fx: &Fx) -> (bool, Vec<(f32, f32)>) {
    if cfg["soft"] != true || page.colors.is_some() || page.lines.is_empty() {
        return (false, Vec::new());
    }
    let strength = cfg["glow"].as_i64().unwrap_or(3) as f32;
    if cfg["glow_mode"] == "halo" {
        let tight = (cw * strength / 6.).max(2.);
        let mut beat = 1.;
        if cfg["pulse"] == true {
            // with music: the loudness; without: a slow breath
            beat = match fx.level.filter(|_| cfg["pulse_mode"] == "beat") {
                Some(level) => 0.35 + 0.8 * level,
                None => 0.7 + 0.3 * (fx.t * 4.).sin(),
            };
        }
        if cfg["neon_flicker"] == true {
            beat *= flicker(fx.t);
        }
        return (false, vec![(tight * 3., (0.7 * beat).min(1.)), (tight, (0.8 * beat).min(1.))]);
    }
    (true, vec![((cw * strength / 9.).max(1.25), 1.)])
}

const WHITE: Rgb = [255; 3];

/// The size in screen pixels of the image of the whole page, None when it is too big for one image.
fn smooth_size(page: &Page, g: Grid) -> Option<(usize, usize)> {
    let (w, h) = ((page.cols as f32 * g.cw * g.ppp).round() as usize, (page.lines.len() as f32 * g.ch * g.ppp).round() as usize);
    (w > 0 && h > 0 && w * h <= 16_000_000 && w.max(h) <= 8192).then_some((w, h))
}

/// Makes the image of the smoothed blocks if it is not there yet. False when there is none (a page too big for an
/// image gets sharp edges, as NfoGlow for Linux does).
fn smooth_image(cache: &mut Cache, ctx: &egui::Context, page: &Page, page_id: u64, cfg: &Cfg, g: Grid) -> bool {
    let Some((w, h)) = smooth_size(page, g) else { return false };
    let amount = cfg["smooth_amount"].as_str().unwrap_or("mid");
    let key = format!("{page_id} {w} {h} {amount}");
    if cache.smooth.as_ref().is_none_or(|(k, _)| *k != key) {
        cache.smooth = smooth::alpha(page, w, h, smooth::amount(amount)).map(|a| {
            let pixels = a.iter().map(|&a| Color32::from_white_alpha(a)).collect();
            (key, ctx.load_texture("smooth", egui::ColorImage::new([w, h], pixels), TextureOptions::LINEAR))
        });
    }
    cache.smooth.is_some()
}

/// The smoothed blocks of rows lo..hi: the image in strips, each coloured with the ramp or the cycling colours and
/// each able to slide sideways (the wobble); during the neon flicker-on in upright slices as well.
#[allow(clippy::too_many_arguments)]
fn smooth_mesh(tex: &TextureHandle, page: &Page, cfg: &Cfg, theme: &Theme, g: Grid, lo: usize, hi: usize, ramp: Option<&[Rgb; 3]>, cycle: Option<&Bands>, sway: f32, dx: &dyn Fn(f32) -> f32, fx: &Fx, lit: &dyn Fn(f32) -> f32, slice_w: f32) -> Mesh {
    let _ = cfg;
    let rows = page.lines.len();
    let size = tex.size_vec2() / g.ppp;
    let rect = Rect::from_min_size(Pos2::new(g.x(0.), g.y(0.)), size);
    let y_of = |r: f32| rect.top() + r / rows as f32 * rect.height();
    let (top, bottom) = (y_of(lo as f32), y_of(hi.min(rows) as f32));
    let mut cuts: Vec<f32> = (lo..=hi.min(rows)).map(|r| y_of(r as f32)).collect();
    if let Some(bands) = cycle {
        cuts.extend(bands.cuts(top - g.origin.y, bottom - g.origin.y).iter().map(|y| y + g.origin.y));
    }
    if sway > 0. {
        cuts.extend((1..).map(|i| top + i as f32 * 2. / g.ppp).take_while(|y| *y < bottom));
    }
    cuts.sort_by(f32::total_cmp);
    // the colour at a height; the rows around a group take its colours too, for the rounded edge
    let color_at = |y: f32, mid: f32| -> Rgb {
        let rel = y - rect.top();
        if let Some(bands) = cycle {
            return bands.at(if bands.hard { mid - rect.top() } else { rel } );
        }
        let row = (((mid - rect.top()) / rect.height() * rows as f32) as usize).min(rows - 1);
        let near = |r: usize| page.group.get(r).copied().flatten();
        let group = near(row).or_else(|| near(row + 1)).or_else(|| row.checked_sub(1).and_then(near));
        match (ramp, group) {
            (Some(ramp), Some((a, b))) => ramp_at(ramp, a, b, rel / rect.height() * rows as f32),
            _ => theme.art,
        }
    };
    let mut slices = Vec::new();
    if fx.neon.is_some() {
        let mut x = rect.left();
        while x < rect.right() {
            slices.push((x, (x + slice_w).min(rect.right())));
            x = if x < g.origin.x { g.origin.x } else { x + slice_w };
        }
    } else {
        slices.push((rect.left(), rect.right()));
    }
    let uv = |p: Pos2| ((p - rect.min) / rect.size()).to_pos2();
    let mut mesh = Mesh::with_texture(tex.id());
    for piece in cuts.windows(2).filter(|p| p[1] > p[0]) {
        let mid = (piece[0] + piece[1]) / 2.;
        let shift = if sway > 0. { dx((mid - g.origin.y) / g.ch) } else { 0. };
        let (c0, c1) = (rgb(color_at(piece[0], mid)), rgb(color_at(piece[1], mid)));
        for &(x0, x1) in &slices {
            let alpha = lit(x0.max(g.origin.x));
            let i = mesh.vertices.len() as u32;
            for (pos, color) in [(Pos2::new(x0, piece[0]), c0), (Pos2::new(x1, piece[0]), c0), (Pos2::new(x1, piece[1]), c1), (Pos2::new(x0, piece[1]), c1)] {
                mesh.vertices.push(egui::epaint::Vertex { pos: pos + Vec2::new(shift, 0.), uv: uv(pos), color: color.gamma_multiply(alpha) });
            }
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i, i + 2, i + 3);
        }
    }
    mesh
}

/// Draws rows lo..hi of the page. page_id: changes with every loaded file (the cache key).
#[allow(clippy::too_many_arguments)]
pub fn paint(painter: &egui::Painter, cache: &mut Cache, page: &Page, page_id: u64, cfg: &Cfg, theme: &Theme, font: &FontId, g: Grid, lo: usize, hi: usize, plain: bool, fx: &Fx) {
    let ctx = painter.ctx();
    let ansi = page.colors.is_some();
    let on = |key: &str| cfg[key] == true && !ansi && !plain; // the effects work on NFOs; ANSI art keeps its own colours
    let ramp_colors: Option<[Rgb; 3]> = (cfg["ramp"] == true && !ansi)
        .then(|| cfg["ramp_colors"].as_array().and_then(|a| Some([core::hex(a.first()?.as_str()?), core::hex(a.get(1)?.as_str()?), core::hex(a.get(2)?.as_str()?)])))
        .flatten();
    let ramp = ramp_colors.as_ref();
    let cycle = on("cycle").then(|| Bands::new(cfg, ramp, g.ch, fx.t).on_page(g.ch));
    // the wobbling logo of the old demos: the art slides sideways on a sine wave that runs up through it
    let sway = if on("wobble") { g.ch * match cfg["wobble_amount"].as_str() { Some("low") => 0.15, Some("high") => 0.45, _ => 0.3 } } else { 0. };
    let dx = |y: f32| match page.group.get(y.max(0.) as usize) {
        Some(Some(_)) if sway > 0. => g.snap(sway * (y / 0.75 + fx.t * 4.).sin()),
        _ => 0.,
    };
    let slice_w = (g.cw * 4.).max(24.); // the neon flicker-on lights the art in slices of about half a letter
    let lit = |x: f32| fx.neon.map_or(1., |t| neon_slice(((x - g.origin.x) / slice_w).max(0.) as usize, t));

    // the glow images: made again only when the page, the size or a colour changes
    let (soft, parts) = if plain { (false, Vec::new()) } else { glow_parts(page, cfg, g.cw, fx) };
    let white = soft && cycle.is_some(); // a stencil that the cycling colours colour in
    let key = format!("{page_id} {} {} {:?} {ramp:?} {:?} {white}", g.cw, g.ch, parts.iter().map(|p| p.0).collect::<Vec<_>>(), theme.art);
    if cache.key != key {
        cache.glow.clear();
        for (i, &(k, _)) in parts.iter().enumerate() {
            if let Some((image, m)) = bake(page, g.cw, g.ch, k, ramp.filter(|_| !white), if white { WHITE } else { theme.art }) {
                cache.glow.push((ctx.load_texture(format!("glow{i}"), image, TextureOptions::LINEAR), k, m));
            }
        }
        cache.key = key;
    }
    let soft = soft && !cache.glow.is_empty(); // a page too big for the image gets sharp blocks instead
    // smooth edges: the blocks as one image of the page at the screen's resolution (made again only when the page or the
    // size changes), instead of the exact rectangles; with soft glow edges the blocks are blurred anyway
    let smooth = on("smooth") && !soft && smooth_image(cache, ctx, page, page_id, cfg, g);
    let mut under = Mesh::default(); // ANSI backgrounds
    let mut blocks = Mesh::default(); // plain quads have no soft edges: no seams between blocks
    let mut shine = Mesh::default(); // the gleam
    let mut marks = Mesh::default(); // selection and search matches
    let mut texts = Vec::new();
    let mut art_cells = Vec::new();
    // the gleam: now and then a slanted band of light sweeps across the art from left to right
    let sweep = (fx.t % 6.) / 1.4;
    let band = (5. * g.cw).max(36.);
    let reach = fx.view.height() * 0.35 + band;
    let (left, right) = (fx.view.left().max(g.x(0.)) - reach, fx.view.right().min(g.x(page.cols as f32)) + reach);
    let gleam = |p: Pos2| {
        // along the leaning band: 0 at its left edge, 1 at its right
        let s = ((p.x - (left + (right - left) * sweep - band / 2.)) * band - (p.y - fx.view.center().y - band * 0.2) * band * 0.4) / (band * band * 1.16);
        let stops = [(0., 0.), (0.42, 0.5), (0.5, 0.75), (0.58, 0.5), (1., 0.)];
        stops.windows(2).find(|w| (w[0].0..=w[1].0).contains(&s)).map_or(0., |w| w[0].1 + (w[1].1 - w[0].1) * (s - w[0].0) / (w[1].0 - w[0].0))
    };
    let sheen = on("sheen") && sweep < 1.;
    for r in lo..hi {
        let line = &page.lines[r];
        for (c, &chr) in line.iter().enumerate() {
            let fg = rgb(ink(page, r, c, theme));
            if let Some(bg) = page.colors.as_ref().map(|colors| colors[r][c].1).filter(|b| *b != core::VGA[0]) {
                under.add_colored_rect(g.cells(r, c, c + 1), rgb(bg));
            }
            let block = core::block(chr);
            if block.is_some() && chr != core::SQUARE {
                art_cells.push((r, c));
            }
            if let Some((rects, alpha)) = block.as_ref() {
                let (xa, w) = (g.x(c as f32), g.x(c as f32 + 1.) - g.x(c as f32));
                let alpha = alpha * lit(xa);
                for q in rects {
                    // shared whole-pixel edges with the neighbours
                    let (x0, x1) = (xa + g.snap(q[0] * w), xa + g.snap(q[2] * w));
                    let (top, bottom) = (r as f32 + q[1], r as f32 + q[3]);
                    if sheen {
                        let corners = [Pos2::new(x0, g.y(top)), Pos2::new(x1, g.y(top)), Pos2::new(x1, g.y(bottom)), Pos2::new(x0, g.y(bottom))];
                        let light = corners.map(gleam);
                        if light.iter().any(|a| *a > 0.) {
                            let i = shine.vertices.len() as u32;
                            for (pos, a) in corners.iter().zip(light) {
                                shine.colored_vertex(*pos + Vec2::new(dx(if pos.y < corners[2].y { top } else { bottom }), 0.), Color32::WHITE.gamma_multiply(a * alpha));
                            }
                            shine.add_triangle(i, i + 1, i + 2);
                            shine.add_triangle(i, i + 2, i + 3);
                        }
                    }
                    if soft || smooth || alpha <= 0. {
                        continue;
                    }
                    // cut where a colour band ends, and in the middle for the wave of the wobble
                    let mut cuts = vec![top];
                    if let Some(bands) = &cycle {
                        cuts.extend(bands.cuts(top * g.ch, bottom * g.ch).iter().map(|y| y / g.ch));
                    }
                    if sway > 0. {
                        // strips of 2 screen pixels, each shifted as a whole, as NfoGlow for Linux does
                        let step = 2. / (g.ppp * g.ch);
                        cuts.extend((1..).map(|i| top + i as f32 * step).take_while(|y| *y < bottom));
                        cuts.sort_by(f32::total_cmp);
                    }
                    cuts.push(bottom);
                    for piece in cuts.windows(2) {
                        let color = |y: f32| {
                            let c = match &cycle {
                                _ if ansi => return fg.gamma_multiply(alpha),
                                Some(bands) if bands.hard => bands.at((piece[0] + piece[1]) / 2. * g.ch),
                                Some(bands) => bands.at(y * g.ch),
                                None => block_color(page, r, y, ramp, theme.art),
                            };
                            rgb(c).gamma_multiply(alpha)
                        };
                        let i = blocks.vertices.len() as u32;
                        for (x, y) in [(x0, piece[0]), (x1, piece[0]), (x1, piece[1]), (x0, piece[1])] {
                            let shift = if sway > 0. { dx((piece[0] + piece[1]) / 2.) } else { 0. };
                            blocks.colored_vertex(Pos2::new(x + shift, g.y(y)), color(y));
                        }
                        blocks.add_triangle(i, i + 1, i + 2);
                        blocks.add_triangle(i, i + 2, i + 3);
                    }
                }
            }
        }
        texts.push((Pos2::new(g.x(0.), g.y(r as f32)), ctx.fonts_mut(|f| f.layout_job(row_text(page, r, theme, font)))));
    }
    for (r, a, b) in page.selection_rows(lo, hi, cfg["copy_text_only"] == true) {
        marks.add_colored_rect(g.cells(r, a, b), rgb(theme.link).gamma_multiply(0.35));
    }
    for (i, &(r, a, b)) in page.matches.iter().enumerate().filter(|(_, m)| (lo..hi).contains(&m.0)) {
        let current = i == page.current;
        marks.add_colored_rect(g.cells(r, a, b), Color32::from_rgb(255, if current { 140 } else { 215 }, 30).gamma_multiply(if current { 0.75 } else { 0.45 }));
    }
    painter.add(under);
    let clip = painter.clip_rect();
    for ((image, k, m), (_, alpha)) in cache.glow.iter().zip(&parts) {
        // the image in strips: each can slide (the wobble) and take a colour of its own (the cycling, through the
        // white stencil); during the neon flicker-on in upright slices instead
        let rect = Rect::from_min_size(Pos2::new(g.x(0.) - m, g.y(0.) - m), image.size_vec2() * *k);
        let mut mesh = Mesh::with_texture(image.id());
        let mut quad = |a: Pos2, b: Pos2, shift: f32, top: Color32, bottom: Color32| {
            let uv = |p: Pos2| ((p - rect.min) / rect.size()).to_pos2();
            let i = mesh.vertices.len() as u32;
            for (pos, color) in [(a, top), (Pos2::new(b.x, a.y), top), (b, bottom), (Pos2::new(a.x, b.y), bottom)] {
                mesh.vertices.push(egui::epaint::Vertex { pos: pos + Vec2::new(shift, 0.), uv: uv(pos), color });
            }
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i, i + 2, i + 3);
        };
        let (top, bottom) = (rect.top().max(clip.top()), rect.bottom().min(clip.bottom()));
        if fx.neon.is_some() {
            let mut x = rect.left();
            while x < rect.right() {
                let tint = Color32::WHITE.gamma_multiply(alpha * lit(x.max(g.origin.x)));
                quad(Pos2::new(x, top), Pos2::new((x + slice_w).min(rect.right()), bottom), 0., tint, tint);
                x = if x < g.origin.x { g.origin.x } else { x + slice_w };
            }
        } else {
            let bands = cycle.as_ref().filter(|_| white);
            let rows = |y: f32| (y - g.origin.y) / g.ch;
            let mut cuts = vec![top];
            if let Some(bands) = bands {
                cuts.extend(bands.cuts(top - g.origin.y, bottom - g.origin.y).iter().map(|y| y + g.origin.y));
            }
            if sway > 0. {
                // strips of 2 screen pixels, each shifted as a whole, as NfoGlow for Linux does
                cuts.extend((1..).map(|i| top + i as f32 * 2. / g.ppp).take_while(|y| *y < bottom));
                cuts.sort_by(f32::total_cmp);
            }
            cuts.push(bottom.max(top));
            for piece in cuts.windows(2) {
                let tint = |y: f32| bands.map_or(Color32::WHITE, |b| rgb(b.at(if b.hard { (piece[0] + piece[1]) / 2. } else { y } - g.origin.y))).gamma_multiply(*alpha);
                quad(Pos2::new(rect.left(), piece[0]), Pos2::new(rect.right(), piece[1]), dx(rows((piece[0] + piece[1]) / 2.)), tint(piece[0]), tint(piece[1]));
            }
        }
        painter.add(mesh);
    }
    if smooth {
        if let Some((_, tex)) = &cache.smooth {
            painter.add(smooth_mesh(tex, page, cfg, theme, g, lo, hi, ramp, cycle.as_ref(), sway, &dx, fx, &lit, slice_w));
        }
    }
    painter.add(blocks);
    painter.add(shine);
    if on("sparkle") && !art_cells.is_empty() {
        // glints: a few blocks of the visible art light up as a small four-pointed star that grows and fades, each
        // in its own rhythm; then the next one somewhere else
        let color = if core::is_dark(theme.bg) { Color32::WHITE } else { rgb(theme.art.map(|v| (v as f32 * 0.6) as u8)) };
        let mut mesh = Mesh::default();
        let count = match cfg["sparkle_amount"].as_str() { Some("few") => 4, Some("many") => 16, _ => 8 };
        for k in 0..count {
            let phase = fx.t / 0.8 + k as f32 * 0.618 * 3.;
            let (r, c) = art_cells[(hash(&[k as f64, phase.floor() as f64]) * art_cells.len() as f32) as usize % art_cells.len()];
            let glow = (phase.fract() * std::f32::consts::PI).sin().powi(2);
            if glow < 0.02 {
                continue;
            }
            let centre = Pos2::new(g.origin.x + (c as f32 + 0.5) * g.cw, g.origin.y + (r as f32 + 0.5) * g.ch);
            let (arm, thick) = (g.ch * (0.35 + 0.55 * glow), (g.cw * 0.14).max(1. / g.ppp));
            let none = color.gamma_multiply(0.);
            let mut fan = |points: &[Pos2], inner: f32| {
                let i = mesh.vertices.len() as u32;
                mesh.colored_vertex(centre, color.gamma_multiply(inner));
                for (n, p) in points.iter().enumerate() {
                    mesh.colored_vertex(*p, none);
                    mesh.add_triangle(i, i + 1 + n as u32, i + 1 + ((n + 1) % points.len()) as u32);
                }
            };
            let ring: Vec<Pos2> = (0..12).map(|i| centre + Vec2::angled(i as f32 / 12. * std::f32::consts::TAU) * arm * 0.6).collect();
            fan(&ring, 0.55 * glow);
            let (h, v, t) = (Vec2::new(arm, 0.), Vec2::new(0., arm), thick / 2.);
            fan(&[centre - h, centre - Vec2::new(0., t), centre + h, centre + Vec2::new(0., t)], glow);
            fan(&[centre - v, centre + Vec2::new(t, 0.), centre + v, centre - Vec2::new(t, 0.)], glow);
        }
        painter.add(mesh);
    }
    painter.add(marks);
    if cfg["text_glow"] == true && !plain {
        // the text blurred twice, tight and wide, under the sharp text (as gpu.py of NfoGlow for Linux); made once
        let key = format!("{page_id} {} {} {} {:?} {:?} {:?} {:?}", g.cw, g.ch, ctx.pixels_per_point(), font, theme.text, theme.art, theme.link);
        if cache.text_key != key {
            cache.text_glow.clear();
            for (i, (k, alpha)) in [((g.ch * 0.2).max(2.) / 2., 0.7), ((g.ch * 0.5).max(4.) / 2., 0.5)].into_iter().enumerate() {
                if let Some((image, m)) = bake_text(ctx, page, theme, font, g.cw, g.ch, k) {
                    cache.text_glow.push((ctx.load_texture(format!("text-glow{i}"), image, TextureOptions::LINEAR), k, m, alpha));
                }
            }
            cache.text_key = key;
        }
        for (image, k, m, alpha) in &cache.text_glow {
            let at = Rect::from_min_size(Pos2::new(g.x(0.), g.y(0.)) - Vec2::splat(*m), image.size_vec2() * *k);
            painter.image(image.id(), at, Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)), Color32::WHITE.gamma_multiply(*alpha));
        }
    }
    for (pos, galley) in texts {
        painter.galley(pos, galley, Color32::PLACEHOLDER);
    }
}

/// The still tube over the view: scanlines every third pixel row and dark corners (a vignette). It stays put while
/// the NFO scrolls.
/// lines: the scanlines (off when the render pass of the view lays a colour mask on instead, see post.rs).
pub fn crt(ui: &egui::Ui, cache: &mut Cache, rect: Rect, scanlines: bool) {
    let rect = rect.intersect(ui.clip_rect());
    let lines = cache.scanlines.get_or_insert_with(|| {
        let image = egui::ColorImage::new([1, 3], vec![Color32::from_black_alpha(71), Color32::TRANSPARENT, Color32::TRANSPARENT]); // .28
        let options = TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..TextureOptions::NEAREST };
        ui.ctx().load_texture("scanlines", image, options)
    });
    let rows = rect.height() * ui.ctx().pixels_per_point() / 3.;
    if scanlines {
        ui.painter().image(lines.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1., rows)), Color32::WHITE);
    }
    // the vignette: clear in the middle, half dark far out
    let (n, mut mesh) = (24, Mesh::default());
    let (inner, outer) = (rect.width().min(rect.height()) * 0.35, rect.width().max(rect.height()) * 0.75);
    for j in 0..=n {
        for i in 0..=n {
            let pos = rect.lerp_inside(Vec2::new(i as f32 / n as f32, j as f32 / n as f32));
            let dark = ((pos.distance(rect.center()) - inner) / (outer - inner)).clamp(0., 1.) * 0.5;
            mesh.colored_vertex(pos, Color32::from_black_alpha((dark * 255.) as u8));
        }
    }
    for j in 0..n {
        for i in 0..n {
            let a = j * (n + 1) + i;
            mesh.add_triangle(a, a + 1, a + n + 2);
            mesh.add_triangle(a, a + n + 2, a + n + 1);
        }
    }
    ui.painter().add(mesh);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ramp_runs_over_each_block_of_art_and_the_glow_is_baked_small() {
        let page = Page::fill("██\n██\ntext\n██".as_bytes(), "auto", false);
        let ramp = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];
        assert_eq!(block_color(&page, 0, 0., Some(&ramp), [9, 9, 9]), [255, 0, 0]);
        assert_eq!(block_color(&page, 1, 1., Some(&ramp), [9, 9, 9]), [0, 255, 0]); // the middle of the two rows
        assert_eq!(block_color(&page, 3, 3., Some(&ramp), [9, 9, 9]), [255, 0, 0]); // a new block of art: a new ramp
        assert_eq!(block_color(&page, 0, 0., None, [9, 9, 9]), [9, 9, 9]);
        let (image, m) = bake(&page, 8., 16., 4., None, [200, 100, 50]).unwrap();
        assert_eq!((image.size, m), ([12, 20], 8.)); // (4 * 8 + 16) / 4 by (4 * 16 + 16) / 4
        assert_eq!(image.pixels[0], Color32::TRANSPARENT); // the margin
        assert_eq!(image.pixels[3 * 12 + 3], Color32::from_rgb(200, 100, 50)); // inside the art: fully covered
        assert_eq!(image.pixels[11 * 12 + 3], Color32::TRANSPARENT); // the text row
        assert!(bake(&Page::default(), 8., 16., 4., None, [0; 3]).is_some_and(|(i, _)| i.pixels.iter().all(|p| *p == Color32::TRANSPARENT)));
    }

    fn still() -> Fx {
        Fx { t: 0., level: None, neon: None, view: Rect::ZERO }
    }

    #[test]
    fn glow_follows_the_settings() {
        let page = Page::fill("██".as_bytes(), "auto", false);
        let mut cfg = core::defaults();
        assert_eq!(glow_parts(&page, &cfg, 9., &still()), (false, vec![(13.5, 0.7), (4.5, 0.8)])); // neon: wide and tight
        cfg.insert("pulse".into(), true.into());
        let loud = Fx { level: Some(1.), ..still() };
        assert!(glow_parts(&page, &cfg, 9., &loud).1[0].1 > glow_parts(&page, &cfg, 9., &Fx { level: Some(0.), ..still() }).1[0].1); // the beat
        let breath: Vec<f32> = (0..20).map(|i| glow_parts(&page, &cfg, 9., &Fx { t: i as f32 / 10., ..still() }).1[0].1).collect();
        assert!(breath.iter().any(|a| *a < 0.4) && breath.iter().any(|a| *a > 0.6)); // without music it breathes
        cfg.insert("glow_mode".into(), "soft".into());
        assert_eq!(glow_parts(&page, &cfg, 9., &loud), (true, vec![(3., 1.)]));
        cfg.insert("soft".into(), false.into());
        assert!(glow_parts(&page, &cfg, 9., &still()).1.is_empty());
        let ansi = Page::fill(b"\x1b[31m\xdb", "auto", false);
        assert!(glow_parts(&ansi, &core::defaults(), 9., &still()).1.is_empty()); // ANSI art never glows
    }

    #[test]
    fn colour_bands_move_and_repeat() {
        let mut cfg = core::defaults();
        let c64 = Bands::new(&cfg, None, 15., 0.);
        assert!(c64.hard && c64.band == 5. && c64.at(0.) == core::C64_CYCLE[0] && c64.at(4.9) == core::C64_CYCLE[0] && c64.at(5.) == core::C64_CYCLE[1]);
        assert_eq!(c64.at(5. * 14.), core::C64_CYCLE[0]); // the pattern repeats
        assert_eq!(c64.cuts(2., 16.), [5., 10., 15.]);
        assert_ne!(Bands::new(&cfg, None, 15., 0.2).at(0.), c64.at(0.)); // 2.5 rows a second
        cfg.insert("cycle_dir".into(), "up".into());
        assert_eq!(Bands::new(&cfg, None, 15., 0.4).at(0.), core::C64_CYCLE[3]); // 15 points up: three bands on
        cfg.insert("cycle_palette".into(), "ramp".into());
        let ramp = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];
        let soft = Bands::new(&cfg, Some(&ramp), 10., 0.);
        assert_eq!((soft.at(0.), soft.at(10.), soft.at(20.), soft.at(60.), soft.at(80.)), ([255, 0, 0], [128, 128, 0], [0, 255, 0], [0, 255, 0], [255, 0, 0]));
    }

    #[test]
    fn neon_flickers_now_and_then_and_comes_on_slice_by_slice() {
        let levels: Vec<f32> = (0..2000).map(|i| flicker(i as f32 / 20.)).collect();
        let steady = levels.iter().filter(|v| **v == 1.).count();
        assert!(steady > 1600 && steady < 2000 && levels.iter().all(|v| (0.1..=1.).contains(v))); // mostly steady
        for i in 0..40 {
            assert_eq!((neon_slice(i, 0.), neon_slice(i, 1.7)), (0., 1.)); // dark at first, all on at the end
        }
        assert!((0..40).any(|i| neon_slice(i, 0.5) == 1.) && (0..40).any(|i| neon_slice(i, 0.5) == 0.)); // each its own start
        assert!((0..1000).all(|i| (0. ..1.).contains(&hash(&[i as f64, 3.]))));
    }
}
