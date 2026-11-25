// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! What moves around the page: the starfield behind it, the sine scroller and the living tube over it, and the
//! effects on opening a file. Ported from overlays.py and gpu.py of NfoGlow for Linux.
//! An effect that shows the page bit by bit (modem, code rain, dither) draws the whole page and covers the rest; the
//! view's own render pass (post.rs) shows what lies under the page (the stars) where it is covered.
use crate::core::{self, Rgb, Theme};
use crate::page::{Cell, Page};
use crate::view::{hash, rgb, Grid};
use eframe::egui::{self, Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Stroke, TextureHandle, TextureOptions, Vec2};

/// The small repeated images, made once.
#[derive(Default)]
pub struct Tiles {
    noise: Option<TextureHandle>,
    dither: Option<(Rgb, Vec<TextureHandle>)>,
}

const REPEAT: TextureOptions = TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..TextureOptions::NEAREST };

/// A tile over rect, one tile pixel on one screen pixel; shift: where the pattern starts, in tile pixels.
fn tiled(painter: &Painter, tile: &TextureHandle, rect: Rect, shift: Vec2) {
    let size = tile.size_vec2() / painter.ctx().pixels_per_point();
    let uv = Rect::from_min_size((shift / tile.size_vec2()).to_pos2(), rect.size() / size);
    painter.image(tile.id(), rect, uv, Color32::WHITE);
}

fn gradient(mesh: &mut Mesh, rect: Rect, top: Color32, bottom: Color32) {
    let i = mesh.vertices.len() as u32;
    for (pos, color) in [(rect.left_top(), top), (rect.right_top(), top), (rect.right_bottom(), bottom), (rect.left_bottom(), bottom)] {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(i, i + 1, i + 2);
    mesh.add_triangle(i, i + 2, i + 3);
}

// ------------------------------------------------------------ around the page

/// A starfield over rect: three layers drifting left, the near ones faster and brighter; or warp stars flying at
/// you out of the middle, each with a streak that grows as it comes closer.
pub fn stars(painter: &Painter, rect: Rect, warp: bool, color: Rgb, zoom: f32, t: f32) {
    let (w, h) = (rect.width(), rect.height());
    if warp {
        let reach = w.min(h) / 4.;
        for i in 0..((w * h / 2500.) as usize).max(60) {
            let (x, y, z0) = (hash(&[i as f64, 1.]) * 2. - 1., hash(&[i as f64, 2.]) * 2. - 1., hash(&[i as f64, 3.]));
            let z = (z0 - t * 0.35).rem_euclid(1.) + 0.02; // 1: far away, near 0: right in front of you
            let at = |z: f32| rect.center() + Vec2::new(x, y) / z * reach;
            if rect.expand(50.).contains(at(z)) {
                let near = (1. - z).max(0.); // z reaches 1.02
                painter.line_segment([at(z + 0.06), at(z)], Stroke::new((near * 2.4).max(0.6), rgb(color).gamma_multiply((near * 1.3).min(1.))));
            }
        }
        return;
    }
    let mut mesh = Mesh::default();
    for layer in 0..3 {
        let (speed, size, alpha) = ([8., 25., 60.][layer] * zoom, [1., 1.5, 2.][layer] * zoom.max(1.), [0.35, 0.6, 0.95][layer]);
        for i in 0..((w * h / 9000.) as usize).max(30) {
            let x = (hash(&[i as f64, layer as f64, 1.]) * w - t * speed).rem_euclid(w);
            let pos = rect.min + Vec2::new(x, hash(&[i as f64, layer as f64, 2.]) * h);
            mesh.add_colored_rect(Rect::from_min_size(pos, Vec2::splat(size)), rgb(color).gamma_multiply(alpha));
        }
    }
    painter.add(mesh);
}

/// The demoscene sine scroller along the bottom of rect: each letter rides its own wave and has its own colour from
/// the C64 cycle. CJK letters are two steps wide, as in NfoGlow for Linux.
pub fn scroller(painter: &Painter, rect: Rect, t: f32, text: &str) {
    use unicode_width::UnicodeWidthChar;
    let size = (rect.height() / 14.).clamp(18., 42.);
    let advance = size * 0.62;
    let letters: Vec<char> = text.chars().collect();
    let widths: Vec<f32> = letters.iter().map(|c| advance * if c.width() == Some(2) { 2. } else { 1. }).collect();
    let start = rect.width() - (t * 140.).rem_euclid(rect.width() + widths.iter().sum::<f32>()); // 140 points a second
    let base = rect.height() - size * 2.2;
    let mut x = start;
    for (i, ch) in letters.iter().enumerate() {
        if (-widths[i]..=rect.width()).contains(&x) && *ch != ' ' {
            let pos = rect.min + Vec2::new(x, base + size * 0.6 * (x / 90. + t * 3.).sin());
            let color = core::C64_CYCLE[(i + (t * 8.) as usize) % core::C64_CYCLE.len()];
            painter.text(pos + Vec2::splat(2.), Align2::LEFT_TOP, ch, FontId::monospace(size), Color32::from_black_alpha(153));
            painter.text(pos, Align2::LEFT_TOP, ch, FontId::monospace(size), rgb(color));
        }
        x += widths[i];
    }
}

/// How dark the flicker of the tube makes the picture right now: irregular, but never jumpy.
pub(crate) fn crt_flicker(t: f32) -> f32 {
    (0.025 + 0.025 * (t * 61.).sin() * (t * 17.).sin()).max(0.)
}

/// The living tube over rect: a roll bar wandering down the screen every 11 s, a faint flicker, picture noise, and
/// round corners (they fake the curved glass without bending the picture, so clicks stay where they are).
pub fn crt_live(painter: &Painter, tiles: &mut Tiles, rect: Rect, t: f32) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let (w, h) = (rect.width(), rect.height());
    let bar = Rect::from_min_size(rect.min + Vec2::new(0., ((t * 0.12).rem_euclid(1.3) - 0.15) * h), Vec2::new(w, h * 0.14));
    let mut mesh = Mesh::default();
    let (clear, light) = (Color32::TRANSPARENT, Color32::WHITE.gamma_multiply(0.07));
    gradient(&mut mesh, bar.split_top_bottom_at_fraction(0.5).0, clear, light);
    gradient(&mut mesh, bar.split_top_bottom_at_fraction(0.5).1, light, clear);
    painter.add(mesh);
    painter.rect_filled(rect, 0, Color32::from_black_alpha((crt_flicker(t) * 255.) as u8));
    let noise = tiles.noise.get_or_insert_with(|| {
        // grey grain, faint: 16 of 255
        let grain: Vec<Color32> = (0..128 * 128).map(|i| (hash(&[i as f64, 7.]) * 16.) as u8).map(|v| Color32::from_rgba_premultiplied(v, v, v, 16)).collect();
        painter.ctx().load_texture("noise", egui::ColorImage::new([128, 128], grain), REPEAT)
    });
    let tick = (t * 30.).floor() as f64; // from a new place every frame
    tiled(&painter, noise, rect, Vec2::new(hash(&[tick]), hash(&[tick, 1.])) * 128.);
    let r = w.min(h) * 0.06;
    painter.rect_stroke(rect, r.min(255.) as u8, Stroke::new(r, Color32::from_black_alpha(235)), egui::StrokeKind::Outside);
}

// ------------------------------------------------------------ on opening

/// How many seconds an effect on opening runs. chars: the characters of the page (the modem takes at least 2400 a
/// second, never over 4 s).
pub fn opening_end(kind: &str, name: &str, chars: usize) -> f32 {
    match kind {
        "modem" => chars as f32 / (chars as f32 / 4.).max(2400.),
        "glitch" => 0.7,
        "loader" | "rain" => 2.4,
        "crt" => 1.1,
        "dither" => 1.6,
        "neon" => 1.7,
        "fade" => 0.3, // stepping to the next NFO with the arrow keys
        "dos" => dos_steps(name, 0, 0).last().map_or(0., |s| s.1 + s.3) + 0.65, // the last command typed, Enter, a moment
        _ => 0.,
    }
}

pub fn total_chars(page: &Page) -> usize {
    page.lines.iter().map(|l| l.len() + 1).sum()
}

/// The cell the modem build-up has reached after n characters.
pub fn reveal_cell(page: &Page, n: usize) -> Cell {
    let mut left = n;
    for (r, line) in page.lines.iter().enumerate() {
        if left <= line.len() {
            return (r, left);
        }
        left -= line.len() + 1; // + 1: the line break
    }
    (page.lines.len(), 0)
}

/// The modem build-up: the page up to the current character, then a block cursor.
/// cover: where the rest is covered (the window's own render pass shows the stars there, see post.rs).
pub fn modem(painter: &Painter, cover: &Painter, g: Grid, view: Rect, at: Cell, bg: Rgb, text: Rgb) {
    let (y, x) = (g.y(at.0 as f32), g.x(at.1 as f32));
    cover.rect_filled(Rect::from_min_max(Pos2::new(view.left(), g.y(at.0 as f32 + 1.)), view.max), 0, rgb(bg));
    cover.rect_filled(Rect::from_min_max(Pos2::new(x, y), Pos2::new(view.right(), g.y(at.0 as f32 + 1.))), 0, rgb(bg));
    painter.rect_filled(Rect::from_min_max(Pos2::new(x, y), Pos2::new(g.x(at.1 as f32 + 1.), g.y(at.0 as f32 + 1.))), 0, rgb(text));
}

/// The glitch: strips of the page to draw again, shifted sideways, like a crackling loader; and how bright the
/// flash over it is.
pub fn glitch(view: Rect, t: f32) -> (Vec<(Rect, f32)>, f32) {
    let seed = (t * 25.).floor() as f64;
    let strip = |i: usize| {
        let rnd = |k: f64| hash(&[seed, i as f64, k]);
        let top = view.top() + rnd(1.) * view.height();
        (Rect::from_min_max(Pos2::new(view.left(), top), Pos2::new(view.right(), top + 3. + rnd(2.) * 33.)), rnd(3.) * 96. - 48.)
    };
    ((0..12).map(strip).collect(), 0.12 * hash(&[seed, 99.]))
}

/// The CRT power-on: black, then a bright line that widens across the screen, then the picture opens out of it from
/// the middle, overexposed at first.
/// stretched: the render pass of the view (post.rs) stretches the picture out of the line; else it is uncovered.
pub fn power_on(painter: &Painter, view: Rect, t: f32, stretched: bool) {
    let (line_from, grow_from, px) = (0.12, 0.42, 1. / painter.ctx().pixels_per_point());
    let middle = |width: f32, height: f32| Rect::from_center_size(view.center(), Vec2::new(width, height));
    if t < grow_from {
        painter.rect_filled(view, 0, Color32::BLACK);
        if t >= line_from {
            let width = view.width() * ((t - line_from) / (grow_from - line_from)).powf(0.6);
            painter.rect_filled(middle(width, 6. * px), 0, Color32::from_rgb(204, 224, 255).gamma_multiply(0.22));
            painter.rect_filled(middle(width, 2. * px), 0, Color32::from_rgb(242, 250, 255));
        }
        return;
    }
    let u = ((t - grow_from) / (1.1 - grow_from)).min(1.);
    let open = middle(view.width(), (view.height() * power_on_grown(t)).max(2. * px));
    if !stretched || u < 1. {
        painter.rect_filled(Rect::from_min_max(view.min, open.right_top()), 0, Color32::BLACK);
        painter.rect_filled(Rect::from_min_max(open.left_bottom(), view.max), 0, Color32::BLACK);
    }
    painter.rect_filled(open, 0, Color32::from_rgb(224, 240, 255).gamma_multiply(0.55 * (1. - u).powi(2)));
}

/// How far the picture has grown out of the line of the CRT power-on at t seconds (0 before it grows, 1 whole).
pub fn power_on_grown(t: f32) -> f32 {
    let u = ((t - 0.42) / (1.1 - 0.42)).clamp(0., 1.);
    1. - (1. - u).powi(3)
}

const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]; // which of 16 pixels light up at a level

/// The dither fade-in: a wave runs down the window; behind it the page is there in steps of ░ ▒ ▓ and then whole.
pub fn dither(painter: &Painter, tiles: &mut Tiles, view: Rect, t: f32, ch: f32, bg: Rgb) {
    if tiles.dither.as_ref().is_none_or(|d| d.0 != bg) {
        // per level, the pixels that are still missing, in the background colour
        let tile = |level: u8| {
            let pixels = BAYER.iter().flatten().map(|b| if *b < level * 4 { Color32::TRANSPARENT } else { rgb(bg) }).collect();
            painter.ctx().load_texture("dither", egui::ColorImage::new([4, 4], pixels), REPEAT)
        };
        tiles.dither = Some((bg, (1..4).map(tile).collect()));
    }
    let span = view.height() * 0.5; // how tall the zone of steps is
    let front = (t / (1.6 * 0.8)).min(1.) * (view.height() + span); // the wave is through at 80 % of the time
    let band = ch.max(8.);
    for k in 0..(view.height() / band).ceil() as usize {
        let u = (front - k as f32 * band) / span;
        let rect = Rect::from_min_size(view.min + Vec2::new(0., k as f32 * band), Vec2::new(view.width(), band)).intersect(view);
        match (u * 4.).ceil() as i32 {
            4.. => {}
            level @ 1..=3 => tiled(painter, &tiles.dither.as_ref().unwrap().1[level as usize - 1], rect, Vec2::ZERO),
            _ => {
                painter.rect_filled(rect, 0, rgb(bg));
            }
        }
    }
}

const RAIN_GLYPHS: &str = concat!("010101010101010101010101010101010101010101010101", "23456789ABCDEF"); // mostly ones and zeros, now and then a hex digit
const RAIN_TRAIL: usize = 12; // characters of trail behind the head

/// The code rain: digits fall in every character column of the NFO, a bright one at the head and a fading trail
/// behind it; above the trail the page is there. Each column has its own start and speed and is through before the
/// end. Only over the NFO's own columns: the NFO rains in.
/// painters: where the digits go (the page and what lies under it); cover: where the page is not there yet.
pub fn rain(painters: &[Option<&Painter>], cover: &Painter, g: Grid, page: &Page, view: Rect, t: f32, theme: &Theme, bg: Rgb, font: &FontId) {
    let trail = RAIN_TRAIL as f32 * g.ch;
    let tick = (t * 16.).floor() as f64; // the digits near the head change quickly, the rest of the trail slowly
    let head_color = if core::is_dark(bg) { theme.art.map(|v| v + ((255 - v) as f32 * 0.75) as u8) } else { theme.art };
    let glyphs: Vec<char> = RAIN_GLYPHS.chars().collect();
    let mut drops = Vec::new();
    for c in 0..page.cols {
        let (start, fall) = (hash(&[c as f64]) * 0.9, 0.8 + hash(&[c as f64, 1.]) * 0.5); // done by 2.2 s
        let u = (t - start) / fall;
        let head = view.top() + u.clamp(0., 1.) * (view.height() + trail);
        // the page shows above the trail; the outer columns also open the margins beside them
        let left = if c == 0 { view.left() } else { g.x(c as f32) };
        let right = if c == page.cols - 1 { view.right() } else { g.x(c as f32 + 1.) };
        cover.rect_filled(Rect::from_min_max(Pos2::new(left, (head - trail).max(view.top())), Pos2::new(right, view.bottom())), 0, rgb(bg));
        if u > 0. {
            drops.push((c, head));
        }
    }
    for (c, head) in drops {
        let row = ((head - g.origin.y) / g.ch).floor() as i64;
        for k in 0..RAIN_TRAIL {
            let y = g.origin.y + (row - k as i64) as f32 * g.ch;
            if y + g.ch < view.top() || y > view.bottom() {
                continue;
            }
            let slow = if k < 2 { tick } else { (tick / 5.).floor() };
            let glyph = glyphs[(hash(&[c as f64, (row - k as i64) as f64, slow]) * glyphs.len() as f32) as usize % glyphs.len()];
            let shade = k * 8 / RAIN_TRAIL; // 8 shades, from the bright head down to the faint end of the trail
            let color = rgb(if shade == 0 { head_color } else { theme.art }).gamma_multiply((1. - shade as f32 / 8.).powf(1.4));
            for painter in painters.iter().flatten() {
                painter.text(Pos2::new(g.x(c as f32), y), Align2::LEFT_TOP, glyph, font.clone(), color);
            }
        }
    }
}

/// The C64 loader at t seconds: (lines of text, cursor (row, column), stripes in the border).
/// READY., the LOAD command typed, SEARCHING, LOADING with the coloured stripes of a tape loader, READY. again.
pub(crate) fn loader_screen(t: f32, name: &str) -> (Vec<String>, Option<Cell>, bool) {
    let stem = name.rsplit_once('.').filter(|n| !n.0.is_empty()).map_or(name, |n| n.0).to_uppercase(); // os.path.splitext: ".nfo" has no extension
    let stem: String = stem.chars().filter(|c| (' '..='~').contains(c)).take(16).collect(); // 16 like on a disk
    let stem = if stem.is_empty() { "NFO".into() } else { stem };
    let typed = format!("LOAD\"{stem}\",8,1");
    let blink = (t * 3.3) as i32 % 2 == 0; // the cursor blinks while it waits
    let mut lines = vec!["READY.".to_string()];
    if t < 0.35 {
        return (lines, blink.then_some((1, 0)), false);
    }
    let shown = typed.len().min(((t - 0.35) / 0.6 * typed.len() as f32) as usize + 1);
    lines.push(typed[..shown].into());
    if t < 0.95 {
        return (lines, Some((1, shown)), false);
    }
    lines.extend([String::new(), format!("SEARCHING FOR {stem}")]);
    if t < 1.15 {
        return (lines, None, false);
    }
    lines.push("LOADING".into());
    if t < 2.05 {
        return (lines, None, true);
    }
    lines.push("READY.".into());
    let row = lines.len();
    (lines, blink.then_some((row, 0)), false)
}

// An 8 x 8 pixel font of our own in the style of the old home computers (two pixels wide strokes), for the tape
// loader. Rows top to bottom, "#" a pixel; the eighth row stays empty. Characters it lacks are shown as "?".
const FONT8: &str = r#"
A ..####.. .##..##. .##..##. .######. .##..##. .##..##. .##..##.
B .#####.. .##..##. .##..##. .#####.. .##..##. .##..##. .#####..
C ..####.. .##..##. .##..... .##..... .##..... .##..##. ..####..
D .####... .##.##.. .##..##. .##..##. .##..##. .##.##.. .####...
E .######. .##..... .##..... .####... .##..... .##..... .######.
F .######. .##..... .##..... .####... .##..... .##..... .##.....
G ..####.. .##..##. .##..... .##.###. .##..##. .##..##. ..####..
H .##..##. .##..##. .##..##. .######. .##..##. .##..##. .##..##.
I ..####.. ...##... ...##... ...##... ...##... ...##... ..####..
J ...####. ....##.. ....##.. ....##.. ....##.. .##.##.. ..###...
K .##..##. .##.##.. .####... .###.... .####... .##.##.. .##..##.
L .##..... .##..... .##..... .##..... .##..... .##..... .######.
M .##...## .###.### .####### .##.#.## .##...## .##...## .##...##
N .##..##. .###.##. .######. .######. .##.###. .##..##. .##..##.
O ..####.. .##..##. .##..##. .##..##. .##..##. .##..##. ..####..
P .#####.. .##..##. .##..##. .#####.. .##..... .##..... .##.....
Q ..####.. .##..##. .##..##. .##..##. .##..##. ..####.. ....###.
R .#####.. .##..##. .##..##. .#####.. .####... .##.##.. .##..##.
S ..####.. .##..##. .##..... ..####.. .....##. .##..##. ..####..
T .######. ...##... ...##... ...##... ...##... ...##... ...##...
U .##..##. .##..##. .##..##. .##..##. .##..##. .##..##. ..####..
V .##..##. .##..##. .##..##. .##..##. .##..##. ..####.. ...##...
W .##...## .##...## .##...## .##.#.## .####### .###.### .##...##
X .##..##. .##..##. ..####.. ...##... ..####.. .##..##. .##..##.
Y .##..##. .##..##. .##..##. ..####.. ...##... ...##... ...##...
Z .######. .....##. ....##.. ...##... ..##.... .##..... .######.
0 ..####.. .##..##. .##.###. .###.##. .##..##. .##..##. ..####..
1 ...##... ..###... ...##... ...##... ...##... ...##... .######.
2 ..####.. .##..##. .....##. ....##.. ..##.... .##..... .######.
3 ..####.. .##..##. .....##. ...###.. .....##. .##..##. ..####..
4 ....##.. ...###.. ..####.. .##.##.. .######. ....##.. ....##..
5 .######. .##..... .#####.. .....##. .....##. .##..##. ..####..
6 ..####.. .##..... .##..... .#####.. .##..##. .##..##. ..####..
7 .######. .....##. ....##.. ...##... ..##.... ..##.... ..##....
8 ..####.. .##..##. .##..##. ..####.. .##..##. .##..##. ..####..
9 ..####.. .##..##. .##..##. ..#####. .....##. .....##. ..####..
. ........ ........ ........ ........ ........ ...##... ...##...
, ........ ........ ........ ........ ...##... ...##... ..##....
" .##.##.. .##.##.. .##.##.. ........ ........ ........ ........
' ...##... ...##... ..##.... ........ ........ ........ ........
: ........ ...##... ...##... ........ ...##... ...##... ........
! ...##... ...##... ...##... ...##... ........ ........ ...##...
? ..####.. .##..##. .....##. ....##.. ...##... ........ ...##...
* ........ .##..##. ..####.. .######. ..####.. .##..##. ........
- ........ ........ ........ .######. ........ ........ ........
_ ........ ........ ........ ........ ........ ........ ########
/ ......## .....##. ....##.. ...##... ..##.... .##..... ##......
( ....##.. ...##... ..##.... ..##.... ..##.... ...##... ....##..
) ..##.... ...##... ....##.. ....##.. ....##.. ...##... ..##....
+ ........ ...##... ...##... .######. ...##... ...##... ........
= ........ ........ .######. ........ .######. ........ ........
"#;

/// The seven pixel rows of a character of the loader font.
fn font8(ch: char) -> Vec<&'static str> {
    let row = |ch: char| FONT8.lines().find(|l| l.starts_with(ch));
    row(ch).or(row('?')).unwrap()[2..].split(' ').collect()
}

/// The C64 tape loader over rect: the blue screen in its own 320 x 200 pixels with the 8 x 8 font, enlarged by a
/// whole number of screen pixels, in its light blue border, which flickers with coloured stripes while it loads.
pub fn loader(painter: &Painter, rect: Rect, t: f32, name: &str) {
    let painter = painter.with_clip_rect(rect);
    let (lines, cursor, stripes) = loader_screen(t, name);
    let mut mesh = Mesh::default();
    mesh.add_colored_rect(rect, rgb(core::C64_LIGHT_BLUE));
    if stripes {
        // a new set of stripes every frame, as the bits come in
        let (seed, mut y, mut i) = ((t * 30.).floor() as f64, rect.top(), 0.);
        while y < rect.bottom() {
            let height = 2. + (hash(&[seed, i, 1.]) * 13.).floor();
            let color = core::C64[(hash(&[seed, i, 2.]) * 13.) as usize % 13];
            mesh.add_colored_rect(Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), height)), rgb(color));
            (y, i) = (y + height, i + 1.);
        }
    }
    let ppp = painter.ctx().pixels_per_point();
    let border = (rect.width().min(rect.height()) * 0.07).max(12.);
    let pixel = (((rect.width() - 2. * border) / 320.).min((rect.height() - 2. * border) / 200.) * ppp).floor().max(1.) / ppp;
    let screen = Rect::from_center_size(rect.center(), Vec2::new(320., 200.) * pixel);
    let origin = Pos2::new((screen.left() * ppp).round() / ppp, (screen.top() * ppp).round() / ppp);
    let dots = |x: usize, y: usize, w: usize, h: usize| Rect::from_min_size(origin + Vec2::new(x as f32, y as f32) * pixel, Vec2::new(w as f32, h as f32) * pixel);
    mesh.add_colored_rect(dots(0, 0, 320, 200), rgb(core::C64_BLUE));
    let ink = rgb(core::C64_LIGHT_BLUE);
    for (row, text) in lines.iter().take(25).enumerate() {
        for (col, ch) in text.chars().take(40).enumerate().filter(|(_, ch)| *ch != ' ') {
            for (y, bits) in font8(ch).iter().enumerate() {
                for (x, _) in bits.bytes().enumerate().filter(|(_, b)| *b == b'#') {
                    mesh.add_colored_rect(dots(col * 8 + x, row * 8 + y, 1, 1), ink);
                }
            }
        }
    }
    if let Some((row, col)) = cursor {
        mesh.add_colored_rect(dots(col * 8, row * 8, 8, 8), ink);
    }
    painter.add(mesh);
}

/// The file name the way DOS 6 shows it: upper case, eight letters and a three letter extension (long names are cut
/// to six letters and ~1).
fn dos_83(name: &str) -> (String, String) {
    let (stem, ext) = name.rsplit_once('.').filter(|n| !n.0.is_empty()).unwrap_or((name, ""));
    let clean = |part: &str| -> String {
        let upper = part.to_uppercase().replace(' ', "");
        upper.chars().map(|c| if c.is_ascii_alphanumeric() || "_-!#$%&'()@^{}~".contains(c) { c } else { '_' }).collect()
    };
    let (stem, ext) = (clean(stem), clean(ext).chars().take(3).collect());
    let stem = if stem.is_empty() { "NFO".into() } else { stem };
    (if stem.chars().count() <= 8 { stem } else { format!("{}~1", stem.chars().take(6).collect::<String>()) }, ext)
}

fn commas(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(d);
    }
    out
}

/// (year, month, day, hour, minute) of seconds since 1970.
/// shortcut: in UTC, the standard library knows no time zones; the listing of the DOS boot is decoration.
fn civil(secs: i64) -> (i64, i64, i64, i64, i64) {
    let (days, rest) = (secs.div_euclid(86400) + 719468, secs.rem_euclid(86400));
    let (era, doe) = (days.div_euclid(146097), days.rem_euclid(146097));
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (month <= 2) as i64, month, doy - (153 * mp + 2) / 5 + 1, rest / 3600, rest % 3600 / 60)
}

/// The DOS boot as [(typed, start, text, seconds)]: a line of output shown at start, or a command typed after the
/// prompt in the given seconds. Boot message, VER, DIR of the file, TYPE of the file.
fn dos_steps(name: &str, size: u64, mtime: i64) -> Vec<(bool, f32, String, f32)> {
    let (stem, ext) = dos_83(name);
    let (_, month, day, hour, minute) = civil(mtime);
    let year = civil(mtime).0 % 100;
    let listing = format!("{stem:<8} {ext:<3} {:>13} {month:02}-{day:02}-{year:02} {:>2}:{minute:02}{}", commas(size), (hour + 11) % 12 + 1, if hour < 12 { 'a' } else { 'p' });
    let file = if ext.is_empty() { stem.clone() } else { format!("{stem}.{ext}") };
    let (mut steps, mut t) = (Vec::new(), 0.15);
    let out = |steps: &mut Vec<_>, t: &mut f32, lines: &[&str], wait: f32| {
        steps.extend(lines.iter().map(|l| (false, *t, l.to_string(), 0.)));
        *t += wait;
    };
    let command = |steps: &mut Vec<_>, t: &mut f32, text: String, pause: f32| {
        let seconds = (text.len() as f32 / 22.).clamp(0.25, 0.9); // about 22 letters a second
        steps.push((true, *t, text, seconds));
        *t += seconds + pause;
    };
    out(&mut steps, &mut t, &["Starting MS-DOS..."], 0.4);
    out(&mut steps, &mut t, &["", "HIMEM is testing extended memory...done.", ""], 0.4);
    command(&mut steps, &mut t, "VER".into(), 0.22);
    out(&mut steps, &mut t, &["", "MS-DOS Version 6.22", ""], 0.25);
    command(&mut steps, &mut t, format!("DIR {file}"), 0.22);
    let (files, free) = (format!("        1 file(s) {:>16} bytes", commas(size)), format!("{:>26} bytes free", commas(412319744)));
    out(&mut steps, &mut t, &["", " Volume in drive C is NFOGLOW", " Directory of C:\\", "", &listing, &files, &free, ""], 0.3);
    command(&mut steps, &mut t, format!("TYPE {file}"), 0.3);
    steps
}

/// The DOS boot at t seconds: (lines, cursor (row, column)).
pub(crate) fn dos_screen(t: f32, name: &str, size: u64, mtime: i64) -> (Vec<String>, Option<Cell>) {
    let prompt = "C:\\>";
    let (mut lines, mut cursor) = (Vec::new(), None);
    for (typed, start, text, seconds) in dos_steps(name, size, mtime) {
        if t < start {
            break;
        }
        if !typed {
            lines.push(text);
            continue;
        }
        let n = if t >= start + seconds { text.len() } else { ((t - start) / seconds * text.len() as f32) as usize };
        lines.push(format!("{prompt}{}", &text[..n]));
        cursor = Some((lines.len() - 1, prompt.len() + n));
        if n < text.len() {
            return (lines, cursor); // still typing: the cursor stays on
        }
    }
    if cursor.is_some() && t >= opening_end("dos", name, 0) - 0.35 {
        lines.push(String::new()); // Enter after the last command
        return (lines.clone(), Some((lines.len() - 1, 0)));
    }
    (lines, cursor.filter(|_| (t * 2.5) as i32 % 2 == 0))
}

/// The DOS boot over rect: black, grey letters in the page font, an underscore cursor.
pub fn dos(painter: &Painter, rect: Rect, t: f32, name: &str, size: u64, mtime: i64) {
    let painter = painter.with_clip_rect(rect);
    painter.rect_filled(rect, 0, Color32::BLACK);
    let (lines, cursor) = dos_screen(t, name, size, mtime);
    let font = FontId::monospace((rect.height() / 30.).clamp(12., 22.));
    let (advance, line) = painter.ctx().fonts_mut(|f| (f.glyph_width(&font, 'M'), f.row_height(&font)));
    let (grey, pad) = (rgb(core::VGA[7]), font.size * 1.2);
    for (row, text) in lines.iter().enumerate() {
        painter.text(rect.min + Vec2::new(pad, pad + row as f32 * line), Align2::LEFT_TOP, text, font.clone(), grey);
    }
    if let Some((row, col)) = cursor {
        let thick = (font.size * 0.12).max(2.);
        let at = rect.min + Vec2::new(pad + col as f32 * advance, pad + (row as f32 + 1.) * line - thick * 1.5);
        painter.rect_filled(Rect::from_min_size(at, Vec2::new(advance, thick)), 0, grey);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tape_loader_runs_its_script() {
        assert_eq!(loader_screen(0., "Group-Release.nfo"), (vec!["READY.".into()], Some((1, 0)), false));
        let (lines, cursor, stripes) = loader_screen(0.6, "Group-Release.nfo");
        assert!(lines[1].starts_with("LOAD\"GR") && cursor == Some((1, lines[1].len())) && !stripes); // being typed
        let (lines, _, stripes) = loader_screen(1.5, "a very long name of a release.nfo");
        assert_eq!((lines[1].as_str(), lines[3].as_str(), lines[4].as_str(), stripes), ("LOAD\"A VERY LONG NAME\",8,1", "SEARCHING FOR A VERY LONG NAME", "LOADING", true));
        assert_eq!(loader_screen(2.3, "x.nfo").0.last().unwrap(), "READY.");
        assert_eq!(loader_screen(1., "日本.nfo").0[1], "LOAD\"NFO\",8,1"); // nothing the C64 could show
        assert_eq!(font8('A').len(), 7);
        assert!(font8('ß') == font8('?') && FONT8.lines().skip(1).all(|l| l[2..].split(' ').all(|bits| bits.len() == 8)));
    }

    #[test]
    fn dos_boot_lists_and_types_the_file() {
        assert_eq!(dos_83("Group-Release.2026.nfo"), ("GROUP-~1".into(), "NFO".into()));
        assert_eq!((dos_83("file_id.diz"), dos_83("Über uns")), (("FILE_ID".into(), "DIZ".into()), ("_BERUNS".into(), "".into())));
        assert_eq!((commas(0), commas(999), commas(1000), commas(412319744)), ("0".into(), "999".into(), "1,000".into(), "412,319,744".into()));
        assert_eq!((civil(0), civil(951782400), civil(1794528000 + 13 * 3600 + 5 * 60)), ((1970, 1, 1, 0, 0), (2000, 2, 29, 0, 0), (2026, 11, 13, 13, 5)));
        let end = opening_end("dos", "demo.nfo", 0);
        assert!(end > 2. && end < 6.);
        let (lines, cursor) = dos_screen(end, "demo.nfo", 1434, 1794528000 + 13 * 3600 + 5 * 60);
        assert_eq!(lines[0], "Starting MS-DOS...");
        assert!(lines.contains(&"C:\\>DIR DEMO.NFO".to_string()) && lines.contains(&"DEMO     NFO         1,434 11-13-26  1:05p".to_string()), "{lines:?}");
        assert_eq!((lines[lines.len() - 2].as_str(), cursor), ("C:\\>TYPE DEMO.NFO", Some((lines.len() - 1, 0)))); // Enter
        assert_eq!(dos_screen(0., "demo.nfo", 0, 0), (vec![], None));
        let (lines, cursor) = dos_screen(1.15, "demo.nfo", 0, 0); // typing VER
        assert!(lines.last().unwrap().starts_with("C:\\>V") && cursor.is_some());
    }

    #[test]
    fn every_opening_has_an_end_and_the_modem_reaches_every_cell() {
        for kind in ["modem", "glitch", "loader", "crt", "dos", "dither", "neon", "rain"] {
            assert!(opening_end(kind, "x.nfo", 5000) > 0.5, "{kind}");
        }
        assert_eq!((opening_end("modem", "", 2400), opening_end("modem", "", 96000), opening_end("off", "", 9)), (1., 4., 0.));
        let page = Page::fill(b"ab\n\ncd", "auto", false);
        assert_eq!(total_chars(&page), 7);
        let cells: Vec<Cell> = (0..=7).map(|n| reveal_cell(&page, n)).collect();
        assert_eq!(cells, [(0, 0), (0, 1), (0, 2), (1, 0), (2, 0), (2, 1), (2, 2), (3, 0)]);
        let (strips, flash) = glitch(Rect::from_min_size(Pos2::new(10., 20.), Vec2::new(300., 200.)), 0.3);
        assert!(strips.len() == 12 && strips.iter().all(|(r, dx)| r.top() >= 20. && r.top() <= 220. && dx.abs() <= 48.) && flash < 0.12);
        assert!(crt_flicker(0.) > 0. && (0..500).all(|i| (0. ..=0.05).contains(&crt_flicker(i as f32 / 30.))));
    }
}
