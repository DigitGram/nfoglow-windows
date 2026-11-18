// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! What every part of NfoGlow shares: the settings file, the look code, the colour themes and palettes, the table
//! of block characters, and turning lines into plain text. No window here. Ported from core.py of NfoGlow for Linux;
//! settings stay a JSON map, so the file and the look codes are the same in both apps.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use serde_json::{json, Map, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub type Cfg = Map<String, Value>;
pub type Rgb = [u8; 3];
/// {setting: allowed values}
pub type Choices<'a> = &'a [(&'a str, &'a [&'a str])];

pub const FONT: &str = "Consolas";
pub const DOS_FONT: &str = "PxPlus IBM VGA 8x16"; // the IBM VGA text font, bundled (CC BY-SA 4.0, by VileR)
pub const LOOK_PREFIX: &str = "nfoglow:1:"; // the 1 is the format; codes of NfoGlow for Linux are read as they are

pub fn defaults() -> Cfg {
    let Value::Object(d) = json!({
        "theme": "light", "font": FONT, "zoom": 100, "soft": true, "glow": 3,
        "encoding": "auto", "language": "auto", "copy_text_only": true,
        "custom": {"bg": "#ffffff", "text": "#1a1a1a", "art": "#1046a0", "link": "#0b57d0"}, "recent": [],
        "remember_recent": true, "glow_mode": "halo", "ramp": true,
        "ramp_colors": ["#73f0ff", "#4d8cff", "#d94dff"], "crt": false,
        "cycle": false, "cycle_palette": "c64", "pulse": false, "crt_live": false,
        "music": false, "music_dir": "", "music_shuffle": true, "volume": 60, "music_fade": true,
        "random_on_start": false, "cycle_dir": "down", "sidebar": false,
        "stars": false, "scroller": false, "scroller_text": "",
        "wobble": false, "wobble_amount": "mid", "stars_mode": "drift", "music_info": true,
        "maximized": false, "pulse_mode": "beat", "opening": "off", "text_glow": false,
        "neon_flicker": false, "sheen": false, "sparkle": false, "sparkle_amount": "some", "chroma": false,
        "crt_mask": "lines"
    }) else { unreachable!() };
    d
}

/// What a look code carries (LOOK_SHARED of the Linux app): the look, never the behaviour, and no font name.
pub const LOOK: &[&str] = &[
    "theme", "soft", "glow_mode", "glow", "ramp", "ramp_colors", "cycle", "cycle_palette", "pulse", "crt", "crt_live",
    "stars", "stars_mode", "scroller", "wobble", "wobble_amount", "text_glow", "neon_flicker", "cycle_dir",
    "pulse_mode", "opening", "sheen", "sparkle", "sparkle_amount", "chroma", "crt_mask",
];
/// The settings of a look that are picked from a list.
pub const LOOK_CHOICES: Choices = &[
    ("theme", &["light", "dark", "bw", "amber", "terminal", "paper", "c64", "custom"]),
    ("glow_mode", &["soft", "halo"]),
    ("cycle_palette", &["c64", "ramp"]),
    ("wobble_amount", &["low", "mid", "high"]),
    ("stars_mode", &["drift", "warp"]),
    ("cycle_dir", &["down", "up"]),
    ("pulse_mode", &["beat", "breathe"]),
    ("opening", &["off", "modem", "glitch", "loader", "crt", "dos", "dither", "neon", "rain"]),
    ("sparkle_amount", &["few", "some", "many"]),
    ("crt_mask", &["lines", "dots", "stripes"]),
];
pub const ENCODINGS: &[&str] = &["auto", "cp437", "cp850", "latin-1", "utf-8"];

pub fn config_path() -> PathBuf {
    let env = |k| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        env("APPDATA").unwrap_or_default().join("NfoGlow")
    } else {
        // only for developing on Linux: its own folder, so it never touches the settings of NfoGlow for Linux
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config"))).unwrap_or_default().join("nfoglow-rs")
    }
    .join("settings.json")
}

/// bool, whole number, other number, text, list and map are different kinds (as type() tells them apart in Python)
fn kind(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(n) if n.is_f64() => 3,
        Value::Number(_) => 2,
        Value::String(_) => 4,
        Value::Array(_) => 5,
        Value::Object(_) => 6,
    }
}

fn is_colour(v: &Value) -> bool {
    v.as_str().is_some_and(|s| s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit()))
}

fn allowed(choices: Choices, k: &str, v: &Value) -> bool {
    choices.iter().find(|(key, _)| *key == k).is_none_or(|(_, list)| v.as_str().is_some_and(|s| list.contains(&s)))
}

pub fn load_settings(choices: Choices) -> Cfg {
    load_settings_from(&config_path(), choices)
}

/// A value of the wrong kind or not among the choices goes back to its default: a hand-edited file or another
/// version's option must not stop the start.
pub fn load_settings_from(path: &Path, choices: Choices) -> Cfg {
    let d = defaults();
    let mut cfg = d.clone();
    let saved = std::fs::read(path).ok().and_then(|data| serde_json::from_slice(&data).ok());
    let Some(Value::Object(saved)) = saved else { return cfg };
    for (k, v) in saved {
        if d.get(&k).is_some_and(|dv| kind(dv) == kind(&v)) && allowed(choices, &k, &v) {
            cfg.insert(k, v);
        }
    }
    cfg["recent"].as_array_mut().unwrap().retain(Value::is_string);
    cfg["volume"] = json!(cfg["volume"].as_i64().unwrap_or(100).clamp(0, 100)); // above 100 the player would amplify
    let ramp = cfg["ramp_colors"].as_array().unwrap();
    if !(ramp.len() == 3 && ramp.iter().chain(cfg["custom"].as_object().unwrap().values()).all(is_colour)) {
        cfg.insert("custom".into(), d["custom"].clone());
        cfg.insert("ramp_colors".into(), d["ramp_colors"].clone());
    }
    cfg
}

pub fn save_settings(cfg: &Cfg) {
    let path = config_path();
    let tmp = path.with_extension("json.tmp");
    // settings are a convenience: on an error the viewer keeps working. The rename never leaves a half-written file.
    let _ = std::fs::create_dir_all(path.parent().unwrap())
        .and_then(|_| std::fs::write(&tmp, serde_json::to_vec_pretty(cfg).unwrap()))
        .and_then(|_| std::fs::rename(&tmp, &path));
}

/// The look (the settings in keys) as one line of text to share: compressed JSON in base64, after the prefix.
/// A Custom theme brings its colours along, and the DOS font as a switch (any other font of yours stays out).
pub fn look_code(cfg: &Cfg, keys: &[&str]) -> String {
    let mut look: Cfg = keys.iter().map(|k| (k.to_string(), cfg[*k].clone())).collect();
    look.insert("dos_font".into(), json!(cfg["font"] == DOS_FONT));
    if cfg["theme"] == "custom" {
        look.insert("custom".into(), cfg["custom"].clone());
    }
    let mut zip = ZlibEncoder::new(Vec::new(), Compression::best());
    zip.write_all(&serde_json::to_vec(&look).unwrap()).unwrap();
    format!("{LOOK_PREFIX}{}", URL_SAFE_NO_PAD.encode(zip.finish().unwrap()))
}

/// The look in a shared code as {setting: value}, or None when the text is no look code. A code comes from outside,
/// so it must never break the window: only the settings in keys are taken, each checked like load_settings does.
/// A value that does not fit (a later version's new option, say) is left out, so the setting stays as it is.
pub fn read_look_code(text: &str, keys: &[&str], choices: Choices) -> Option<Cfg> {
    let text = text.trim();
    let body = text.strip_prefix(LOOK_PREFIX)?;
    if text.chars().count() > 4000 {
        return None;
    }
    let packed = URL_SAFE_NO_PAD.decode(body.trim_end_matches('=')).ok()?;
    let mut raw = Vec::new();
    // as Python's zlib.decompressobj: a code cut short (no checksum at its end) still gives what it holds
    match ZlibDecoder::new(&packed[..]).take(20001).read_to_end(&mut raw) {
        Err(e) if e.kind() != std::io::ErrorKind::UnexpectedEof => return None,
        _ => {}
    }
    if raw.len() > 20000 {
        return None; // a zip bomb stays out
    }
    let Value::Object(data) = serde_json::from_slice(&raw).ok()? else { return None };
    let d = defaults();
    let mut look = Cfg::new();
    for &k in keys {
        let Some(v) = data.get(k) else { continue };
        let fits = match k {
            "glow" => (1..=5).contains(&v.as_i64().unwrap_or(0)),
            "ramp_colors" => v.as_array().is_some_and(|a| a.len() == 3 && a.iter().all(is_colour)),
            _ => true,
        };
        if kind(v) == kind(&d[k]) && allowed(choices, k, v) && fits {
            look.insert(k.into(), v.clone());
        }
    }
    if let Some(v @ Value::Bool(_)) = data.get("dos_font") {
        look.insert("dos_font".into(), v.clone());
    }
    if let (Some("custom"), Some(Value::Object(custom))) = (look.get("theme").and_then(Value::as_str), data.get("custom")) {
        let names = d["custom"].as_object().unwrap();
        if custom.len() == names.len() && custom.iter().all(|(k, v)| names.contains_key(k) && is_colour(v)) {
            look.insert("custom".into(), data["custom"].clone());
        }
    }
    (!look.is_empty()).then_some(look)
}

pub fn hex(h: &str) -> Rgb {
    [1, 3, 5].map(|i| u8::from_str_radix(h.get(i..i + 2).unwrap_or("0"), 16).unwrap_or(0))
}

pub struct Theme {
    pub bg: Rgb,
    pub text: Rgb,
    pub art: Rgb,
    pub link: Rgb,
    pub dark: bool, // dark window chrome
}

// name, background, text, art, link, dark
const THEMES: &[(&str, &str, &str, &str, &str, bool)] = &[
    ("light", "#ffffff", "#1a1a1a", "#1046a0", "#0b57d0", false),
    ("bw", "#ffffff", "#000000", "#000000", "#0000cc", false), // black on white, as NFOs long looked in plain viewers
    ("dark", "#14151a", "#d8dae0", "#6ea8ff", "#8ab4ff", true),
    ("amber", "#0c0802", "#ffb000", "#ff8c00", "#ffd27a", true),
    ("terminal", "#050805", "#3dff70", "#1fbf4f", "#9dffb8", true),
    ("paper", "#f3ead6", "#3b3226", "#8b3a1e", "#a0522d", false),
    ("c64", "#352879", "#a59ff0", "#7c70da", "#d2cdff", true), // the blue screen, its light blue a bit lighter to read
];

/// Whether a background colour is dark (by its perceived brightness).
pub fn is_dark(bg: Rgb) -> bool {
    0.2126 * bg[0] as f32 + 0.7152 * bg[1] as f32 + 0.0722 * (bg[2] as f32) < 127.5
}

/// The colours of the configured theme; "custom" comes from cfg["custom"].
pub fn theme_colors(cfg: &Cfg) -> Theme {
    let name = cfg.get("theme").and_then(Value::as_str).unwrap_or("light");
    if name == "custom" {
        let d = defaults();
        let c = |k: &str| hex(cfg.get("custom").and_then(|c| c.get(k)).and_then(Value::as_str).unwrap_or(d["custom"][k].as_str().unwrap()));
        return Theme { bg: c("bg"), text: c("text"), art: c("art"), link: c("link"), dark: is_dark(c("bg")) };
    }
    let t = THEMES.iter().find(|t| t.0 == name).unwrap_or(&THEMES[0]);
    Theme { bg: hex(t.1), text: hex(t.2), art: hex(t.3), link: hex(t.4), dark: t.5 }
}

/// The classic VGA text-mode palette, in ANSI order (black, red, green, brown, blue, magenta, cyan, grey, then bright).
pub const VGA: [Rgb; 16] = [
    [0, 0, 0], [0xaa, 0, 0], [0, 0xaa, 0], [0xaa, 0x55, 0], [0, 0, 0xaa], [0xaa, 0, 0xaa], [0, 0xaa, 0xaa], [0xaa, 0xaa, 0xaa],
    [0x55, 0x55, 0x55], [0xff, 0x55, 0x55], [0x55, 0xff, 0x55], [0xff, 0xff, 0x55], [0x55, 0x55, 0xff], [0xff, 0x55, 0xff],
    [0x55, 0xff, 0xff], [0xff, 0xff, 0xff],
];

/// The C64 palette ("Pepto"): black, white, red, cyan, purple, green, blue, yellow, orange, light red, light green,
/// light blue, grey.
pub const C64: [Rgb; 13] = [
    [0, 0, 0], [0xff, 0xff, 0xff], [0x68, 0x37, 0x2b], [0x70, 0xa4, 0xb2], [0x6f, 0x3d, 0x86], [0x58, 0x8d, 0x43], [0x35, 0x28, 0x79],
    [0xb8, 0xc7, 0x6f], [0x6f, 0x4f, 0x25], [0x9a, 0x67, 0x59], [0x9a, 0xd2, 0x84], [0x6c, 0x5e, 0xb5], [0x95, 0x95, 0x95],
];
pub const C64_BLUE: Rgb = C64[6];
pub const C64_LIGHT_BLUE: Rgb = C64[11];
/// The light blue border around the C64 screen (the theme shows it around the view) and its width: this share of the
/// window width on every side, like the original (about 8 % of the screen width), so it keeps its proportions at any
/// zoom or window size.
pub const C64_BORDER: Rgb = [0x6c, 0x5e, 0xb5];
pub const C64_FRAME: f32 = 0.075;
/// The bands the C64 demos cycled through their logos: blue, light blue, cyan, white and back, then purple, light
/// red, yellow, white and back.
pub const C64_CYCLE: [Rgb; 14] = [C64[6], C64[11], C64[3], C64[1], C64[3], C64[11], C64[6], C64[4], C64[9], C64[7], C64[1], C64[7], C64[9], C64[4]];

pub const SQUARE: char = '\u{25A0}';

/// A block element (U+2580..259F and ■) as rectangles in cell fractions (x0, y0, x1, y1) and their alpha.
pub fn block(ch: char) -> Option<(Vec<[f32; 4]>, f32)> {
    const QUADS: [[f32; 4]; 4] = [[0., 0., 0.5, 0.5], [0.5, 0., 1., 0.5], [0., 0.5, 0.5, 1.], [0.5, 0.5, 1., 1.]]; // UL UR LL LR
    const MASKS: [u8; 10] = [4, 8, 1, 13, 9, 7, 11, 2, 6, 14]; // U+2596..259F, bit = index in QUADS
    let cp = ch as u32;
    let one = |r| (vec![r], 1.);
    Some(match cp {
        0x2580 => one([0., 0., 1., 0.5]),
        0x2581..=0x2587 => one([0., 1. - (cp - 0x2580) as f32 / 8., 1., 1.]),
        0x2588 => one([0., 0., 1., 1.]),
        0x2589..=0x258F => one([0., 0., (0x2590 - cp) as f32 / 8., 1.]),
        0x2590 => one([0.5, 0., 1., 1.]),
        0x2591..=0x2593 => (vec![[0., 0., 1., 1.]], (cp - 0x2590) as f32 * 0.25),
        0x2594 => one([0., 0., 1., 0.125]),
        0x2595 => one([0.875, 0., 1., 1.]),
        0x2596..=0x259F => {
            let m = MASKS[(cp - 0x2596) as usize];
            ((0..4).filter(|i| m >> i & 1 == 1).map(|i| QUADS[i]).collect(), 1.)
        }
        0x25A0 => one([0.125, 0.28, 0.875, 0.72]), // ■, the square dot of CP437: in the art colours like the blocks
        _ => return None,
    })
}

/// Drawn in the art colour: the blocks, box drawing (═║╔), geometric shapes (▪ ▲ ► ○) and the two CP437 dots ∙ and ·.
pub fn is_art(ch: char) -> bool {
    matches!(ch as u32, 0x2500..=0x25FF | 0x2219 | 0xB7)
}

/// What DOS drew for the control codes 1..31 (NFOs use them as bullets and arrows).
pub const DOS_GLYPHS: &str = "☺☻♥♦♣♠•◘○◙♂♀♪♫☼►◄↕‼¶§▬↨↑↓→←∟↔▲▼";

/// A control code as DOS showed it; tab, line feed and carriage return stay what they are, NUL was a blank.
pub fn dos_glyph(ch: char) -> char {
    match ch as u32 {
        0 => ' ',
        9 | 10 | 13 => ch,
        n @ 1..=31 => DOS_GLYPHS.chars().nth(n as usize - 1).unwrap(),
        _ => ch,
    }
}

/// The lines of a plain NFO as they are shown: any line ending (also \r\r\n), DOS symbols for control codes, tabs
/// as spaces, and a double-spaced file without its extra blank lines.
pub fn plain_lines(text: &str) -> Vec<String> {
    let text = text.trim_end_matches('\x1a').replace("\r\n", "\n"); // \x1a ends a DOS file
    let mut lines: Vec<String> = Vec::new();
    // every \r left is a line end of its own, except the ones right before a \n ("\r\r\n" is one line end)
    for part in text.split('\n') {
        let part = part.trim_end_matches('\r');
        for line in part.split('\r') {
            let mut out = String::new();
            for ch in line.chars() {
                if ch == '\t' {
                    let n = 8 - out.chars().count() % 8;
                    out.extend(std::iter::repeat_n(' ', n));
                } else {
                    out.push(dos_glyph(ch));
                }
            }
            lines.push(out.trim_end().to_string());
        }
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    if lines.len() > 5 && lines.iter().skip(1).step_by(2).all(String::is_empty) {
        lines = lines.into_iter().step_by(2).collect(); // double-spaced: every second line empty
    }
    lines
}

/// Letter-spaced text ("F o r e v e r"): four or more single letters or digits with one space between them.
fn letter_spaced(c: &[char]) -> bool {
    let a = |i: usize| c.get(i).is_some_and(|ch| ch.is_alphanumeric());
    (0..c.len()).any(|i| {
        if !a(i) || (i > 0 && a(i - 1)) {
            return false;
        }
        let (mut n, mut j) = (1, i);
        while c.get(j + 1) == Some(&' ') && a(j + 2) {
            n += 1;
            j += 2;
        }
        n >= 4 && !a(j + 1)
    })
}

/// Lines without art characters: for the "text only" view and for copying. Blank runs collapse to one line; dedent
/// keeps the relative indentation, else every line is stripped. A line without a single word of two letters or
/// digits (ASCII used as art) goes too; a rule of one repeated sign ("-----") stays.
pub fn clean_text<S: AsRef<str>>(lines: &[S], dedent: bool) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        // art, no-break spaces (CP437 255, gaps in the art) and control codes become spaces
        let line: String =
            line.as_ref().chars().map(|ch| if is_art(ch) || ch == '\u{a0}' || (ch < ' ' && ch != '\t') { ' ' } else { ch }).collect();
        let mut line = line.trim_end();
        let core = line.trim();
        let rule = core.chars().count() > 3 && core.chars().all(|c| Some(c) == core.chars().next());
        let chars: Vec<char> = line.chars().collect();
        if !chars.windows(2).any(|w| w[0].is_alphanumeric() && w[1].is_alphanumeric()) && !rule && !letter_spaced(&chars) {
            line = "";
        }
        if !dedent {
            line = line.trim();
        }
        if !line.is_empty() || out.last().is_some_and(|l| !l.is_empty()) {
            out.push(line.to_string());
        }
    }
    while out.last().is_some_and(String::is_empty) {
        out.pop();
    }
    if dedent {
        // the leading whitespace all lines with text share
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut margin: Option<&str> = None;
        for l in out.iter().filter(|l| !l.is_empty()) {
            let lead = &l[..indent(l)];
            margin = Some(match margin {
                None => lead,
                Some(m) => &m[..m.bytes().zip(lead.bytes()).take_while(|(a, b)| a == b).count()],
            });
        }
        let n = margin.map_or(0, str::len);
        out = out.iter().map(|l| l.get(n..).unwrap_or("").to_string()).collect();
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_endings_and_double_spacing() {
        for eol in ["\n", "\r\n", "\r", "\r\r\n"] {
            assert_eq!(plain_lines(&["one", "", "three"].join(eol)), ["one", "", "three"], "{eol:?}");
        }
        assert_eq!(plain_lines(&["one", "two", "", "four", "five", "six"].join("\r\n\r\n")), ["one", "two", "", "four", "five", "six"]);
        assert_eq!(plain_lines(&["one", "two", "three", "four", "five", "six"].join("\n\n")), ["one", "two", "three", "four", "five", "six"]);
        assert_eq!(plain_lines("a\n\nb"), ["a", "", "b"]); // a short text keeps its blank line
        assert_eq!(plain_lines("a\n\nb\nc\n\nd\ne\nf"), ["a", "", "b", "c", "", "d", "e", "f"]);
        assert_eq!(plain_lines("a\tb"), ["a       b"]);
    }

    #[test]
    fn dos_symbols_in_plain_text() {
        assert_eq!(plain_lines("\x04 UNLEASHED \x04\n\x1b\x1a\x00\x10 ok\x1a"), ["♦ UNLEASHED ♦", "←→ ► ok"]);
    }

    #[test]
    fn text_only_skips_art_and_lone_signs() {
        assert_eq!(clean_text(&["██ ║ Title: x ║", "", "", "▓▓▓", "   more"], false), "Title: x\n\nmore");
        assert_eq!(clean_text(&["  a1", "    b2"], true), "a1\n  b2");
        let text = "■  Release Info  ■\n■";
        assert_eq!(clean_text(&text.split('\n').collect::<Vec<_>>(), false), "Release Info");
        assert!(is_art('\u{2219}') && is_art('\u{b7}') && !is_art('.'));
        let lines = ["█%                █%", "Shout out to all of those", "█%  █%", "------------------", "  +     +", "  o",
            "11 > Carmilla's Masque", "We miss you!"];
        assert_eq!(clean_text(&lines, false).split('\n').collect::<Vec<_>>(),
            ["Shout out to all of those", "", "------------------", "", "11 > Carmilla's Masque", "We miss you!"]);
        let lines = ["\u{a0}\u{a0}\u{a0}\u{2593}\u{2588}_\u{2584}\u{a0}\u{a0}\u{a0}\u{a0}", "___", "\x08", "Artist...: Somebody", "Rel_Date: 2026"];
        assert_eq!(clean_text(&lines, false), "Artist...: Somebody\nRel_Date: 2026");
        let logo = [r"\______  \  |  \_ ____/\_ ____/", r"|  |  _/  |      __)_ |  __)_", r"  \/        \/", "artist: x", "-----"];
        assert_eq!(clean_text(&logo, false), "artist: x\n-----");
        assert_eq!(clean_text(&["  F o r e v e r   I m m o r t a l", "  o  o", "  + +", "  a b c"], false), "F o r e v e r   I m m o r t a l");
    }

    #[test]
    fn blocks() {
        assert_eq!(block('█'), Some((vec![[0., 0., 1., 1.]], 1.)));
        assert_eq!(block('▄'), Some((vec![[0., 0.5, 1., 1.]], 1.)));
        assert_eq!(block('▌'), Some((vec![[0., 0., 0.5, 1.]], 1.)));
        assert_eq!(block('▒').unwrap().1, 0.5);
        assert_eq!(block('▚').unwrap().0, [[0., 0., 0.5, 0.5], [0.5, 0.5, 1., 1.]]); // UL LR
        assert_eq!(block('▟').unwrap().0.len(), 3);
        assert!(block('═').is_none() && is_art('═'));
        assert!((0x2580..=0x25A0).all(|cp| block(char::from_u32(cp).unwrap()).is_some()));
    }

    #[test]
    fn a_broken_settings_file_falls_back_to_default_colours() {
        let path = std::env::temp_dir().join(format!("nfoglow-test-{}.json", std::process::id()));
        let d = defaults();
        for saved in [json!({"ramp_colors": ["#fff"]}), json!({"ramp_colors": ["#73f0ff", "red", "#d94dff"]}),
            json!({"custom": {"bg": 5}}), json!({"custom": {"bg": "#12345"}})] {
            let mut file = saved.as_object().unwrap().clone();
            file.insert("zoom".into(), json!(90));
            std::fs::write(&path, serde_json::to_vec(&file).unwrap()).unwrap();
            let cfg = load_settings_from(&path, &[]);
            assert!(cfg["ramp_colors"] == d["ramp_colors"] && cfg["custom"] == d["custom"], "{saved}");
            assert_eq!(cfg["zoom"], 90); // the rest of the file still counts
        }
        std::fs::write(&path, r##"{"ramp_colors": ["#000000", "#111111", "#222222"]}"##).unwrap();
        assert_eq!(load_settings_from(&path, &[])["ramp_colors"][0], "#000000"); // a good file is left alone
        std::fs::write(&path, r#"{"recent": [1, "/a.nfo"], "encoding": "ebcdic", "glow_mode": "x", "wobble_amount": "x",
            "theme": "amber", "volume": 5000, "zoom": 1.5, "soft": 1}"#).unwrap();
        let choices = [LOOK_CHOICES, &[("encoding", ENCODINGS)]].concat();
        let cfg = load_settings_from(&path, &choices); // values no list knows: back to the defaults, the good ones stay
        assert_eq!(json!([cfg["recent"], cfg["encoding"], cfg["glow_mode"], cfg["wobble_amount"], cfg["theme"], cfg["volume"], cfg["zoom"], cfg["soft"]]),
            json!([["/a.nfo"], "auto", "halo", "mid", "amber", 100, 100, true]));
        std::fs::write(&path, "[1]").unwrap();
        assert_eq!(load_settings_from(&path, &[]), d);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(load_settings_from(&path, &[]), d);
    }

    fn make(json: &str) -> String {
        let mut zip = ZlibEncoder::new(Vec::new(), Compression::default());
        zip.write_all(json.as_bytes()).unwrap();
        format!("nfoglow:1:{}", base64::engine::general_purpose::URL_SAFE.encode(zip.finish().unwrap())) // padded, as Python makes it
    }

    #[test]
    fn look_code_round_trips_and_refuses_bad_codes() {
        let read = |code: &str| read_look_code(code, LOOK, LOOK_CHOICES);
        let mut cfg = defaults();
        for (k, v) in json!({"theme": "custom", "glow": 5, "opening": "rain", "ramp_colors": ["#010203", "#040506", "#070809"],
            "custom": {"bg": "#000000", "text": "#ffffff", "art": "#ff0000", "link": "#00ff00"}}).as_object().unwrap() {
            cfg.insert(k.clone(), v.clone());
        }
        let code = look_code(&cfg, LOOK);
        assert!(code.starts_with("nfoglow:1:") && !code.contains('\n') && code.len() < 600);
        let mut want: Cfg = LOOK.iter().map(|k| (k.to_string(), cfg[*k].clone())).collect();
        want.insert("custom".into(), cfg["custom"].clone());
        want.insert("dos_font".into(), json!(false));
        assert_eq!(read(&format!("  {code}\n")), Some(want));
        cfg.insert("font".into(), json!(DOS_FONT));
        assert_eq!(read(&look_code(&cfg, LOOK)).unwrap()["dos_font"], true);
        assert!(!read(&look_code(&defaults(), LOOK)).unwrap().contains_key("custom"));

        let bomb = make(&format!("[{}]", " ".repeat(30000)));
        for bad in ["", "hello", "nfoglow:1:", "nfoglow:1:!!!", &format!("nfoglow:2:{}", &code[10..]), &make("[1]"), &make("{}"),
            &make("{\"x\": 1}"), &format!("nfoglow:1:{}", "A".repeat(5000)), &bomb] {
            assert_eq!(read(bad), None, "{bad}");
        }
        // a wrong value is left out, the rest is taken; settings outside the look are never taken
        let got = read(&make(r#"{"theme": "nope", "glow": 9, "opening": 3, "crt": true, "ramp_colors": ["red", "red", "red"],
            "volume": 100, "music_dir": "/etc", "stars_mode": "warp"}"#));
        assert_eq!(Value::Object(got.unwrap()), json!({"crt": true, "stars_mode": "warp"}));
        assert_eq!(Value::Object(read(&make(r#"{"dos_font": "yes", "crt": true}"#)).unwrap()), json!({"crt": true}));
        // a custom theme needs all four colours, valid
        let bad_custom = make(r##"{"theme": "custom", "custom": {"bg": "#000000", "text": "#ffffff", "art": "x", "link": "#00ff00"}}"##);
        assert!(!read(&bad_custom).unwrap().contains_key("custom"));
        let d = defaults();
        assert!(LOOK_CHOICES.iter().all(|(k, list)| LOOK.contains(k) && list.contains(&d[*k].as_str().unwrap())));
    }

    /// A code made by NfoGlow for Linux 1.0 (core.look_code with its defaults, the theme set to "amber").
    #[test]
    fn reads_a_code_of_the_linux_app() {
        let code = include_str!("../tests/look_from_python.txt");
        let look = read_look_code(code, LOOK, LOOK_CHOICES).unwrap();
        assert_eq!(look["theme"], "amber");
        assert_eq!(look.len(), LOOK.len() + 1); // every setting of the look and dos_font
    }

    #[test]
    fn themes() {
        let mut cfg = defaults();
        assert_eq!(theme_colors(&cfg).art, [0x10, 0x46, 0xa0]);
        cfg.insert("theme".into(), json!("custom"));
        cfg.insert("custom".into(), json!({"bg": "#000000"}));
        let t = theme_colors(&cfg);
        assert!(t.dark && t.text == [0x1a, 0x1a, 0x1a]);
    }
}
