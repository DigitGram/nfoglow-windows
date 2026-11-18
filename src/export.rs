// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! The whole page as a picture, saved as PNG or PDF: the background, the glow, the blocks with their colours, the
//! text (the very glyphs the window shows, taken from egui's font image) and the CRT screen. What moves stands
//! still at its first moment: colour cycling, the stars, the wobble. As in NfoGlow for Linux: the text glow and the
//! tube's colour mask are there; no sparkle, gleam or colour fringe.
use crate::core::{self, Cfg, Rgb, Theme};
use crate::page::Page;
use crate::view::{self, Bands};
use eframe::egui::{self, FontId, Rect};
use std::io::Write;

pub const MAX_PNG: usize = 32767; // many programs cannot open a wider or higher image
const MAX_PIXELS: usize = 150_000_000; // about 450 MB while it is made

pub struct Picture {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
}

impl Picture {
    fn blend(&mut self, x: i64, y: i64, color: [f32; 3], alpha: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let at = (y as usize * self.w + x as usize) * 3;
            for i in 0..3 {
                self.rgb[at + i] = (color[i] * alpha + self.rgb[at + i] as f32 * (1. - alpha)).round() as u8;
            }
        }
    }

    fn fill(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, color: Rgb, alpha: f32) {
        for y in y0.max(0)..y1.min(self.h as i64) {
            for x in x0.max(0)..x1.min(self.w as i64) {
                self.blend(x, y, color.map(f32::from), alpha);
            }
        }
    }
}

/// A small blurred image (premultiplied, as view::bake makes them) scaled up smoothly onto the picture: k pixels of
/// the picture per image pixel, its corner at (x0, y0); shift: how far a row of pixels slides sideways (the wobble).
fn lay(pic: &mut Picture, image: &egui::ColorImage, k: f32, x0: f32, y0: f32, alpha: f32, shift: &dyn Fn(i64) -> i64) {
    let [iw, ih] = image.size;
    let texel = |px: i64, py: i64| image.pixels[py.clamp(0, ih as i64 - 1) as usize * iw + px.clamp(0, iw as i64 - 1) as usize].to_array().map(f32::from);
    let (w, h) = (pic.w, pic.h);
    for py in 0..(ih as f32 * k) as i64 {
        let v = (py as f32 + 0.5) / k - 0.5;
        let dy = py + y0 as i64;
        let slide = shift(dy);
        for px in 0..(iw as f32 * k) as i64 {
            let u = (px as f32 + 0.5) / k - 0.5;
            let (fx, fy) = (u - u.floor(), v - v.floor());
            let (a, b, c, d) = (texel(u.floor() as i64, v.floor() as i64), texel(u.floor() as i64 + 1, v.floor() as i64), texel(u.floor() as i64, v.floor() as i64 + 1), texel(u.floor() as i64 + 1, v.floor() as i64 + 1));
            let mixed = [0, 1, 2, 3].map(|i| (a[i] * (1. - fx) + b[i] * fx) * (1. - fy) + (c[i] * (1. - fx) + d[i] * fx) * fy);
            if mixed[3] > 0. {
                // the image has its colours multiplied by its alpha already
                let dx = px + x0 as i64 + slide;
                if dx >= 0 && dy >= 0 && (dx as usize) < w && (dy as usize) < h {
                    let at = (dy as usize * w + dx as usize) * 3;
                    for i in 0..3 {
                        pic.rgb[at + i] = (mixed[i] * alpha + pic.rgb[at + i] as f32 * (1. - mixed[3] / 255. * alpha)).round().min(255.) as u8;
                    }
                }
            }
        }
    }
}

/// The starfield as the window shows it at its first moment, over the whole picture (s: pixels per point).
fn stars(pic: &mut Picture, warp: bool, color: Rgb, zoom: f32, s: f32) {
    let (w, h) = (pic.w as f32, pic.h as f32);
    let hash = view::hash;
    if warp {
        let reach = w.min(h) / 4.;
        let (cx, cy) = (w / 2., h / 2.);
        for i in 0..((w * h / s / s / 2500.) as usize).max(60) {
            let (x, y, z0) = (hash(&[i as f64, 1.]) * 2. - 1., hash(&[i as f64, 2.]) * 2. - 1., hash(&[i as f64, 3.]));
            let z = z0.rem_euclid(1.) + 0.02;
            let near = (1. - z).max(0.);
            let (width, alpha) = ((near * 2.4).max(0.6) * s, (near * 1.3).min(1.));
            let ((ax, ay), (bx, by)) = ((cx + x / (z + 0.06) * reach, cy + y / (z + 0.06) * reach), (cx + x / z * reach, cy + y / z * reach));
            // the streak as the pixels it covers, each blended once
            let mut covered = std::collections::HashSet::new();
            let steps = ((bx - ax).hypot(by - ay) * 2.).ceil().max(1.) as usize;
            for k in 0..=steps {
                let (px, py) = (ax + (bx - ax) * k as f32 / steps as f32, ay + (by - ay) * k as f32 / steps as f32);
                for dy in 0..width.ceil() as i64 {
                    for dx in 0..width.ceil() as i64 {
                        covered.insert(((px - width / 2.) as i64 + dx, (py - width / 2.) as i64 + dy));
                    }
                }
            }
            for (px, py) in covered {
                pic.blend(px, py, color.map(f32::from), alpha);
            }
        }
        return;
    }
    for layer in 0..3 {
        let (size, alpha) = ([1., 1.5, 2.][layer] * zoom.max(1.) * s, [0.35, 0.6, 0.95][layer]);
        for i in 0..((w * h / s / s / 9000.) as usize).max(30) {
            let (x, y) = (hash(&[i as f64, layer as f64, 1.]) * w, hash(&[i as f64, layer as f64, 2.]) * h);
            pic.fill(x.round() as i64, y.round() as i64, (x + size).round() as i64, (y + size).round() as i64, color, alpha);
        }
    }
}

/// The page as a picture, at the zoom and the screen scale of the window (cw, ch: the size of a character there).
pub fn render(ctx: &egui::Context, page: &Page, cfg: &Cfg, theme: &Theme, font: &FontId, cw: f32, ch: f32, plain: bool) -> Result<Picture, String> {
    let s = ctx.pixels_per_point();
    let (cw, ch) = (cw * s, ch * s); // from here on in pixels
    let (ox, oy) = ((cw * 2.).round(), (ch * 1.5).round());
    let (w, h) = ((page.cols as f32 * cw + 2. * ox).ceil().max(1.) as usize, (page.lines.len() as f32 * ch + 2. * oy).ceil().max(1.) as usize);
    if w * h > MAX_PIXELS {
        return Err(format!("{w} × {h}"));
    }
    let ansi = page.colors.is_some();
    let bg = if ansi { core::VGA[0] } else { theme.bg };
    let mut pic = Picture { w, h, rgb: bg.repeat(w * h) };
    // whole-pixel edges shared by neighbours: no seams
    let (x, y) = (|c: f32| (ox + c * cw).round() as i64, |r: f32| (oy + r * ch).round() as i64);
    if let Some(colors) = &page.colors {
        for (r, row) in colors.iter().enumerate() {
            for (c, cell) in row.iter().enumerate().filter(|(_, cell)| cell.1 != core::VGA[0]) {
                pic.fill(x(c as f32), y(r as f32), x(c as f32 + 1.), y(r as f32 + 1.), cell.1, 1.);
            }
        }
    }
    let on = |key: &str| cfg[key] == true && !ansi && !plain;
    if on("stars") {
        stars(&mut pic, cfg["stars_mode"] == "warp", theme.text, font.size / 16., s);
    }
    // the wobble at its first moment: each row of pixels of the art slides sideways on a sine wave, the text stays
    let sway = if on("wobble") { ch * match cfg["wobble_amount"].as_str() { Some("low") => 0.15, Some("high") => 0.45, _ => 0.3 } } else { 0. };
    let shift = |py: i64| -> i64 {
        let rows = (py as f32 + 0.5 - oy) / ch;
        match page.group.get(rows.max(0.) as usize) {
            Some(Some(_)) if sway > 0. && rows >= 0. => (sway * (rows / 0.75).sin()).round() as i64,
            _ => 0,
        }
    };
    let ramp_colors: Option<[Rgb; 3]> = on("ramp").then(|| [0, 1, 2].map(|i| core::hex(cfg["ramp_colors"][i].as_str().unwrap_or("#000000"))));
    let ramp = ramp_colors.as_ref();
    // the glow: the small blurred images of the window, scaled up smoothly
    let still = view::Fx { t: 0., level: None, neon: None, view: Rect::ZERO };
    let (mut soft, parts) = if plain { (false, Vec::new()) } else { view::glow_parts(page, cfg, cw / s, &still) };
    for (k, alpha) in &parts {
        let k = k * s;
        let Some((image, m)) = view::bake(page, cw, ch, k, ramp, theme.art) else {
            soft = false; // a page too big for it gets sharp blocks instead
            continue;
        };
        lay(&mut pic, &image, k, ox - m, oy - m, *alpha, &shift);
    }
    let cycle = on("cycle").then(|| Bands::new(cfg, ramp, ch, 0.).on_page(ch));
    for (r, line) in page.lines.iter().enumerate() {
        for (c, &chr) in line.iter().enumerate() {
            let Some((rects, alpha)) = core::block(chr).filter(|_| !soft) else { continue };
            let (xa, cell) = (x(c as f32), (x(c as f32 + 1.) - x(c as f32)) as f32);
            for q in rects {
                for py in y(r as f32 + q[1])..y(r as f32 + q[3]) {
                    let rows = (py as f32 + 0.5 - oy) / ch;
                    let color = match &cycle {
                        _ if ansi => view::ink(page, r, c, theme),
                        Some(bands) => bands.at(rows * ch),
                        None => view::block_color(page, r, rows, ramp, theme.art),
                    };
                    let slide = shift(py);
                    pic.fill(xa + (q[0] * cell).round() as i64 + slide, py, xa + (q[2] * cell).round() as i64 + slide, py + 1, color, alpha);
                }
            }
        }
    }
    if on("text_glow") {
        // the text blurred twice, tight and wide, as in the window (view::bake_text works in points)
        for (k, alpha) in [((ch / s * 0.2).max(2.) / 2., 0.7), ((ch / s * 0.5).max(4.) / 2., 0.5)] {
            if let Some((image, m)) = view::bake_text(ctx, page, theme, font, cw / s, ch / s, k) {
                lay(&mut pic, &image, k * s, ox - m * s, oy - m * s, alpha, &|_| 0);
            }
        }
    }
    // the text: laid out as in the window, then every glyph copied out of the font image
    let mut glyphs = Vec::new();
    for (r, line) in page.lines.iter().enumerate() {
        let galley = ctx.fonts_mut(|f| f.layout_job(view::row_text(page, r, theme, font)));
        for placed in &galley.rows {
            let known = placed.row.glyphs.len() == line.len(); // one glyph per character, else the colours are not known
            for (i, glyph) in placed.row.glyphs.iter().enumerate().filter(|(_, g)| !g.uv_rect.is_nothing()) {
                let at = placed.pos + glyph.pos.to_vec2() + glyph.uv_rect.offset;
                let color = if known { view::ink(page, r, i, theme) } else { theme.text };
                glyphs.push((x(0.) + (at.x * s).round() as i64, y(r as f32) + (at.y * s).round() as i64, glyph.uv_rect, color));
            }
        }
        for (a, b, _) in page.links.get(r).into_iter().flatten() {
            let under = y(r as f32) + (ch * 0.92) as i64;
            pic.fill(x(*a as f32), under, x(*b as f32), under + s.round().max(1.) as i64, theme.link, 1.);
        }
    }
    let atlas = ctx.fonts(|f| f.image());
    for (gx, gy, uv, color) in glyphs {
        for (j, ay) in (uv.min[1]..uv.max[1]).enumerate() {
            for (i, ax) in (uv.min[0]..uv.max[0]).enumerate() {
                let cover = atlas.pixels[ay as usize * atlas.size[0] + ax as usize].a() as f32 / 255.;
                pic.blend(gx + i as i64, gy + j as i64, color.map(f32::from), cover);
            }
        }
    }
    if cfg["crt"] == true && !plain {
        // the still tube: scanlines every third pixel row (or the colour mask of a shadow mask or an aperture grille),
        // and darker corners
        let mask = match cfg["crt_mask"].as_str() { Some("dots") => 1, Some("stripes") => 2, _ => 0 };
        let (centre, inner, outer) = ((w as f32 / 2., h as f32 / 2.), w.min(h) as f32 * 0.35, w.max(h) as f32 * 0.75);
        for py in 0..h {
            for px in 0..w {
                let far = ((px as f32 - centre.0).hypot(py as f32 - centre.1) - inner) / (outer - inner);
                let dark = 1. - far.clamp(0., 1.) * 0.5;
                let line = if mask == 0 && py % 3 == 0 { 0.72 } else { 1. };
                let tint = if mask == 0 { [1.; 3] } else { crate::post::mask(mask, px, py) };
                for (v, t) in pic.rgb[(py * w + px) * 3..][..3].iter_mut().zip(tint) {
                    *v = (*v as f32 * dark * line * t) as u8;
                }
            }
        }
    }
    Ok(pic)
}

/// The picture as a PDF of one page that is exactly as big (a pixel is 1/96 inch).
/// shortcut: the page is one image, its text cannot be selected as in the PDF of NfoGlow for Linux; upgrade with
/// a PDF library that draws text.
pub fn pdf(pic: &Picture) -> Vec<u8> {
    let mut zip = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    zip.write_all(&pic.rgb).unwrap();
    let image = zip.finish().unwrap();
    let (pw, ph) = (pic.w as f32 * 0.75, pic.h as f32 * 0.75);
    let content = format!("q {pw} 0 0 {ph} 0 0 cm /Im Do Q");
    let objects: [Vec<u8>; 5] = [
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw} {ph}] /Resources << /XObject << /Im 5 0 R >> >> /Contents 4 0 R >>").into_bytes(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).into_bytes(),
        [format!("<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n", pic.w, pic.h, image.len()).as_bytes(), &image, b"\nendstream"].concat(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend(object);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}

/// Save the picture: a PDF when the path ends in .pdf, else a PNG.
pub fn save(pic: &Picture, path: &std::path::Path) -> Result<(), String> {
    let text = |e: &dyn std::fmt::Display| e.to_string();
    let file = std::fs::File::create(path).map_err(|e| text(&e))?;
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
        return std::io::BufWriter::new(file).write_all(&pdf(pic)).map_err(|e| text(&e));
    }
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), pic.w as u32, pic.h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().and_then(|mut writer| writer.write_image_data(&pic.rgb)).map_err(|e| text(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::empty();
        fonts.font_data.insert("dos".into(), egui::FontData::from_static(include_bytes!("../assets/PxPlus_IBM_VGA_8x16.ttf")).into());
        fonts.families.insert(egui::FontFamily::Monospace, vec!["dos".into()]);
        fonts.families.insert(egui::FontFamily::Proportional, vec!["dos".into()]);
        ctx.set_fonts(fonts);
        let _ = ctx.run_ui(Default::default(), |_| {}); // the fonts are there after the first frame
        ctx
    }

    fn pixel(pic: &Picture, x: usize, y: usize) -> Rgb {
        pic.rgb[(y * pic.w + x) * 3..][..3].try_into().unwrap()
    }

    fn crt_free(ctx: &egui::Context, page: &Page, cfg: &Cfg, theme: &Theme, font: &FontId) -> Picture {
        let mut still = cfg.clone();
        still.insert("wobble".into(), false.into());
        render(ctx, page, &still, theme, font, 8., 16., false).unwrap()
    }

    #[test]
    fn the_page_as_a_picture() {
        let ctx = context();
        let font = FontId::monospace(16.);
        let page = Page::fill("██ x\n█▀\n\nhi www.x.org".as_bytes(), "auto", false);
        let mut cfg = core::defaults();
        for (key, value) in [("soft", false), ("ramp", false)] {
            cfg.insert(key.into(), value.into());
        }
        let theme = core::theme_colors(&cfg);
        let pic = render(&ctx, &page, &cfg, &theme, &font, 8., 16., false).unwrap();
        assert_eq!((pic.w, pic.h, pic.rgb.len()), (12 * 8 + 32, 4 * 16 + 48, pic.w * pic.h * 3)); // two cells left and right, one and a half rows above and below
        assert_eq!((pixel(&pic, 0, 0), pixel(&pic, 16, 24), pixel(&pic, 31, 39)), (theme.bg, theme.art, theme.art)); // the blocks: exact cells
        assert_eq!((pixel(&pic, 24, 47), pixel(&pic, 24, 48)), (theme.art, theme.bg)); // ▀: the upper half
        let ink = |x0: usize, y0: usize, color: Rgb| (0..8).any(|x| (0..16).any(|y| pixel(&pic, x0 + x, y0 + y) == color));
        assert!(ink(16 + 24, 24, theme.text) && !ink(16 + 16, 24, theme.text)); // the x of the first row, and the blank before it
        assert!(ink(16 + 24, 24 + 48, theme.link) && ink(16, 24 + 48, theme.text)); // the link in its colour, underlined
        // a colour ramp, the glow around the art, the tube
        cfg.insert("ramp".into(), true.into());
        cfg.insert("ramp_colors".into(), serde_json::json!(["#ff0000", "#00ff00", "#0000ff"]));
        let ramp = render(&ctx, &page, &cfg, &theme, &font, 8., 16., false).unwrap();
        assert!(pixel(&ramp, 16, 24)[0] > 240 && pixel(&ramp, 16, 55)[2] > 240); // red at the top, blue at the bottom
        cfg.insert("soft".into(), true.into());
        let glow = render(&ctx, &page, &cfg, &theme, &font, 8., 16., false).unwrap();
        assert_ne!(pixel(&glow, 14, 30), theme.bg); // light beside the art
        let text_only = render(&ctx, &page, &cfg, &theme, &font, 8., 16., true).unwrap().rgb;
        for key in ["soft", "ramp"] {
            cfg.insert(key.into(), false.into());
        }
        assert!(text_only == render(&ctx, &page, &cfg, &theme, &font, 8., 16., false).unwrap().rgb); // the text-only view: no effect
        cfg.insert("crt".into(), true.into());
        let crt = render(&ctx, &page, &cfg, &theme, &font, 8., 16., false).unwrap();
        assert!(pixel(&crt, 60, 3)[0] < pixel(&crt, 60, 4)[0] && pixel(&crt, 0, 1)[0] < pixel(&crt, 60, 1)[0]); // a scanline; a dark corner
        cfg.insert("crt_mask".into(), "stripes".into());
        let grille = render(&ctx, &page, &cfg, &theme, &font, 8., 16., false).unwrap();
        let [r, g, _] = pixel(&grille, 60, 30); // red, then green stripes, no scanline
        assert!(r > g && pixel(&grille, 61, 30)[1] > pixel(&grille, 61, 30)[0] && pixel(&grille, 60, 3)[0].abs_diff(pixel(&grille, 60, 4)[0]) < 4);
        cfg.insert("crt".into(), false.into());
        cfg.insert("wobble".into(), true.into());
        let wobble = render(&ctx, &page, &cfg, &theme, &font, 8., 16., false).unwrap();
        assert!((24..56).any(|y| pixel(&wobble, 16, y) != pixel(&crt_free(&ctx, &page, &cfg, &theme, &font), 16, y))); // the art slides
        cfg.insert("wobble".into(), false.into());
        // ANSI art: its own colours on black
        let ansi = Page::fill(b"\x1b[41;33m\xdbA", "auto", false);
        let pic = render(&ctx, &ansi, &core::defaults(), &theme, &font, 8., 16., false).unwrap();
        assert_eq!((pixel(&pic, 0, 0), pixel(&pic, 16, 24), pixel(&pic, 24, 24)), ([0, 0, 0], core::VGA[3], core::VGA[1]));
        let long = Page::new(vec!["x".repeat(200); 60000], None, String::new());
        assert!(render(&ctx, &long, &core::defaults(), &theme, &font, 8., 16., false).is_err()); // too big to make
    }

    #[test]
    fn png_and_pdf_files() {
        let pic = Picture { w: 3, h: 2, rgb: vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 9, 9, 9, 8, 8, 8, 7, 7, 7] };
        let dir = std::env::temp_dir().join(format!("nfoglow-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        save(&pic, &dir.join("a.png")).unwrap();
        let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(dir.join("a.png")).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut data = vec![0; 18];
        let info = reader.next_frame(&mut data).unwrap();
        assert_eq!((info.width, info.height, data), (3, 2, pic.rgb.clone()));
        save(&pic, &dir.join("a.PDF")).unwrap();
        let file = std::fs::read(dir.join("a.PDF")).unwrap();
        assert_eq!(file, pdf(&pic));
        let text = String::from_utf8_lossy(&file);
        assert!(text.starts_with("%PDF-1.4") && text.ends_with("%%EOF\n") && text.contains("/Width 3 /Height 2") && text.contains("/MediaBox [0 0 2.25 1.5]"));
        // the table at the end points at every object
        let xref: usize = text.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(file[xref..].starts_with(b"xref\n0 6\n"));
        for (i, line) in String::from_utf8_lossy(&file[xref..]).lines().skip(3).take(5).enumerate() {
            let offset: usize = line[..10].parse().unwrap();
            assert!(file[offset..].starts_with(format!("{} 0 obj\n", i + 1).as_bytes()), "{line}");
        }
        assert!(save(&pic, &dir.join("missing/a.png")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
