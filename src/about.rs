// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! The About window, as the About dialog of NfoGlow for Linux, and its artwork: Code-Cat, the pixel cat behind the
//! easter egg (five quick clicks on the logo). Ported from ui_about.py and ui_about_art.py; the cat, her movements and
//! the three scenes are drawn the same way, rectangle by rectangle. What she does is a scene: a function of the stage
//! that returns how long it runs; drawing it at a moment is draw_scene().
use crate::i18n::tr;
use crate::view::hash;
use crate::{ui, App};
use eframe::egui::{self, Color32, FontId, Mesh, Painter, Pos2, Rect, RichText, Vec2};
use std::f32::consts::PI;

type Rgb = (f32, f32, f32);

const P: f32 = 4.; // screen pixels per sprite pixel
const CP: f32 = 3.; // ... and per cat pixel
const OUTLINE: Rgb = (0.1, 0.1, 0.15);
const GREY: Rgb = (0.62, 0.62, 0.68);
const STRIPE: Rgb = (0.36, 0.36, 0.42);
const WHITE: Rgb = (1., 1., 1.);
const CHEEK: Rgb = (1., 0.55, 0.65);
const ORANGE: Rgb = (0.9, 0.6, 0.1);
const DARK: Rgb = (0.1, 0.1, 0.15);
const CAT_W: f32 = 24. * CP;
const CAT_H: f32 = 16. * CP;
const LOGO: f32 = 128.;

fn lerp(a: f32, b: f32, f: f32) -> f32 {
    a + (b - a) * f
}

fn clamp(v: f32) -> f32 {
    v.clamp(0., 1.)
}

/// Position at f (0..1) of a hop from a to b that rises `lift` pixels in the middle.
fn jump(f: f32, a: (f32, f32), b: (f32, f32), lift: f32) -> (f32, f32) {
    (lerp(a.0, b.0, f), lerp(a.1, b.1, f) - lift * 4. * f * (1. - f))
}

/// Draws like a cairo context: rectangles and ellipses under a transformation, collected into meshes, each with the
/// clip it was drawn in.
struct Pen<'a> {
    painter: &'a Painter,
    origin: Vec2,
    m: [f32; 6], // x' = a x + c y + e, y' = b x + d y + f
    stack: Vec<[f32; 6]>,
    mesh: Mesh,
    clip: Option<Rect>,
}

fn color(c: Rgb, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied((c.0 * 255.) as u8, (c.1 * 255.) as u8, (c.2 * 255.) as u8, (clamp(alpha) * 255.) as u8)
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        let [a, b, c, d, e, f] = self.m;
        Pos2::new(a * x + c * y + e, b * x + d * y + f) + self.origin
    }
    fn save(&mut self) {
        self.stack.push(self.m);
    }
    fn restore(&mut self) {
        self.m = self.stack.pop().unwrap();
    }
    fn translate(&mut self, tx: f32, ty: f32) {
        let [a, b, c, d, ..] = self.m;
        self.m[4] += a * tx + c * ty;
        self.m[5] += b * tx + d * ty;
    }
    fn scale(&mut self, sx: f32, sy: f32) {
        self.m[0] *= sx;
        self.m[1] *= sx;
        self.m[2] *= sy;
        self.m[3] *= sy;
    }
    fn rotate(&mut self, r: f32) {
        let ([a, b, c, d, ..], (s, k)) = (self.m, r.sin_cos());
        self.m[..4].copy_from_slice(&[a * k + c * s, b * k + d * s, -a * s + c * k, -b * s + d * k]);
    }
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color32) {
        let i = self.mesh.vertices.len() as u32;
        for (px, py) in [(x, y), (x + w, y), (x + w, y + h), (x, y + h)] {
            let p = self.at(px, py);
            self.mesh.colored_vertex(p, c);
        }
        self.mesh.add_triangle(i, i + 1, i + 2);
        self.mesh.add_triangle(i, i + 2, i + 3);
    }
    fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Color32) {
        let i = self.mesh.vertices.len() as u32;
        let centre = self.at(cx, cy);
        self.mesh.colored_vertex(centre, c);
        for n in 0..24 {
            let a = n as f32 / 24. * 2. * PI;
            let p = self.at(cx + rx * a.cos(), cy + ry * a.sin());
            self.mesh.colored_vertex(p, c);
            self.mesh.add_triangle(i, i + 1 + n, i + 1 + (n + 1) % 24);
        }
    }
    fn flush(&mut self) {
        let mesh = std::mem::take(&mut self.mesh);
        let painter = self.clip.map_or(self.painter.clone(), |clip| self.painter.with_clip_rect(clip.intersect(self.painter.clip_rect())));
        painter.add(mesh);
    }
    /// Only what lies in x, y, w, h (untransformed stage points) shows, until unclip().
    fn clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.flush();
        self.clip = Some(Rect::from_min_size(Pos2::new(x, y) + self.origin, Vec2::new(w, h)));
    }
    fn unclip(&mut self) {
        self.flush();
        self.clip = None;
    }
    fn text(&mut self, x: f32, y: f32, ch: char, font: &FontId, c: Color32) {
        self.flush();
        let painter = self.clip.map_or(self.painter.clone(), |clip| self.painter.with_clip_rect(clip));
        painter.text(self.at(x, y), egui::Align2::LEFT_TOP, ch, font.clone(), c);
    }
}

impl Drop for Pen<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Fill boxes (x, y, w, h in cat pixels); flip mirrors them inside the 24 wide cat.
fn rects(pen: &mut Pen, x: f32, y: f32, flip: bool, scale: f32, c: Rgb, boxes: &[(f32, f32, f32, f32)]) {
    for &(bx, by, bw, bh) in boxes {
        let bx = if flip { 24. - bx - bw } else { bx };
        pen.rect(x + bx * scale, y + by * scale, bw * scale + 0.5, bh * scale + 0.5, color(c, 1.));
    }
}

/// How the cat looks at a moment (see nyan).
#[derive(Default, Clone, Copy)]
struct Look {
    flip: bool,
    eating: bool,
    run: bool,
    alert: bool,
    sx: f32,
    sy: f32,
    air: bool,
    rot: f32,
    swipe: bool,
    sleep: bool,
    no_lines: bool,
}

fn look() -> Look {
    Look { sx: 1., sy: 1., ..Default::default() }
}

/// The cat: a grey tabby with white paws and muzzle, a big cute head (big eyes, blush, pink ears). Faces right unless
/// flipped. run: trotting (diagonal leg pairs, bobbing body, speed lines); air: legs stretched out; alert: exclamation
/// marks; sx, sy: squash and stretch around the paws; rot: nose up in radians; swipe: a paw strikes; sleep: eyes shut.
fn nyan(pen: &mut Pen, x: f32, y: f32, t: f32, l: Look) {
    let (f, k) = (l.flip, CP);
    pen.save();
    pen.translate(x + CAT_W / 2., y + CAT_H);
    pen.rotate(if f { l.rot } else { -l.rot });
    pen.scale(l.sx, l.sy);
    pen.translate(-x - CAT_W / 2., -y - CAT_H);
    let ph = if l.run && !l.air { t * 17. } else { 0. };
    let bob = if l.run && !l.air && (2. * ph).sin() > 0.2 { 1. } else { 0. }; // the body rises between the footfalls
    let yb = y - bob * k;
    let yh = yb + if l.eating { 2. * k } else { 0. }; // head dips when it bites
    if l.run && !l.no_lines {
        for (n, ly) in [6., 9., 12.].into_iter().enumerate() {
            let off = (((t / 0.08) as i64 + n as i64) % 3) as f32;
            rects(pen, x, y, f, k, GREY, &[(-9. - off, ly, 5., 1.)]);
        }
    }
    for r in 0..9 {
        // tail: a wave runs up from its root
        let dx = ((if l.run { 2.0 } else { 1.3 }) * (t * 6. + r as f32 * 0.55).sin()).round();
        let r = r as f32;
        rects(pen, x, yb, f, k, OUTLINE, &[(dx, 2. + r, 3., 1.)]);
        rects(pen, x, yb, f, k, if r == 2. || r == 5. { STRIPE } else { GREY }, &[(1. + dx, 2. + r, 1., 1.)]);
    }
    for (i, dx) in [4., 8., 14., 18.].into_iter().enumerate() {
        // legs with white paws; opposite corners move together
        let p = ph + if i == 1 || i == 2 { PI } else { 0. };
        let lift = if l.run { (p.sin().max(0.) * 2.).round() } else { 0. };
        let shift = if l.air { if dx > 10. { 1. } else { -1. } } else if l.run { (p.cos() * 1.4).round() } else { 0. };
        rects(pen, x, y, f, k, OUTLINE, &[(dx - 1. + shift, 11. + lift, 4., 5. - lift)]);
        rects(pen, x, y, f, k, GREY, &[(dx + shift, 11. + lift, 2., 2.)]);
        rects(pen, x, y, f, k, WHITE, &[(dx + shift, 13. + lift, 2., 2. - lift)]);
    }
    rects(pen, x, yb, f, k, OUTLINE, &[(3., 5., 14., 9.), (2., 6., 16., 7.)]); // body
    rects(pen, x, yb, f, k, GREY, &[(4., 6., 12., 7.), (3., 7., 14., 5.)]);
    rects(pen, x, yb, f, k, STRIPE, &[(6., 6., 1., 2.), (9., 6., 1., 3.), (12., 6., 1., 2.)]);
    rects(pen, x, yb, f, k, WHITE, &[(5., 11., 8., 2.)]); // belly
    rects(pen, x, yh, f, k, OUTLINE, &[(12., 4., 11., 11.), (11., 5., 13., 9.), (12., 2., 3., 3.), (20., 2., 3., 3.)]); // head with ears
    rects(pen, x, yh, f, k, GREY, &[(13., 5., 9., 9.), (12., 6., 11., 7.), (13., 3., 1., 2.), (21., 3., 1., 2.)]);
    rects(pen, x, yh, f, k, CHEEK, &[(14., 3., 1., 2.), (20., 3., 1., 2.)]); // inner ears
    rects(pen, x, yh, f, k, STRIPE, &[(16., 5., 1., 2.), (18., 5., 1., 2.), (20., 5., 1., 1.)]); // forehead stripes
    rects(pen, x, yh, f, k, WHITE, &[(15., 11., 5., 3.), (16., 10., 3., 1.)]); // muzzle
    rects(pen, x, yh, f, k, CHEEK, &[(12., 10., 2., 2.), (21., 10., 2., 2.)]); // blush
    if l.sleep || ((t * 10.) as i64 % 27 == 0 && !l.alert) {
        // asleep, or a blink
        rects(pen, x, yh, f, k, OUTLINE, &[(14., 9., 2., 1.), (20., 9., 2., 1.)]);
    } else {
        rects(pen, x, yh, f, k, OUTLINE, &[(14., 7., 2., 3.), (20., 7., 2., 3.)]); // eyes
        rects(pen, x, yh, f, k, WHITE, &[(14., 7., 1., 1.), (20., 7., 1., 1.), (15., 9., 1., 1.), (21., 9., 1., 1.)]); // sparkles
    }
    rects(pen, x, yh, f, k, CHEEK, &[(17., 10., 2., 1.)]); // nose
    if l.eating || l.alert {
        rects(pen, x, yh, f, k, OUTLINE, &[(16., 11., 4., 3.)]); // open mouth with a tongue
        rects(pen, x, yh, f, k, CHEEK, &[(17., 13., 2., 1.)]);
    } else {
        rects(pen, x, yh, f, k, OUTLINE, &[(16., 11., 1., 1.), (17., 12., 2., 1.), (19., 11., 1., 1.)]);
    }
    if l.swipe {
        // a white paw strikes in front of the face
        rects(pen, x, yh, f, k, OUTLINE, &[(21., 9., 5., 4.)]);
        rects(pen, x, yh, f, k, WHITE, &[(22., 10., 3., 2.)]);
    }
    if l.alert {
        rects(pen, x, yh, f, k, ORANGE, &[(15., -4., 1., 2.), (15., -1., 1., 1.), (21., -4., 1., 2.), (21., -1., 1., 1.)]);
    }
    pen.restore();
}

/// Soft shadow of a cat standing on the ground (x, y: the cat's top left).
fn shadow(pen: &mut Pen, x: f32, y: f32) {
    pen.ellipse(x + CAT_W / 2., y + CAT_H, CAT_W / 2., 5., Color32::from_black_alpha(46));
}

/// Where things are: the logo in the middle, with room above it and a strip of ground below.
struct Stage {
    size: f32,
    w: f32,
    lx: f32,
    ground: f32,
    top: f32,
    sit_y: f32,
    out_y: f32,
}

const ABOVE: f32 = 56.;

impl Stage {
    fn new(size: f32) -> Self {
        let (w, ly) = (size + 300., ABOVE);
        let (ground, top) = (ly + size, ly + 6.);
        Stage { size, w, lx: (w - size) / 2., ground, top, sit_y: top - CAT_H, out_y: ground - CAT_H }
    }
    fn h(&self) -> f32 {
        ABOVE + self.size + 40.
    }
}

/// The colour k (0..1) of the way through the colour ramp of the window.
fn ramp(colors: &[Rgb; 3], k: f32) -> Rgb {
    let k = clamp(k) * 2.;
    let i = (k as usize).min(1);
    let (a, b, f) = (colors[i], colors[i + 1], k - i as f32);
    (lerp(a.0, b.0, f), lerp(a.1, b.1, f), lerp(a.2, b.2, f))
}

// The way in and out that the tube and the scroller share: she comes out from behind the logo, winds up, jumps onto
// it, sits there, and at the end leaps off to the right.
const T_OUT: f32 = 1.3;
const T_JUMP: f32 = 1.7;
const T_LAND: f32 = 2.2;
const LEAP: f32 = 0.6;

/// Until T_LAND: out from behind the logo, the wind-up and the jump onto it. False once she sits.
fn arrive(pen: &mut Pen, st: &Stage, t: f32, sit_x: f32) -> bool {
    if t >= T_LAND {
        return false;
    }
    let x0 = st.lx + st.size + 28.;
    if t < T_OUT {
        // only what is right of the logo's edge shows
        pen.clip(st.lx + st.size, 0., st.w, st.h());
        let x = lerp(st.lx + st.size - CAT_W + 20., x0, t / T_OUT);
        shadow(pen, x, st.out_y);
        nyan(pen, x, st.out_y, t, Look { run: true, ..look() });
        pen.unclip();
    } else if t < T_JUMP {
        shadow(pen, x0, st.out_y);
        let c = clamp((t - T_JUMP + 0.25) / 0.25) * 0.3;
        nyan(pen, x0, st.out_y, t, Look { flip: true, alert: true, sx: 1. + c * 0.6, sy: 1. - c, rot: (t * 45.).sin() * 0.06 * c / 0.3, ..look() });
    } else {
        let f = (t - T_JUMP) / 0.5;
        let (x, y) = jump(f, (x0, st.out_y), (sit_x, st.sit_y), 24.);
        let s = (PI * f).sin();
        nyan(pen, x, y, t, Look { flip: true, run: true, air: true, sx: 1. - 0.15 * s, sy: 1. + 0.25 * s, rot: 0.4 * (PI * f).cos(), ..look() });
    }
    true
}

/// Sitting on the logo: the landing squash, her breathing, and from 0.3 s before t_leap the crouch. Then the leap off
/// to the right until t_leap + LEAP.
fn sit(pen: &mut Pen, st: &Stage, t: f32, sit_x: f32, t_leap: f32, flip: bool, lift: f32, squash: f32, l: Look) {
    if t >= t_leap {
        let f = clamp((t - t_leap) / LEAP);
        let (x, y) = jump(f, (sit_x, st.sit_y), (st.lx + st.size + 160., st.out_y), 30.);
        let s = (PI * f).sin();
        nyan(pen, x, y, t, Look { run: true, air: f <= 0.9, sx: 1. - 0.12 * s, sy: 1. + 0.25 * s, rot: 0.45 * (PI * f).cos(), ..look() });
        return;
    }
    let tau = t - T_LAND;
    let land = 0.3 * (-9. * tau).exp() * (22. * tau).cos();
    let crouch = clamp((t - t_leap + 0.3) / 0.3).powi(2);
    let s = land + crouch * 0.32 + squash + 0.025 * (t * 3.5).sin();
    nyan(pen, sit_x, st.sit_y - lift, t, Look { flip: flip && crouch == 0., sx: 1. + s * 0.6, sy: 1. - s, rot: l.rot - 0.05 * crouch, ..l });
}

/// (lift, squash) of a hop of joy (or of fright, a lower one) that starts at t0.
fn hop(t: f32, t0: f32, height: f32) -> (f32, f32) {
    if !(t0..t0 + 0.4).contains(&t) {
        return (0., 0.);
    }
    let f = (t - t0) / 0.4;
    (height * 4. * f * (1. - f), -0.18 * (PI * f).sin())
}

// ------------------------------------------------------------------------------------------------------------------
// Chase: a mouse dashes along the ground in front of the logo, the cat after her, and her trail is the colour ramp.
// Back again the other way. She never catches it.
const MW: usize = 14;
const MH: usize = 10;

/// The mouse running to the right in MW x MH pixels, as cairo draws it without smoothing: a pixel takes the colour of
/// the last shape over its middle. frame 0 stretches the legs, 1 tucks them.
fn mouse_pixels(frame: usize) -> Vec<Option<Rgb>> {
    let mut px = vec![None; MW * MH];
    let fr = frame as f32;
    let near = |p: (f32, f32), a: (f32, f32), b: (f32, f32), r: f32| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let u = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0., 1.);
        ((p.0 - a.0 - u * dx).powi(2) + (p.1 - a.1 - u * dy).powi(2)).sqrt() <= r
    };
    let tail = [(3., 6.), (1., 6. - fr), (0., 3. + fr)];
    let legs: [((f32, f32), (f32, f32)); 2] = if frame == 0 { [((4., 8.), (2., 9.)), ((9., 8.), (11., 9.))] } else { [((4., 8.), (5., 9.)), ((9., 8.), (9., 9.))] };
    let body = (0.8, 0.8, 0.85);
    let ellipses = [(7., 5.5, 4.2, 2.8, body), (11., 5.5, 2.2, 1.8, body), (9.5, 3., 1.4, 1.4, (1., 0.6, 0.7))];
    for (i, cell) in px.iter_mut().enumerate() {
        let p = ((i % MW) as f32 + 0.5, (i / MW) as f32 + 0.5);
        if tail.windows(2).any(|w| near(p, w[0], w[1], 0.5)) {
            *cell = Some((0.95, 0.7, 0.75)); // the tail flicks
        }
        if legs.iter().any(|(a, b)| near(p, *a, *b, 1.)) {
            *cell = Some(DARK);
        }
        for (cx, cy, rx, ry, c) in ellipses {
            // body, head, ear: each with its outline
            for (g, c) in [(1., DARK), (0., c)] {
                if ((p.0 - cx) / (rx + g)).powi(2) + ((p.1 - cy) / (ry + g)).powi(2) <= 1. {
                    *cell = Some(c);
                }
            }
        }
    }
    px[5 * MW + 13] = Some(CHEEK); // nose
    px[4 * MW + 11] = Some(DARK); // eye
    px
}

/// The mouse with its top left at x, y.
fn mouse(pen: &mut Pen, x: f32, y: f32, flip: bool, t: f32) {
    let frame = (t * 14.) as usize % 2;
    pen.save();
    pen.translate(x + if flip { MW as f32 * CP } else { 0. }, y - frame as f32 * CP);
    pen.scale(if flip { -CP } else { CP }, CP);
    for (i, c) in mouse_pixels(frame).into_iter().enumerate() {
        if let Some(c) = c {
            pen.rect((i % MW) as f32, (i / MW) as f32, 1.02, 1.02, color(c, 1.));
        }
    }
    pen.restore();
}

/// The ramp behind the cat at x, y: six stripes that wave in steps and fade out. Her body is its source.
fn trail(pen: &mut Pen, colors: &[Rgb; 3], x: f32, y: f32, flip: bool, t: f32) {
    let (length, band) = (110, 4.);
    for d in (0..length).step_by(6) {
        let (k, d) = (d as f32 / length as f32, d as f32);
        let wave = 3. * if (t * 12. - d / 14.).sin() > 0. { 1. } else { -1. };
        for i in 0..6 {
            let bx = if flip { x + 21. * CP + d } else { x + 3. * CP - d - 6. };
            pen.rect(bx, y + 16. + i as f32 * band + wave, 6., band, color(ramp(colors, i as f32 / 5.), (1. - k).powf(1.5)));
        }
    }
}

const SPEED: f32 = 230.;
const LAG: f32 = 0.75;
const LEGS: [(f32, f32); 2] = [(0.2, 1.), (3.6, -1.)]; // when the mouse starts, and which way

fn chase_end(st: &Stage) -> f32 {
    3.6 + LAG + (st.w + CAT_W) / SPEED + 0.1
}

fn chase(pen: &mut Pen, st: &Stage, colors: &[Rgb; 3], t: f32) {
    for (start, d) in LEGS {
        for (cat, begin, width) in [(false, start, MW as f32 * CP), (true, start + LAG, CAT_W)] {
            let run = (t - begin) * SPEED;
            if !(0. ..=st.w + width).contains(&run) {
                continue;
            }
            let x = if d > 0. { run - width } else { st.w - run };
            if cat {
                shadow(pen, x, st.out_y);
                trail(pen, colors, x, st.out_y, d < 0., t);
                nyan(pen, x, st.out_y, t, Look { flip: d < 0., run: true, no_lines: true, ..look() });
            } else {
                mouse(pen, x, st.ground - MH as f32 * CP, d < 0., t);
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// Tube: a little CRT rises from behind the logo. The cat sits down beside it, taps the switch, the picture jumps on as
// a bright line that opens up, shows block art in the ramp colours with a glow, flickers and gets her scared, and goes
// off in a dot.
const LETTERS: [[&str; 5]; 3] = [["X...X", "XX..X", "X.X.X", "X..XX", "X...X"], ["XXXXX", "X....", "XXXX.", "X....", "X...."], [".XXX.", "X...X", "X...X", "X...X", ".XXX."]]; // N, F, O
const TEXT_LINES: [f32; 5] = [36., 30., 38., 24., 33.];
const M_W: f32 = 56.;
const M_H: f32 = 52.;
const S_X: f32 = 5.;
const S_Y: f32 = 5.;
const S_W: f32 = 46.;
const S_H: f32 = 32.;
const C_RISE: f32 = 0.3;
const C_PRESS: f32 = 2.45;
const C_ON: f32 = 2.8;
const C_FLICK: f32 = 4.5;
const C_OFF: f32 = 5.6;
const C_SINK: f32 = 6.1;
const C_LEAP: f32 = 6.5;
const C_END: f32 = 7.3;

/// The picture of the screen in its own 46 x 32 pixels, at x, y.
fn screen_picture(pen: &mut Pen, colors: &[Rgb; 3], x: f32, y: f32, t: f32) {
    pen.rect(x, y, S_W, S_H, color((0.02, 0.03, 0.05), 1.));
    for (c, letter) in LETTERS.iter().enumerate() {
        for (r, row) in letter.iter().enumerate() {
            for (q, ch) in row.chars().enumerate() {
                if ch == 'X' {
                    pen.rect(x + 2. + (c * 6 + q) as f32 * 2.5, y + 4. + r as f32 * 2.5, 2.5, 2.5, color(ramp(colors, r as f32 / 4.), 1.));
                }
            }
        }
    }
    for (i, width) in TEXT_LINES.iter().enumerate() {
        pen.rect(x + 4., y + 20. + i as f32 * 2.4, *width, 1.4, color((0.45, 0.62, 0.7), 1.));
    }
    for line in (0..S_H as usize).step_by(2) {
        pen.rect(x, y + line as f32, S_W, 1., Color32::from_black_alpha(77)); // scanlines
    }
    if (C_FLICK..C_FLICK + 0.5).contains(&t) {
        // flicker and a bright band that rolls down
        if (t * 70.).sin() > 0. {
            pen.rect(x, y, S_W, S_H, Color32::from_black_alpha(140));
        }
        pen.rect(x, y + ((t - C_FLICK) * 3.).rem_euclid(1.) * S_H, S_W, 5., Color32::from_white_alpha(64));
    }
}

fn tube(pen: &mut Pen, st: &Stage, colors: &[Rgb; 3], t: f32) {
    let (lx, top) = (st.lx, st.top);
    let sit_x = lx + st.size - CAT_W + 6.;
    let mx = lx + 4.;
    let up = 1. - (1. - clamp((t - C_RISE) / 0.7)).powi(3);
    let y = top - M_H * lerp(up, 0., clamp((t - C_SINK) / 0.4).powi(2));
    // (width, height, brightness) of the lit part of the screen, 0..1: the line, the opening and the dot of a CRT
    let (on, off) = (t - C_ON, t - C_OFF);
    let (pw, ph, bright) = if on < 0. {
        (0., 0., 0.)
    } else if off >= 0. {
        if off < 0.2 {
            (1., lerp(1., 0.06, off / 0.2), 1.)
        } else if off < 0.35 {
            (lerp(1., 0.06, (off - 0.2) / 0.15), 0.06, 1.)
        } else {
            (0.06, 0.06, (1. - (off - 0.35) / 0.15).max(0.))
        }
    } else if on < 0.12 {
        (on / 0.12, 0.06, 1.)
    } else {
        (1., lerp(0.06, 1., clamp((on - 0.12) / 0.28)), 1.)
    };
    if y < top - 1. {
        pen.clip(mx - 24., 0., M_W + 48., top); // comes from behind the logo
        if bright > 0. {
            // the glow around the screen, a soft frame of the ramp's middle colour
            let pulse = 0.8 + 0.2 * (t * 5.).sin();
            for i in 1..=5 {
                let g = 3. * i as f32;
                pen.rect(mx + S_X - g, y + S_Y - g, S_W + 2. * g, S_H + 2. * g, color(ramp(colors, 0.5), 0.09 * pulse * bright));
            }
        }
        pen.rect(mx + 22., y + 44., 12., 6., color(DARK, 1.)); // neck, base, body
        pen.rect(mx + 14., y + 49., 28., 3., color(DARK, 1.));
        pen.rect(mx, y, M_W, 45., color((0.3, 0.32, 0.38), 1.));
        pen.rect(mx + 2., y + 2., M_W - 4., 1., color((0.45, 0.47, 0.54), 1.));
        pen.rect(mx + M_W - 8., y + 39., 3., 3., color(if bright > 0. { (0.3, 0.9, 0.4) } else { (0.55, 0.2, 0.2) }, 1.)); // the power light
        pen.rect(mx + S_X, y + S_Y, S_W, S_H, color((0.03, 0.04, 0.06), 1.));
        if bright > 0. {
            let (lw, lh) = (S_W * pw, (S_H * ph).max(2.));
            pen.clip(mx + S_X + (S_W - lw) / 2., y + S_Y + (S_H - lh) / 2., lw, lh);
            if ph < 0.1 {
                pen.rect(mx + S_X, y + S_Y, S_W, S_H, color(WHITE, bright));
            } else {
                screen_picture(pen, colors, mx + S_X, y + S_Y, t);
            }
        }
        pen.unclip();
    }
    if arrive(pen, st, t, sit_x) {
        return;
    }
    let (lift, squash) = if t >= C_LEAP { (0., 0.) } else if t < C_OFF { hop(t, C_FLICK, 12.) } else { hop(t, C_OFF + 0.35, 24.) };
    let scared = (C_FLICK..C_FLICK + 0.6).contains(&t);
    let pawing = (C_PRESS..C_PRESS + 0.4).contains(&t) && ((t - C_PRESS) / 0.12) as i64 % 2 == 0;
    let rot = if scared { 0.07 * (t * 45.).sin() } else { 0. };
    sit(pen, st, t, sit_x, C_LEAP, true, lift, squash, Look { swipe: pawing, alert: scared, rot, ..look() });
}

// ------------------------------------------------------------------------------------------------------------------
// Scroller: a demoscene sine scroller wanders over the logo. She sits and reads along, and hops for joy when her own
// name comes by.
const TEXT: &str = "GREETINGS FROM CODE-CAT";
const SCROLL_START: f32 = 1.5;
const SCROLL_SPEED: f32 = 150.;
const HEART: [&str; 6] = [".pp.pp.", "ppppppp", "ppppppp", ".ppppp.", "..ppp..", "...p..."];
const HEARTS: f32 = 1.2; // seconds that the hearts rise

/// The times of the scroller: (her name above her head, half the text, the leap, the end), for letters cw wide.
fn scroller_times(st: &Stage, cw: f32) -> (f32, f32, f32, f32) {
    let sit_x = st.lx + (st.size - CAT_W) / 2.;
    let head_x = sit_x + 36.;
    let reaches = |i: usize, x: f32| SCROLL_START + (st.w + i as f32 * cw - x) / SCROLL_SPEED; // when letter i is at x
    let (t_name, t_turn) = (reaches(TEXT.find("CODE-CAT").unwrap(), head_x), reaches(TEXT.len() / 2, head_x));
    let t_leap = t_name + 1.5;
    (t_name, t_turn, t_leap, (t_leap + LEAP + 0.3).max(reaches(TEXT.len(), 0.)))
}

fn scroller(pen: &mut Pen, st: &Stage, colors: &[Rgb; 3], font: &FontId, cw: f32, t: f32) {
    let sit_x = st.lx + (st.size - CAT_W) / 2.;
    let (t_name, t_turn, t_leap, _) = scroller_times(st, cw);
    for (i, ch) in TEXT.chars().enumerate() {
        let x = st.w + i as f32 * cw - SCROLL_SPEED * (t - SCROLL_START);
        if ch == ' ' || x < -cw || x > st.w {
            continue;
        }
        let y = 5. + 4. * (x / 30. + t * 6.).sin();
        pen.text(x + 1., y + 1., ch, font, Color32::from_black_alpha(140));
        pen.text(x, y, ch, font, color(ramp(colors, i as f32 / (TEXT.len() - 1) as f32), 1.));
    }
    if arrive(pen, st, t, sit_x) {
        return;
    }
    let (lift, mut squash) = hop(t, if t < t_name + 0.45 { t_name } else { t_name + 0.45 }, 24.); // two hops of joy
    if t < t_name {
        squash += 0.04 * (t * 6.).sin(); // until then she nods along with the wave
    }
    let happy = (t_name..t_name + HEARTS).contains(&t); // eyes shut with joy, hearts drift up from her head
    sit(pen, st, t, sit_x, t_leap, t >= t_turn, lift, squash, Look { sleep: happy, ..look() });
    if happy {
        for i in 0..3 {
            let f = ((t - t_name) / HEARTS + i as f32 / 3.).rem_euclid(1.);
            let k = 0.45 + 0.3 * f;
            let side = if i % 2 == 1 { 1. } else { -1. } * if i == 2 { 0.4 } else { 1. };
            let (hx, hy) = (sit_x + CAT_W * 0.27 + (14. + 26. * f) * side - 3.5 * 4. * k, st.sit_y + 6. - 16. * f);
            for (r, row) in HEART.iter().enumerate() {
                for (c, ch) in row.chars().enumerate() {
                    if ch == 'p' {
                        pen.rect(hx + c as f32 * P * k, hy + r as f32 * P * k, P * k + 0.5, P * k + 0.5, color((1., 0.45, 0.6), 1.));
                    }
                }
            }
        }
    }
}

const SCENES: usize = 3; // chase, tube, scroller

/// How long a scene runs.
fn scene_end(scene: usize, st: &Stage, cw: f32) -> f32 {
    match scene {
        0 => chase_end(st),
        1 => C_END,
        _ => scroller_times(st, cw).3,
    }
}

/// The scene at t seconds, on a stage whose top left is at origin.
fn draw_scene(painter: &Painter, origin: Pos2, scene: usize, colors: &[Rgb; 3], t: f32) {
    let st = Stage::new(LOGO);
    let font = FontId::monospace(16.);
    let cw = painter.ctx().fonts_mut(|f| f.glyph_width(&font, 'M'));
    let mut pen = Pen { painter, origin: origin.to_vec2(), m: [1., 0., 0., 1., 0., 0.], stack: Vec::new(), mesh: Mesh::default(), clip: None };
    match scene {
        0 => chase(&mut pen, &st, colors, t),
        1 => tube(&mut pen, &st, colors, t),
        _ => scroller(&mut pen, &st, colors, &font, cw, t),
    }
}

/// The About window while it is open: the egg's state lives with it.
#[derive(Default)]
pub struct About {
    clicks: (u32, f64), // how many quick clicks on the logo, and when the last one was
    hint: Option<(usize, f64)>, // the note that counts down the last clicks, and when it appeared
    playing: Option<(usize, f64)>, // a scene and when it began
    last: Option<usize>,
    seen: bool, // the cat joins the credits
    page: Option<usize>, // a page under the main one (DETAILS, CREDITS, LEGAL, THANKS)
}

const DETAILS: usize = 0;
const CREDITS: usize = 1;
const LEGAL: usize = 2;
const THANKS: usize = 3;
const WEBSITE: usize = 4;
const ISSUE: usize = 5;
const DEVELOPER: usize = 6;

/// The words of the About window that NfoGlow for Linux takes from libadwaita (so its .po files lack them): Details,
/// Credits, Legal, Acknowledgements, Website, Report an Issue, Developer.
fn word(which: usize) -> &'static str {
    const WORDS: [(&str, [&str; 7]); 10] = [
        ("en", ["Details", "Credits", "Legal", "Acknowledgements", "Website", "Report an Issue", "Developer"]),
        ("de", ["Details", "Mitwirkende", "Rechtliches", "Danksagungen", "Webseite", "Fehler melden", "Entwickler"]),
        ("es", ["Detalles", "Créditos", "Legal", "Agradecimientos", "Sitio web", "Informar de un problema", "Desarrollador"]),
        ("fr", ["Détails", "Crédits", "Mentions légales", "Remerciements", "Site web", "Signaler un problème", "Développeur"]),
        ("it", ["Dettagli", "Riconoscimenti", "Note legali", "Ringraziamenti", "Sito web", "Segnala un problema", "Sviluppatore"]),
        ("ja", ["詳細", "クレジット", "法的情報", "謝辞", "ウェブサイト", "問題を報告", "開発者"]),
        ("pl", ["Szczegóły", "Autorzy", "Informacje prawne", "Podziękowania", "Strona internetowa", "Zgłoś błąd", "Twórca"]),
        ("pt_BR", ["Detalhes", "Créditos", "Informações legais", "Agradecimentos", "Site", "Relatar um problema", "Desenvolvedor"]),
        ("uk", ["Подробиці", "Автори", "Правова інформація", "Подяки", "Вебсайт", "Повідомити про проблему", "Розробник"]),
        ("zh_CN", ["详细信息", "制作人员", "法律信息", "致谢", "网站", "报告问题", "开发者"]),
    ];
    let words = WORDS.iter().find(|w| w.0 == crate::i18n::current()).unwrap_or(&WORDS[0]);
    words.1[which]
}

/// A page under the main one: a way back and its rows.
fn about_page(ui: &mut egui::Ui, page: usize, seen: bool, open: &mut Option<usize>) {
    ui.horizontal(|ui| {
        if ui::icon_button(ui, ui::BACK, &tr("Back"), false).clicked() {
            *open = None;
        }
        ui.label(RichText::new(word(page)).size(18.).strong());
    });
    ui.add_space(12.);
    let stennish = ("Stennish", "https://soundcloud.com/autistgamer1337/sets/chiptunes");
    let rows: Vec<(String, (&str, &str))> = match page {
        CREDITS => [(word(DEVELOPER).to_string(), ("DigitGram", "https://github.com/DigitGram")), (tr("Music (CC BY 4.0)"), stennish)]
            .into_iter()
            .chain(seen.then(|| (tr("Mascot"), ("Code-Cat", ""))))
            .collect(),
        LEGAL => vec![
            ("© 2026 DigitGram".into(), ("GPL-3.0-or-later", "https://www.gnu.org/licenses/gpl-3.0.html")),
            (tr("DOS font"), ("VileR · CC BY-SA 4.0", "https://int10h.org/oldschool-pc-fonts/")),
            (tr("Music (CC BY 4.0)"), stennish),
        ],
        THANKS => vec![(tr("Inspired by"), ("iNFekt", "https://github.com/syndicodefront/infekt")), (tr("Thank you for the music"), stennish)],
        _ => {
            ui.add(egui::Label::new(tr("A viewer for NFO and ANSI art, the way the scene meant it: crisp block graphics with an optional soft glow, CP437 out of the box, clickable links and a text-only view.")).wrap());
            return;
        }
    };
    ui::card(ui, |ui| {
        for (i, (title, (name, url))) in rows.iter().enumerate() {
            if i > 0 {
                ui::divider(ui);
            }
            ui::row(ui, title, None, true, |ui| if url.is_empty() { ui.label(*name) } else { ui.hyperlink_to(*name, *url) });
        }
    });
}

impl App {
    pub(crate) fn about_window(&mut self, ctx: &egui::Context) {
        if self.about.is_none() {
            return;
        }
        let logo = self.logo_texture(ctx);
        let about = self.about.as_mut().unwrap();
        let ramp_colors = [0, 1, 2].map(|i| {
            let c = crate::core::hex(self.cfg["ramp_colors"][i].as_str().unwrap_or("#888888"));
            (c[0] as f32 / 255., c[1] as f32 / 255., c[2] as f32 / 255.)
        });
        let st = Stage::new(LOGO);
        let builder = egui::ViewportBuilder::default().with_title(tr("About NfoGlow")).with_inner_size([st.w + 40., 640.]).with_resizable(false).with_icon(crate::window_icon());
        let mut close = false;
        ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("about"), builder, |ui, _class| {
            close = ui.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape));
            let now = ui.input(|i| i.time);
            egui::CentralPanel::default().frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(20)).show(ui, |ui| {
                if let Some(page) = about.page {
                    about_page(ui, page, about.seen, &mut about.page);
                    return;
                }
                ui.vertical_centered(|ui| {
                    ui.add_space(ABOVE - 20.);
                    let response = ui.add(egui::Image::new((logo.id(), Vec2::splat(LOGO))).sense(egui::Sense::click()));
                    // five quick clicks play one of the scenes, never the same twice in a row
                    if response.clicked() {
                        // as in NfoGlow for Linux: our own count, slow clicking (one every 1.5 s at most) counts too
                        let n = if now - about.clicks.1 < 1.5 { about.clicks.0 + 1 } else { 1 };
                        about.clicks = (n, now);
                        about.hint = (2..=4).contains(&n).then_some((n as usize - 2, now)); // like Android's developer options
                        if n == 5 {
                            about.clicks.0 = 0;
                            about.hint = None;
                            if about.playing.is_none() {
                                let pick = (hash(&[now, 17.]) * SCENES as f32) as usize % SCENES;
                                let scene = if Some(pick) == about.last { (pick + 1) % SCENES } else { pick };
                                (about.playing, about.last, about.seen) = (Some((scene, now)), Some(scene), true);
                            }
                        }
                    }
                    if let Some((i, since)) = about.hint {
                        if now - since < 1.2 {
                            let text = tr(["3 more clicks …", "2 more clicks …", "1 more click …"][i]);
                            egui::Area::new(egui::Id::new("about-hint")).order(egui::Order::Tooltip).fixed_pos(response.rect.center_top() - Vec2::new(40., 34.)).show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(text));
                            });
                            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
                        } else {
                            about.hint = None;
                        }
                    }
                    if let Some((scene, start)) = about.playing {
                        let t = (now - start) as f32;
                        let cw = ui.ctx().fonts_mut(|f| f.glyph_width(&FontId::monospace(16.), 'M'));
                        if t > scene_end(scene, &st, cw) {
                            about.playing = None;
                        } else {
                            let origin = response.rect.min - Vec2::new(st.lx, ABOVE);
                            let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("about-stage"));
                            draw_scene(&ui.ctx().layer_painter(layer), origin, scene, &ramp_colors, t);
                            ui.ctx().request_repaint();
                        }
                    }
                    ui.add_space(8.);
                    ui.label(RichText::new("NfoGlow").size(24.).strong());
                    ui.label("DigitGram");
                    ui.add_space(6.);
                    // the version as a small badge, as in the About dialog of GNOME
                    let badge = egui::Button::new(RichText::new(env!("CARGO_PKG_VERSION")).color(ui::accent(ui))).fill(ui.visuals().widgets.inactive.weak_bg_fill).corner_radius(10).sense(egui::Sense::hover());
                    ui.add(badge);
                    ui.add_space(18.);
                    let web = "https://github.com/DigitGram/nfoglow-windows";
                    ui::card(ui, |ui| {
                        let links: [(&str, ui::Icon, Option<usize>, Option<String>); 6] = [
                            (word(DETAILS), ui::RIGHT, Some(DETAILS), None),
                            (word(WEBSITE), ui::LINK, None, Some(web.into())),
                            (word(ISSUE), ui::LINK, None, Some(format!("{web}/issues"))),
                            (word(CREDITS), ui::RIGHT, Some(CREDITS), None),
                            (word(LEGAL), ui::RIGHT, Some(LEGAL), None),
                            (word(THANKS), ui::RIGHT, Some(THANKS), None),
                        ];
                        for (i, (title, sign, page, url)) in links.into_iter().enumerate() {
                            if i > 0 {
                                ui::divider(ui);
                            }
                            if ui::nav_row(ui, title, sign).clicked() {
                                match (page, url) {
                                    (Some(page), _) => about.page = Some(page),
                                    (_, Some(url)) => ui.ctx().open_url(egui::OpenUrl::new_tab(url)),
                                    _ => {}
                                }
                            }
                        }
                    });
                });
            });
        });
        if close {
            self.about = None;
        }
    }
}
