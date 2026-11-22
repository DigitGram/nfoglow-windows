// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! Smooth edges for the block graphics, for those who hate pixels. The art is read as the pixel art it is, enlarged by
//! Scale2x (EPX) so that staircases become diagonals and curves become round, halved (the finest steps become
//! anti-aliasing), blurred a little, scaled to the screen's pixels and cut off steeply again. The shades (░▒▓) belong
//! to the outline and fade softly into the whole blocks inside it. Ported from page.py of NfoGlow for Linux.
//! The result is the alpha of the blocks at the resolution of the screen; the view colours it in.
use crate::core::block;
use crate::page::Page;

/// (Scale2x steps, blur passes) of low, mid and high.
pub fn amount(name: &str) -> (u32, u32) {
    match name {
        "low" => (3, 0),
        "high" => (3, 10),
        _ => (3, 3),
    }
}

struct Grid {
    w: usize,
    h: usize,
    v: Vec<u8>,
}

/// The art as pixels (0/1) and how strong each is (1, or the alpha of a shade), a pixel being half a cell high and a
/// cell wide, or half a cell wide when some block (▌ ▐ ▖ ...) needs it. Returns (grid, strength, pixels per cell across).
fn art_pixels(page: &Page) -> (Grid, Vec<f32>, usize) {
    let rects = |ch| block(ch).into_iter().flat_map(|(rs, a)| rs.into_iter().map(move |r| (r, a)));
    let wide = page.lines.iter().flatten().flat_map(|&c| rects(c)).any(|(r, _)| r[0].fract() != 0. || r[2].fract() != 0.);
    let gx = if wide { 2 } else { 1 };
    let (w, h) = (page.cols * gx, page.lines.len() * 2);
    let mut grid = Grid { w, h, v: vec![0; w * h] };
    let mut strength = vec![1f32; w * h];
    for (r, line) in page.lines.iter().enumerate() {
        for (c, &ch) in line.iter().enumerate() {
            for (q, alpha) in rects(ch) {
                let xa = ((c as f32 + q[0]) * gx as f32).round() as usize;
                let xb = (((c as f32 + q[2]) * gx as f32).round() as usize).max(xa + 1);
                let (ya, yb) = ((2. * r as f32 + 2. * q[1]).round() as usize, (2. * r as f32 + 2. * q[3]).round() as usize); // a thin ▁ is a whole pixel row
                for y in ya..yb.min(h) {
                    for x in xa..xb.min(w) {
                        grid.v[y * w + x] = 1;
                        if alpha < 1. {
                            strength[y * w + x] = alpha;
                        }
                    }
                }
            }
        }
    }
    (grid, strength, gx)
}

/// One Scale2x step: every pixel P becomes four, from its neighbours above (A), right (B), left (C) and below (D).
fn scale2x(g: &Grid) -> Grid {
    let (w, h) = (g.w, g.h);
    let mut out = Grid { w: w * 2, h: h * 2, v: vec![0; w * h * 4] };
    let at = |x: isize, y: isize| if x < 0 || y < 0 || x >= w as isize || y >= h as isize { 0 } else { g.v[y as usize * w + x as usize] };
    for y in 0..h {
        for x in 0..w {
            let (xi, yi) = (x as isize, y as isize);
            let (p, a, b, c, d) = (at(xi, yi), at(xi, yi - 1), at(xi + 1, yi), at(xi - 1, yi), at(xi, yi + 1));
            let four = if c == a && c != d && a != b { a } else { p };
            let one = if a == b && a != c && b != d { b } else { p };
            let three = if d == c && d != b && c != a { c } else { p };
            let two = if b == d && b != a && d != c { d } else { p };
            let o = y * 2 * out.w + x * 2;
            out.v[o] = four;
            out.v[o + 1] = one;
            out.v[o + out.w] = three;
            out.v[o + out.w + 1] = two;
        }
    }
    out
}

/// One [1 2 1] / 4 pass across and down, the edge pixels repeated.
fn blur(v: &mut [f32], w: usize, h: usize) {
    let mut row = vec![0f32; w.max(h)];
    for y in 0..h {
        let line = &mut v[y * w..(y + 1) * w];
        for x in 0..w {
            row[x] = (line[x.saturating_sub(1)] + 2. * line[x] + line[(x + 1).min(w - 1)]) / 4.;
        }
        line.copy_from_slice(&row[..w]);
    }
    for x in 0..w {
        for y in 0..h {
            row[y] = (v[y.saturating_sub(1) * w + x] + 2. * v[y * w + x] + v[(y + 1).min(h - 1) * w + x]) / 4.;
        }
        for y in 0..h {
            v[y * w + x] = row[y];
        }
    }
}

/// The value of an image at (x, y) given in its own pixels, bilinear, the edges repeated.
fn sample(v: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let (x, y) = ((x - 0.5).clamp(0., (w - 1) as f32), (y - 0.5).clamp(0., (h - 1) as f32));
    let (x0, y0) = (x as usize, y as usize);
    let (x1, y1, fx, fy) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1), x.fract(), y.fract());
    let top = v[y0 * w + x0] * (1. - fx) + v[y0 * w + x1] * fx;
    let bottom = v[y1 * w + x0] * (1. - fx) + v[y1 * w + x1] * fx;
    top * (1. - fy) + bottom * fy
}

/// The alpha of the art at out_w x out_h screen pixels (the whole page), as 0..255; None when the page has no art.
pub fn alpha(page: &Page, out_w: usize, out_h: usize, (steps, passes): (u32, u32)) -> Option<Vec<u8>> {
    let (mut grid, strength, gx) = art_pixels(page);
    if grid.w == 0 || grid.h == 0 || out_w == 0 || out_h == 0 || !grid.v.contains(&1) {
        return None;
    }
    // a big page: fewer steps (about 16 million pixels at most before the last one)
    let steps = steps.min(((16e6 / (grid.w * grid.h) as f64).log2() / 2.).max(0.) as u32);
    for _ in 0..steps {
        grid = scale2x(&grid);
    }
    // halved, each pixel the average of 2 x 2 (without a step there is nothing to halve)
    let half = steps > 0;
    let (sw, sh) = if half { (grid.w.div_ceil(2), grid.h.div_ceil(2)) } else { (grid.w, grid.h) };
    let mut small = vec![0f32; sw * sh];
    if half {
        for y in 0..sh {
            for x in 0..sw {
                let get = |dx: usize, dy: usize| *grid.v.get((2 * y + dy).min(grid.h - 1) * grid.w + (2 * x + dx).min(grid.w - 1)).unwrap_or(&0) as f32;
                small[y * sw + x] = (get(0, 0) + get(1, 0) + get(0, 1) + get(1, 1)) / 4.;
            }
        }
    } else {
        for (s, &v) in small.iter_mut().zip(&grid.v) {
            *s = v as f32;
        }
    }
    drop(grid);
    for _ in 0..passes {
        blur(&mut small, sw, sh);
    }
    // device pixels per sample, and the steepness of the cut: an edge blurred by sigma rises by about .4 / sigma per
    // device pixel, steepened to ramp over about 1.4 pixels
    let per = (if half { (1usize << steps) / 2 } else { 1 }) as f32;
    let (px, py) = (out_w as f32 / (page.cols * gx) as f32 / per, out_h as f32 / (page.lines.len() * 2) as f32 / per);
    let gain = (1.8 * px.max(py) * (0.5 * passes as f32 + 0.3).sqrt()).max(1.);
    let shades = strength.iter().any(|s| *s < 1.).then_some(strength);
    let (gw, gh) = (page.cols * gx, page.lines.len() * 2);
    let mut out = vec![0u8; out_w * out_h];
    for y in 0..out_h {
        let v = (y as f32 + 0.5) / py;
        for x in 0..out_w {
            let u = (x as f32 + 0.5) / px;
            let a = ((sample(&small, sw, sh, u, v) - 0.5) * gain + 0.5).clamp(0., 1.);
            // the shades are part of the outline: they only weaken it inside
            let s = shades.as_ref().map_or(1., |s| sample(s, gw, gh, u / per, v / per));
            out[y * out_w + x] = (a * s * 255.).round() as u8;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(text: &str) -> Page {
        Page::fill(text.as_bytes(), "auto", false)
    }

    #[test]
    fn a_staircase_gets_in_between_tones_and_a_full_block_stays_solid() {
        let stairs = page("█\n██\n███\n████");
        let (w, h) = (4 * 16, 4 * 32);
        let sharp = alpha(&stairs, w, h, (0, 0)).unwrap();
        let mid = alpha(&stairs, w, h, amount("mid")).unwrap();
        assert!(mid.iter().any(|a| *a > 0 && *a < 255)); // anti-aliasing
        assert_ne!(sharp, mid);
        let low = alpha(&stairs, w, h, amount("low")).unwrap();
        let high = alpha(&stairs, w, h, amount("high")).unwrap();
        assert!(low != mid && mid != high);
        let solid = alpha(&page("████\n████\n████"), 64, 96, amount("high")).unwrap();
        assert_eq!(solid[solid.len() / 2], 255); // the middle of a field is not eaten
        assert!(alpha(&page("no art here"), 100, 20, amount("mid")).is_none());
    }

    #[test]
    fn shades_fade_inside_the_outline() {
        let a = alpha(&page("███\n█▒█\n███"), 48, 96, amount("mid")).unwrap();
        assert!(a[48 * 48 + 24] < 200 && a[48 * 48 + 24] > 60); // the ▒ in the middle: about half
    }
}
