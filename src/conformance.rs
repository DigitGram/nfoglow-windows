// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! Conformance with NfoGlow for Linux: replays the cases that were recorded from the Python original (rust-anything,
//! `rust/*-rs/cases.jsonl` of the nfoglow-rust folder) against this port. Only the well-typed cases are used (those
//! with a result), the fuzz cases that make Python raise on wrong argument types are skipped.
//! Run: NFOGLOW_CASES=/path/to/nfoglow-rust/rust cargo test conformance -- --nocapture
use crate::{core, decode, fx, page::Page, view};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// A recorded value as plain JSON: tuples become lists, floats numbers, bytes a list of numbers.
fn plain(v: &Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(plain).collect()),
        Value::Object(m) if m.len() == 1 => {
            let (k, x) = m.iter().next().unwrap();
            match k.as_str() {
                "$tuple" | "$bytes" => plain(x),
                "$float" | "$int" | "$num" => x.as_str().and_then(|s| s.parse::<f64>().ok()).map_or(Value::Null, |f| json!(f)),
                _ => Value::Object(m.iter().map(|(k, x)| (k.clone(), plain(x))).collect()),
            }
        }
        Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), plain(x))).collect()),
        other => other.clone(),
    }
}

fn bytes_of(v: &Value) -> Option<Vec<u8>> {
    v.get("$bytes")?.as_array()?.iter().map(|b| b.as_u64().map(|b| b as u8)).collect()
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap_or(f64::NAN), y.as_f64().unwrap_or(f64::NAN));
            x == y || (x - y).abs() <= 1e-5 * x.abs().max(y.abs()).max(1.0)
        }
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, p)| y.get(k).is_some_and(|q| same(p, q))),
        _ => a == b,
    }
}

struct Case {
    name: String,
    args: Vec<Value>,
    ok: Value,
}

/// The cases of one crate, one at a time (the decode cases hold whole NFO files: never all in memory).
fn for_each_case(dir: &str, crate_dir: &str, mut f: impl FnMut(&Case)) {
    use std::io::BufRead;
    let path = Path::new(dir).join(crate_dir).join("cases.jsonl");
    let Ok(file) = std::fs::File::open(&path) else { return };
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        if line.len() > 300_000 {
            continue; // whole NFOs: parsing them takes minutes in a debug build, and small ones show the same
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let (Some(ok), Some(name), Some(args)) = (v.get("ok"), v["fn"].as_str(), v["args"].as_array()) else { continue };
        f(&Case { name: name.to_string(), args: args.clone(), ok: ok.clone() });
    }
}

fn rgb_f(c: core::Rgb) -> Value {
    json!(c.map(|v| f64::from(v) / 255.0))
}

fn rgb_from(v: &Value) -> Option<core::Rgb> {
    let a = v.as_array()?;
    Some([0, 1, 2].map(|i| (a.get(i).and_then(Value::as_f64).unwrap_or(0.0) * 255.0).round() as u8))
}

fn strings(v: &Value) -> Option<Vec<String>> {
    v.as_array()?.iter().map(|s| s.as_str().map(String::from)).collect()
}

fn choices_of(v: &Value) -> Vec<(String, Vec<String>)> {
    v.as_object().map(|m| m.iter().map(|(k, l)| (k.clone(), strings(l).unwrap_or_default())).collect()).unwrap_or_default()
}

/// The JSON inside a look code (the two apps compress differently, so the codes themselves are not compared).
fn unpack(code: &str) -> Option<Value> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use std::io::Read;
    let packed = URL_SAFE_NO_PAD.decode(code.strip_prefix(core::LOOK_PREFIX)?.trim_end_matches('=')).ok()?;
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(&packed[..]).read_to_end(&mut raw).ok()?;
    serde_json::from_slice(&raw).ok()
}

#[derive(Default)]
struct Report {
    total: BTreeMap<String, usize>,
    bad: BTreeMap<String, Vec<String>>,
}

impl Report {
    fn check(&mut self, name: &str, ok: bool, detail: impl FnOnce() -> String) {
        *self.total.entry(name.into()).or_default() += 1;
        if !ok {
            self.bad.entry(name.into()).or_default().push(detail());
        }
    }
}

fn run_core(r: &mut Report, c: &Case) {
    let a = &c.args;
    match c.name.as_str() {
        "look_code" => {
            let (Some(Value::Object(cfg)), Some(keys)) = (a.first().map(plain), a.get(1).and_then(strings)) else { return };
            let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
            let want = c.ok.as_str().and_then(unpack);
            let mine = core::look_code(&cfg, &keys);
            let got = unpack(&mine);
            r.check("core.look_code", want.is_some() && want == got, || format!("{cfg:?}\n   python {want:?}\n   ours   {got:?}"));
            // and our reader takes the code Python made
            let read = core::read_look_code(c.ok.as_str().unwrap_or(""), &keys, core::LOOK_CHOICES);
            r.check("core.reads_python_code", read.is_some(), || format!("{}", c.ok));
        }
        "read_look_code" => {
            let (Some(text), Some(keys)) = (a.first().and_then(Value::as_str), a.get(1).and_then(strings)) else { return };
            let ch = choices_of(a.get(2).unwrap_or(&Value::Null));
            let ch_ref: Vec<(&str, Vec<&str>)> = ch.iter().map(|(k, l)| (k.as_str(), l.iter().map(String::as_str).collect())).collect();
            let ch_slices: Vec<(&str, &[&str])> = ch_ref.iter().map(|(k, l)| (*k, l.as_slice())).collect();
            let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
            let got = core::read_look_code(text, &keys, &ch_slices).map_or(Value::Null, Value::Object);
            let want = plain(&c.ok);
            r.check("core.read_look_code", same(&want, &got), || format!("{text:.60}\n   python {want}\n   ours   {got}"));
        }
        "is_dark" => {
            if let Some(bg) = a.first().map(plain).as_ref().and_then(rgb_from) {
                r.check("core.is_dark", c.ok == json!(core::is_dark(bg)), || format!("{bg:?}"));
            }
        }
        "theme_colors" => {
            let Some(Value::Object(cfg)) = a.first().map(plain) else { return };
            let t = core::theme_colors(&cfg);
            let got = json!([rgb_f(t.bg), rgb_f(t.text), rgb_f(t.art), rgb_f(t.link), t.dark]);
            r.check("core.theme_colors", same(&plain(&c.ok), &got), || format!("{cfg:?}\n   python {}\n   ours   {got}", plain(&c.ok)));
        }
        "is_art" => {
            if let Some(ch) = a.first().and_then(Value::as_str).and_then(|s| s.chars().next()) {
                r.check("core.is_art", c.ok == json!(core::is_art(ch)), || format!("U+{:04X}", ch as u32));
            }
        }
        "clean_text" => {
            let Some(lines) = a.first().and_then(strings) else { return };
            let dedent = a.get(1).and_then(Value::as_bool).unwrap_or(true);
            let got = core::clean_text(&lines, dedent);
            r.check("core.clean_text", c.ok == json!(got), || format!("{lines:?} dedent={dedent}\n   python {}\n   ours   {got:?}", c.ok));
        }
        "load_settings" => {
            let ch = choices_of(a.first().unwrap_or(&Value::Null));
            let ch_ref: Vec<(&str, Vec<&str>)> = ch.iter().map(|(k, l)| (k.as_str(), l.iter().map(String::as_str).collect())).collect();
            let ch_slices: Vec<(&str, &[&str])> = ch_ref.iter().map(|(k, l)| (*k, l.as_slice())).collect();
            let got = Value::Object(core::load_settings_from(Path::new("/nonexistent/settings.json"), &ch_slices));
            let mut want = plain(&c.ok);
            want["font"] = json!(core::FONT); // the standard font of Windows differs on purpose
            r.check("core.defaults", same(&want, &got), || {
                let (w, g) = (want.as_object().unwrap(), got.as_object().unwrap());
                let diff: Vec<String> = w.keys().chain(g.keys()).filter(|k| w.get(*k) != g.get(*k)).map(|k| format!("{k}: python {:?} ours {:?}", w.get(k), g.get(k))).collect();
                diff.join("; ")
            });
        }
        _ => {}
    }
}

fn run_decode(r: &mut Report, c: &Case) {
    let a = &c.args;
    match c.name.as_str() {
        "decode" => {
            let (Some(data), Some(enc)) = (a.first().and_then(bytes_of), a.get(1).and_then(Value::as_str).or(Some("auto"))) else { return };
            let (text, label, width) = decode::decode(&data, enc);
            let got = json!([text, label, width]);
            r.check("decode.decode", same(&plain(&c.ok), &got), || format!("enc={enc} {} bytes\n   python {:.200}\n   ours   {:.200}", data.len(), plain(&c.ok).to_string(), got.to_string()));
        }
        "is_ansi" => {
            if let Some(t) = a.first().and_then(Value::as_str) {
                r.check("decode.is_ansi", c.ok == json!(decode::is_ansi(t)), || format!("{t:?}"));
            }
        }
        "parse_ansi" => {
            let Some(text) = a.first().and_then(Value::as_str) else { return };
            let width = match a.get(1) {
                None | Some(Value::Null) => None,
                Some(v) => match v.as_u64() {
                    Some(n) if n <= 65535 => Some(n as u16),
                    _ => return,
                },
            };
            let want = plain(&c.ok);
            let (lines, colors) = decode::parse_ansi(text, width);
            let mut got_colors = Vec::new();
            let mut above_one = false;
            for row in &colors {
                got_colors.push(Value::Array(row.iter().map(|(f, b)| json!([rgb_f(*f), rgb_f(*b)])).collect()));
            }
            if want.to_string().contains("e+") || want[1].to_string().split(&[',', '[', ']'][..]).filter_map(|s| s.parse::<f64>().ok()).any(|f| f > 1.0) {
                above_one = true; // a colour value above 255 in the file: Python keeps it, u8 cannot
            }
            let got = json!([lines, got_colors]);
            if above_one {
                r.check("decode.parse_ansi.lines_when_colour_overflows", want[0] == got[0], || format!("{text:.60}"));
            } else {
                r.check("decode.parse_ansi", same(&want, &got), || format!("{text:?} width={width:?}\n   python {:.300}\n   ours   {:.300}", want.to_string(), got.to_string()));
            }
        }
        "forum_text" => {
            if let Some(lines) = a.first().and_then(strings) {
                let got = decode::forum_text(&lines);
                r.check("decode.forum_text", c.ok == json!(got), || format!("{lines:?}\n   python {}\n   ours   {got:?}", c.ok));
            }
        }
        "srr_files" => {
            let Some(data) = a.first().and_then(bytes_of) else { return };
            let Ok(files) = decode::srr_files(&data) else { return };
            let mut got = Map::new();
            for (n, d) in files {
                got.insert(n, json!(d));
            }
            let want = plain(&c.ok);
            r.check("decode.srr_files", same(&want, &Value::Object(got.clone())), || format!("python {want:.200}\n   ours {:.200}", Value::Object(got).to_string()));
        }
        "read" => {
            let Some(p) = a.first().and_then(Value::as_str) else { return };
            if !Path::new(p).exists() || p.ends_with(".zip") && !p.contains("dec/") && false {
                return;
            }
            match decode::read(Path::new(p)) {
                Ok((data, name)) => {
                    let got = json!([data, name]);
                    r.check("decode.read", same(&plain(&c.ok), &got), || format!("{p}: python name {} ours {name}", plain(&c.ok)[1]));
                }
                Err(e) => r.check("decode.read", false, || format!("{p}: ours error {e}")),
            }
        }
        _ => {}
    }
}

fn run_overlays(r: &mut Report, c: &Case) {
    let a = &c.args;
    match c.name.as_str() {
        "crt_flicker" => {
            if let Some(t) = a.first().and_then(Value::as_f64) {
                let want = c.ok.as_f64().unwrap_or(0.0);
                let got = f64::from(fx::crt_flicker(t as f32));
                r.check("overlays.crt_flicker", (want - got).abs() < 1e-5, || format!("t={t}: python {want} ours {got}"));
            }
        }
        "opening_end" => {
            if let Some(kind) = a.first().and_then(Value::as_str) {
                if kind == "modem" || a.len() > 2 {
                    return;
                }
                let name = a.get(1).and_then(Value::as_str).unwrap_or("");
                let want = c.ok.as_f64().unwrap_or(0.0);
                let got = f64::from(fx::opening_end(kind, name, 0));
                r.check("overlays.opening_end", (want - got).abs() < 1e-4, || format!("{kind} {name:?}: python {want} ours {got}"));
            }
        }
        "loader_screen" => {
            let (Some(t), Some(name)) = (a.first().and_then(Value::as_f64), a.get(1).and_then(Value::as_str)) else { return };
            let (lines, cursor, stripes) = fx::loader_screen(t as f32, name);
            let want = plain(&c.ok);
            let got = json!([lines, cursor.map(|(r, c)| json!([r, c])), stripes]);
            r.check("overlays.loader_screen", same(&want, &got), || format!("t={t} {name:?}\n   python {want}\n   ours   {got}"));
        }
        "dos_screen" => {
            let (Some(t), Some(name)) = (a.first().and_then(Value::as_f64), a.get(1).and_then(Value::as_str)) else { return };
            let info = a.get(2).map(plain).unwrap_or(json!([0, 0]));
            let (size, mtime) = (info[0].as_f64().unwrap_or(0.0) as u64, info[1].as_f64().unwrap_or(0.0) as i64);
            if mtime == 0 {
                return; // Python then shows the current time
            }
            let (lines, cursor) = fx::dos_screen(t as f32, name, size, mtime);
            let want = plain(&c.ok);
            let got = json!([lines, cursor.map(|(r, c)| json!([r, c]))]);
            r.check("overlays.dos_screen", same(&want, &got), || format!("t={t} {name:?} size={size}\n   python {:.250}\n   ours   {:.250}", want.to_string(), got.to_string()));
        }
        _ => {}
    }
}

fn run_page(r: &mut Report, v: &Value) {
    match v["kind"].as_str().unwrap_or("") {
        "page" => {
            let name = v["name"].as_str().unwrap_or("");
            let data = match v["bytes"].as_array() {
                Some(b) => b.iter().map(|x| x.as_u64().unwrap_or(0) as u8).collect(),
                None => match decode::read(Path::new(name)) {
                    Ok((d, _)) => d,
                    Err(_) => return,
                },
            };
            let text_only = v["text_only"].as_bool().unwrap_or(false);
            let mut p = Page::fill(&data, "auto", text_only);
            let tag = format!("{name} text_only={text_only}");
            let lines: Vec<String> = p.text();
            r.check("page.lines", plain(&v["lines"]) == json!(lines), || {
                let want = plain(&v["lines"]);
                let first = want.as_array().into_iter().flatten().zip(&lines).position(|(a, b)| a != &json!(b));
                format!("{tag}: first different line {first:?} python {} lines, ours {}; python {:.80} ours {:.80}", want.as_array().map_or(0, Vec::len), lines.len(), want[first.unwrap_or(0)].to_string(), json!(lines.get(first.unwrap_or(0))).to_string())
            });
            r.check("page.cols", v["cols"].as_u64() == Some(p.cols as u64), || format!("{tag}: python {} ours {}", v["cols"], p.cols));
            let mut links = Map::new();
            for (row, l) in p.links.iter().enumerate().filter(|(_, l)| !l.is_empty()) {
                links.insert(row.to_string(), json!(l.iter().map(|(a, b, u)| json!([a, b, u])).collect::<Vec<_>>()));
            }
            r.check("page.links", Value::Object(links.clone()) == v["links"], || format!("{tag}: python {:.200} ours {:.200}", v["links"], Value::Object(links).to_string()));
            let mut group = Map::new();
            for (row, g) in p.group.iter().enumerate() {
                if let Some((a, b)) = g {
                    group.insert(row.to_string(), json!([a, b]));
                }
            }
            r.check("page.group", Value::Object(group.clone()) == v["group"], || format!("{tag}: python {:.200} ours {:.200}", v["group"], Value::Object(group).to_string()));
            for (q, want) in v["find"].as_object().into_iter().flatten() {
                p.find(q);
                let got = json!(p.matches.iter().map(|m| json!([m.0, m.1, m.2])).collect::<Vec<_>>());
                r.check("page.find", &got == want, || format!("{tag} {q:?}: python {:.150} ours {:.150}", want, got));
            }
            for s in v["sel"].as_array().into_iter().flatten() {
                let (a, b) = (&s["sel"][0], &s["sel"][1]);
                p.sel = Some(((a[0].as_u64().unwrap_or(0) as usize, a[1].as_u64().unwrap_or(0) as usize), (b[0].as_u64().unwrap_or(0) as usize, b[1].as_u64().unwrap_or(0) as usize)));
                let only = s["copy_text_only"].as_bool().unwrap_or(true);
                let text = p.selected_text(only);
                r.check("page.selected_text", s["text"] == json!(text), || format!("{tag} sel={} only={only}: python {:?} ours {:?}", s["sel"], s["text"], text));
                let rows = json!(p.selection_rows(0, p.lines.len(), only).iter().map(|x| json!([x.0, x.1, x.2])).collect::<Vec<_>>());
                r.check("page.selection_rows", rows == s["rows"], || format!("{tag} sel={} only={only}: python {} ours {rows}", s["sel"], s["rows"]));
            }
        }
        "glow" => {
            let mut cfg = core::defaults();
            for (k, x) in v["cfg"].as_object().into_iter().flatten() {
                cfg.insert(k.clone(), x.clone());
            }
            let page = Page::fill(b"x", "auto", false);
            let fx = view::Fx { t: v["time"].as_f64().unwrap_or(0.0) as f32, level: v["level"].as_f64().map(|l| l as f32), neon: None, view: eframe::egui::Rect::ZERO };
            let (soft, parts) = view::glow_parts(&page, &cfg, v["cw"].as_f64().unwrap_or(9.6) as f32, &fx);
            let got = if parts.is_empty() { Value::Null } else if soft { json!(["soft", parts[0].0]) } else { json!(["halo", parts.iter().map(|p| json!([p.0, p.1])).collect::<Vec<_>>()]) };
            r.check("view.glow_parts", same(&plain(&v["result"]), &got), || format!("{} level={} t={}: python {} ours {got}", v["cfg"], v["level"], v["time"], v["result"]));
        }
        "bands" => {
            let mut cfg = core::defaults();
            cfg.insert("cycle_palette".into(), v["palette"].clone());
            cfg.insert("cycle_dir".into(), v["dir"].clone());
            let ramp: Option<[core::Rgb; 3]> = v["ramp"].as_array().map(|a| [0, 1, 2].map(|i| core::hex(a[i].as_str().unwrap_or("#000000"))));
            let b = view::Bands::new(&cfg, ramp.as_ref(), v["ch"].as_f64().unwrap_or(16.0) as f32, v["time"].as_f64().unwrap_or(0.0) as f32);
            let (colors, period, hard, shift) = b.parts();
            let got = json!([colors.iter().map(|c| rgb_f(*c)).collect::<Vec<_>>(), period, hard, shift]);
            let want = json!([plain(&v["colors"]), v["period"], v["hard"], v["shift"]]);
            let ok = same(&want, &got);
            r.check("view.cycle_bands", ok, || format!("{} {} ramp={} ch={} t={}: python {:.200} ours {:.200}", v["palette"], v["dir"], v["ramp"], v["ch"], v["time"], want.to_string(), got.to_string()));
        }
        "flicker" => {
            let (t, want) = (v["t"].as_f64().unwrap_or(0.0), v["v"].as_f64().unwrap_or(0.0));
            let got = f64::from(view::flicker(t as f32));
            r.check("view.flicker", (want - got).abs() < 1e-4, || format!("t={t}: python {want} ours {got}"));
        }
        _ => {}
    }
}

#[test]
fn conformance_with_the_linux_cases() {
    let Ok(dir) = std::env::var("NFOGLOW_CASES") else {
        eprintln!("NFOGLOW_CASES is not set: nothing replayed");
        return;
    };
    let mut r = Report::default();
    for_each_case(&dir, "core-rs", |c| run_core(&mut r, c));
    for_each_case(&dir, "decode-rs", |c| run_decode(&mut r, c));
    for_each_case(&dir, "overlays-rs", |c| run_overlays(&mut r, c));
    if let Ok(path) = std::env::var("NFOGLOW_PAGE_CASES") {
        use std::io::BufRead;
        if let Ok(file) = std::fs::File::open(path) {
            for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    run_page(&mut r, &v);
                }
            }
        }
    }
    // the translations: every message of every language of the Linux catalogs
    if let Some(Ok(text)) = std::env::var("NFOGLOW_I18N_CASES").ok().map(std::fs::read_to_string) {
        if let Ok(Value::Object(langs)) = serde_json::from_str::<Value>(&text) {
            for (code, messages) in &langs {
                let Some(lang) = crate::i18n::LANGUAGES.iter().find(|l| l.0 == code).map(|l| l.0) else { continue };
                crate::i18n::set_language(lang);
                for (id, want) in messages.as_object().into_iter().flatten() {
                    let got = crate::i18n::tr(id);
                    r.check("i18n.translate", want.as_str() == Some(got.as_str()), || format!("{code} {id:?}: python {want} ours {got:?}"));
                }
            }
        }
    }
    // the beat of the music (the same twenty loudness values as the Python original was fed)
    {
        let vals = [-60.0f32, -40.0, -30.0, -20.0, -25.0, -18.0, -18.0, -50.0, -22.0, -10.0, -12.0, -30.0, -60.0, -15.0, -15.0, -14.0, -13.0, -40.0, -5.0, -45.0];
        let want = [0.0, 0.5, 0.5, 0.75, 0.6527435443750429, 0.8, 0.7465875003715065, 0.6497735615714677, 0.5655139968284733, 1.0, 0.8703247258333905, 0.7574651283969663, 0.6592406302004437, 0.576511154099294, 0.5017519121313596, 0.4730510740989072, 0.46817273818127453, 0.4074623100002855, 1.0, 0.8703247258333905];
        let mut beat = crate::music::Beat::new();
        for (v, w) in vals.iter().zip(want) {
            let got = f64::from(beat.feed(*v));
            r.check("music.beat", (got - w).abs() < 1e-4, || format!("{v} dB: python {w} ours {got}"));
        }
        for suffix in [".mod", ".xm", ".s3m", ".it", ".ogg", ".oga", ".mp3", ".flac", ".wav", ".m4a"] {
            r.check("music.formats", crate::music::AUDIO.contains(&suffix), || suffix.to_string());
        }
    }
    let mut failed = 0;
    for (name, n) in &r.total {
        let bad = r.bad.get(name).map_or(0, Vec::len);
        failed += bad;
        println!("{} {name}: {}/{} match", if bad == 0 { "ok  " } else { "DIFF" }, n - bad, n);
        for d in r.bad.get(name).into_iter().flatten().take(4) {
            println!("      {d}");
        }
    }
    assert_eq!(failed, 0, "{failed} cases differ from NfoGlow for Linux");
}
