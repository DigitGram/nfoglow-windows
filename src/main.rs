// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! NfoGlow for Windows: the window. Command bar, search bar, the view, the status bar and the effects sidebar;
//! loading and browsing files, zoom, keys, the clipboard and the look with its undo.
#![recursion_limit = "256"] // the json! table of the default settings
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window behind the release build
mod about;
#[cfg(test)]
mod conformance;
mod core;
mod decode;
mod export;
mod fx;
mod i18n;
mod music;
mod page;
mod post;
mod settings;
mod sidebar;
mod smooth;
mod ui;
mod view;

use eframe::egui::{self, Color32, FontId, Key, Modifiers, Pos2, RichText, Vec2};
use i18n::tr;
use page::Page;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const BASE_PX: f32 = 16.; // font size at 100 % zoom: the DOS font then has its 8 x 16 pixels
const SIDEBAR: f32 = 320.; // as wide as the sidebar of NfoGlow for Linux at most: the style names fit a third of it
const BROWSE_SUFFIXES: [&str; 5] = [".nfo", ".diz", ".ans", ".asc", ".srr"]; // what the arrow keys step through in a folder
const THEMES: [(&str, &str); 8] = [("light", "Light"), ("dark", "Dark"), ("bw", "Black & White"), ("amber", "Amber"),
    ("terminal", "Terminal"), ("paper", "Paper"), ("c64", "C64"), ("custom", "Custom")];
const ENCODINGS: [(&str, &str); 5] = [("auto", "Automatic"), ("cp437", "CP437 (DOS)"), ("cp850", "CP850"), ("latin-1", "Latin-1"), ("utf-8", "UTF-8")];
const CUSTOM_COLORS: [(&str, &str); 4] = [("bg", "Background"), ("text", "Text"), ("art", "Block graphics"), ("link", "Links")];
// the colour ramp that comes with each theme (top, middle, bottom of a block of art); Custom keeps your own
const THEME_RAMPS: [(&str, [&str; 3]); 7] = [("light", ["#1046a0", "#5b2bb5", "#b5179e"]), ("bw", ["#000000", "#3d3d3d", "#808080"]),
    ("dark", ["#73f0ff", "#4d8cff", "#d94dff"]), ("amber", ["#ffe08a", "#ffb000", "#ff6a00"]), ("terminal", ["#c8ffd8", "#3dff70", "#12a64a"]),
    ("paper", ["#b5651d", "#8b3a1e", "#5c1f12"]), ("c64", ["#9ee8f0", "#7c70da", "#b66fcf"])];
// the ready-made ramps of the random look of NfoGlow for Linux: a look code may bring one, and it is not "set by hand"
const RAMPS: [[&str; 3]; 12] = [["#73f0ff", "#4d8cff", "#d94dff"], ["#fff36b", "#ff9a2e", "#e8322b"], ["#a6fff2", "#2ec4e8", "#2b4fe8"],
    ["#e8ff6b", "#5cff6e", "#1fae73"], ["#ffd36b", "#ff6b8b", "#8a4dff"], ["#ffffff", "#a6d8ff", "#4d7dff"], ["#fff2b0", "#e8b93a", "#a8661f"],
    ["#70a4b2", "#6c5eb5", "#6f3d86"], ["#1046a0", "#5b2bb5", "#b5179e"], ["#1f7a3a", "#127a7a", "#1046a0"], ["#b5281b", "#c2560f", "#8a1f5c"],
    ["#3a2bb5", "#1046a0", "#127a7a"]];

/// Styles: a few looks that set all the effects at once; the theme is left alone.
fn styles() -> [(&'static str, Value); 6] {
    let still = json!({"cycle": false, "pulse": false, "crt": false, "crt_live": false, "stars": false, "scroller": false, "wobble": false,
        "text_glow": false, "neon_flicker": false, "sheen": false, "sparkle": false, "chroma": false});
    let with = |more: Value| Value::Object(still.as_object().unwrap().clone().into_iter().chain(more.as_object().unwrap().clone()).collect());
    [("Classic", with(json!({"soft": false, "ramp": false}))),
     ("Soft", with(json!({"soft": true, "glow_mode": "soft", "ramp": false}))),
     ("Neon", with(json!({"soft": true, "glow_mode": "halo", "ramp": true}))),
     ("CRT", with(json!({"soft": true, "glow_mode": "soft", "crt_mask": "dots", "ramp": false, "crt": true, "crt_live": true, "text_glow": true, "chroma": true}))),
     ("Retrowave", with(json!({"soft": true, "glow_mode": "halo", "crt_mask": "lines", "ramp": true, "crt": true, "crt_live": true, "stars": true,
        "text_glow": true, "neon_flicker": true, "chroma": true}))),
     ("Demoscene", with(json!({"soft": true, "glow_mode": "halo", "ramp": false, "cycle": true, "pulse": true, "stars": true, "scroller": true,
        "wobble": true, "sheen": true, "sparkle": true})))]
}

const OPENINGS: [(&str, &str, &str); 9] = [
    ("off", "None", "Opens at once"),
    ("modem", "Modem build-up", "The NFO appears line by line, like over a 2400 baud BBS; a click or key skips it"),
    ("glitch", "Glitch", "A short flicker of torn lines, like a crackling loader"),
    ("loader", "Tape loader", "A C64 screen types LOAD and loads with colored stripes in its border; a click or key skips it"),
    ("crt", "CRT power-on", "A bright line grows into the picture, like an old CRT TV switching on; a click or key skips it"),
    ("dos", "DOS boot", "A DOS prompt boots, lists the file and types it out; a click or key skips it"),
    ("dither", "Dither fade-in", "The picture builds up in dithered steps, like ░ ▒ ▓ █; a click or key skips it"),
    ("neon", "Neon flicker-on", "The art flickers on like a neon sign; a click or key skips it"),
    ("rain", "Code rain", "Ones and zeros rain down the window and leave the NFO behind; a click or key skips it")];

struct App {
    cfg: core::Cfg,
    page: Page,
    page_id: u64, // counts the loaded pages: the key of the images made for one
    path: Option<PathBuf>,
    data: Vec<u8>,
    name: String,
    siblings: Vec<PathBuf>, // the NFOs in the folder of the open file
    text_only: bool,
    zoom: i64, // percent, of this window; cfg["zoom"] is the default
    cache: view::Cache,
    tiles: fx::Tiles,
    opening: Option<(String, f64)>, // the effect on opening that runs, and when it started
    rolls: u64,
    section: Option<usize>,  // the open section of the effects sidebar
    settings: Option<usize>, // the page of the settings window while it is open
    flash: Option<(&'static str, std::time::Instant)>, // the row of the settings the gear pointed at, and since when it lights up
    asking_reset: bool,
    logo: Option<egui::TextureHandle>,
    about: Option<about::About>, // the About window while it is open
    system_look: bool, // take light or dark from Windows at the next frame (first start)
    maximize: u32,     // frames left to maximize the window as the last one was left (0: done)
    player: Option<music::Player>, // made when music is first switched on
    level: Option<f32>,            // the beat of the music, for the pulse (None: no music)
    last_frame: f64,
    search: Option<String>,
    focus_search: bool,
    toast: Option<(String, f64)>, // text, and the time it goes away
    scroll_to: Option<Vec2>,
    fit: bool, // fit width: the zoom follows the width of the window
    zoom_before_fit: Option<i64>,
    offset: Vec2,   // of the view, as last drawn
    visible: Vec2,  // the size of the view
    content: Vec2,  // the size of the page
    cell: (f32, f32), // width and height of a character, as last drawn
    resize: bool,   // fit the window to the page at the next frame
    sidebar_was: Option<bool>, // whether the sidebar was open at the last frame: the window grows and shrinks with it
    width_before: Option<f32>, // the width of the window before the sidebar made it wider
    settle: u8,        // frames in which the window is still kept inside the screen (see fit_to_screen)
    settle_until: f64, // the latest time (egui time) for that, however slow the frames are; -1: not started yet
    drag_from: Option<page::Cell>,
    looks_before: Vec<core::Cfg>, // the Undo list
    saved_look: Option<core::Cfg>, // the look in the settings while another one is tried in the sidebar
    last_change: (String, f64),
    shortcuts: bool, // the keyboard shortcuts window is open
    stamp: Option<(Option<std::time::SystemTime>, u64)>, // when the open file was written and how big it was
    stamp_seen: Option<(Option<std::time::SystemTime>, u64)>, // a change seen once: read when it holds still
    next_look: f64, // when to look at the file again
    render: Option<eframe::egui_wgpu::RenderState>, // the graphics card, for the view's own render pass (post.rs)
    post: post::Post,
}

fn windows_font(name: &str) -> String {
    format!("{}\\Fonts\\{name}", std::env::var("WINDIR").unwrap_or("C:\\Windows".into()))
}

/// The monospace fonts of Windows that can show the NFO: (name, file in the Fonts folder). Consolas is the default.
pub const MONO_FONTS: [(&str, &str); 6] = [("Consolas", "consola.ttf"), ("Cascadia Mono", "CascadiaMono.ttf"),
    ("Cascadia Code", "CascadiaCode.ttf"), ("Lucida Console", "lucon.ttf"), ("Courier New", "cour.ttf"), ("JetBrains Mono", "JetBrainsMono-Regular.ttf")];

/// Where a font file can be: the Fonts folder of Windows, or the user's own (fonts installed "for me only").
fn font_paths(file: &str) -> Vec<PathBuf> {
    let own = std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Microsoft\\Windows\\Fonts").join(file));
    [Some(PathBuf::from(windows_font(file))), own].into_iter().flatten().collect()
}

/// The monospace fonts there are on this computer (on Linux, while developing, all of them: DejaVu stands in).
pub fn installed_fonts() -> Vec<&'static str> {
    let found: Vec<_> = MONO_FONTS.iter().filter(|f| font_paths(f.1).iter().any(|p| p.exists())).map(|f| f.0).collect();
    if found.is_empty() { MONO_FONTS.iter().map(|f| f.0).collect() } else { found }
}

/// The fonts: Segoe UI for the window, the chosen monospace font (Consolas by default) or the bundled DOS font for the
/// NFO. On Linux (developing) DejaVu stands in.
fn set_fonts(ctx: &egui::Context, font: &str) {
    let dos = font == core::DOS_FONT;
    let mut fonts = egui::FontDefinitions::empty();
    let mut add = |name: &str, data: egui::FontData| {
        fonts.font_data.insert(name.into(), data.into());
        name.to_string()
    };
    let file = |paths: [String; 2]| paths.iter().find_map(|p| std::fs::read(p).ok()).map(egui::FontData::from_owned);
    let dos_font = add("dos", egui::FontData::from_static(include_bytes!("../assets/PxPlus_IBM_VGA_8x16.ttf")));
    let ui_font = file([windows_font("segoeui.ttf"), "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".into()]).map(|d| add("ui", d));
    let chosen = MONO_FONTS.iter().find(|f| f.0 == font).filter(|_| !dos).and_then(|f| font_paths(f.1).iter().find_map(|p| std::fs::read(p).ok()));
    let mono = chosen.map(egui::FontData::from_owned)
        .or_else(|| file([windows_font("consola.ttf"), "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf".into()]))
        .filter(|_| !dos)
        .map(|d| add("mono", d));
    // Japanese and Chinese need a font of their own, a big one: only read for these languages
    let cjk = match i18n::current() {
        "ja" => file([windows_font("YuGothM.ttc"), "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into()]).or_else(|| file([windows_font("meiryo.ttc"), String::new()])),
        "zh_CN" => file([windows_font("msyh.ttc"), "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into()]),
        _ => None,
    }
    .map(|d| add("cjk", d));
    // the icons of Windows 11, else those of Windows 10 (the same code points); none when developing on Linux
    if let Some(icons) = file([windows_font("SegoeIcons.ttf"), windows_font("segmdl2.ttf")]) {
        fonts.families.insert(ui::icon_family(), vec![add("icons", icons)]);
    }
    // the DOS font last: it has every sign of CP437, for those the others lack
    fonts.families.insert(egui::FontFamily::Proportional, ui_font.into_iter().chain(cjk).chain([dos_font.clone()]).collect());
    fonts.families.insert(egui::FontFamily::Monospace, mono.into_iter().chain([dos_font]).collect());
    ctx.set_fonts(fonts);
}

/// The look of Windows 11: its greys, its blue, small round corners. The window (title bar, bars, sidebar, settings)
/// follows the light or dark mode of Windows, whatever the colour theme of the NFO is: the theme colours the page only.
/// Both looks are set, egui takes the one of Windows and switches when Windows does.
fn set_style(ctx: &egui::Context) {
    ctx.set_visuals_of(egui::Theme::Light, windows_visuals(false));
    ctx.set_visuals_of(egui::Theme::Dark, windows_visuals(true));
    ctx.set_theme(egui::ThemePreference::System);
    ctx.all_styles_mut(|s| {
        for (style, size) in [(egui::TextStyle::Body, 14.), (egui::TextStyle::Button, 14.), (egui::TextStyle::Small, 12.), (egui::TextStyle::Heading, 16.)] {
            s.text_styles.insert(style, FontId::proportional(size));
        }
        s.spacing.item_spacing = Vec2::new(8., 6.);
        s.spacing.button_padding = Vec2::new(10., 4.);
        s.spacing.interact_size.y = 28.;
    });
}

fn windows_visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let grey = |light: u8, dark_value: u8| Color32::from_gray(if dark { dark_value } else { light });
    v.panel_fill = grey(0xf3, 0x20);
    v.window_fill = grey(0xfb, 0x2b);
    v.extreme_bg_color = grey(0xff, 0x1a);
    v.selection.bg_fill = if dark { Color32::from_rgb(0x60, 0xcd, 0xff) } else { Color32::from_rgb(0x00, 0x67, 0xc0) };
    v.selection.stroke.color = if dark { Color32::BLACK } else { Color32::WHITE };
    v.hyperlink_color = v.selection.bg_fill;
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = egui::CornerRadius::same(4);
    }
    v.widgets.inactive.weak_bg_fill = grey(0xfb, 0x2d);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1., grey(0xd6, 0x3f));
    v.window_corner_radius = egui::CornerRadius::same(8);
    v.handle_shape = egui::style::HandleShape::Circle; // the knob of a slider is round in Windows 11
    v.menu_corner_radius = egui::CornerRadius::same(8);
    v
}

impl App {
    /// The app icon as a picture for the window (the empty page, the About window).
    fn logo_texture(&mut self, ctx: &egui::Context) -> egui::TextureHandle {
        self.logo
            .get_or_insert_with(|| {
                let icon = window_icon();
                ctx.load_texture("logo", egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba), egui::TextureOptions::LINEAR)
            })
            .clone()
    }

    /// Light or Dark as Windows is set, with the colour ramp of that theme: on the first start and after a reset, as
    /// NfoGlow for Linux does. egui knows the setting from the first frame on.
    fn use_system_theme(&mut self, ctx: &egui::Context) {
        let theme = if ctx.system_theme() == Some(egui::Theme::Dark) { "dark" } else { "light" };
        self.cfg.insert("theme".into(), json!(theme));
        self.cfg.insert("ramp_colors".into(), json!(THEME_RAMPS.iter().find(|t| t.0 == theme).unwrap().1));
        self.saved_look = None;
        self.save();
    }

    fn toast(&mut self, ctx: &egui::Context, text: impl Into<String>, seconds: f64) {
        self.toast = Some((text.into(), ctx.input(|i| i.time) + seconds));
    }

    fn dos_font(&self) -> bool {
        self.cfg["font"] == core::DOS_FONT
    }

    // ------------------------------------------------------------ settings and the look

    /// Change one setting, save it and show it. A change to the look goes on the Undo list first.
    fn change(&mut self, ctx: &egui::Context, key: &str, value: Value) {
        if self.cfg[key] == value {
            return;
        }
        let now = ctx.input(|i| i.time);
        let look = core::LOOK.contains(&key) || key == "font" || key == "custom";
        // a slider or a colour dragged around is one step, and so are the many settings of a style (one frame)
        if look && !(self.last_change.0 == key && now - self.last_change.1 < 1.) && self.last_change != ("".into(), now) {
            let before = self.look_now();
            self.looks_before.push(before);
            self.looks_before.drain(..self.looks_before.len().saturating_sub(100));
        }
        // a change made with the sidebar open is a trial until it is kept (as in NfoGlow for Linux); with the
        // sidebar shut (the settings by Ctrl+,) it is saved at once, or it would be lost without a sign
        if look && self.saved_look.is_none() && self.cfg["sidebar"] == true {
            self.saved_look = Some(self.look_now());
        }
        if look {
            self.last_change = (key.into(), now);
        }
        self.set(ctx, key, value);
    }

    /// The look as it is now: what Undo, Keep and Back work with.
    fn look_now(&self) -> core::Cfg {
        core::LOOK.iter().chain(&["font", "custom"]).map(|k| (k.to_string(), self.cfg[*k].clone())).collect()
    }

    /// Write the settings; a look that is still on trial is not written until it is kept.
    fn save(&self) {
        let mut cfg = self.cfg.clone();
        cfg.extend(self.saved_look.clone().unwrap_or_default());
        core::save_settings(&cfg);
    }

    /// Whether the look differs from the one in the settings (Keep and Back show then).
    fn trial(&self) -> bool {
        self.saved_look.as_ref().is_some_and(|saved| saved.iter().any(|(k, v)| self.cfg[k] != *v))
    }

    /// The look on trial becomes the settings.
    fn keep_look(&mut self) {
        self.saved_look = None;
        self.save();
    }

    /// Back to the look in the settings; the one left is a step on the Undo list.
    fn back_look(&mut self, ctx: &egui::Context) {
        let Some(saved) = self.saved_look.take() else { return };
        let now = self.look_now();
        self.looks_before.push(now);
        for (key, value) in saved {
            if self.cfg[&key] != value {
                self.set(ctx, &key, value);
            }
        }
    }

    fn set(&mut self, ctx: &egui::Context, key: &str, value: Value) {
        self.cfg.insert(key.into(), value);
        self.save();
        match key {
            "encoding" => self.reload(ctx),
            "font" => set_fonts(ctx, self.cfg["font"].as_str().unwrap_or(core::FONT)),
            "language" => {
                i18n::set_language(self.cfg["language"].as_str().unwrap());
                set_fonts(ctx, self.cfg["font"].as_str().unwrap_or(core::FONT));
            }
            "music" | "music_dir" | "music_shuffle" => self.apply_music(ctx),
            "volume" => {
                if let Some(player) = &mut self.player {
                    player.set_volume(self.cfg["volume"].as_i64().unwrap_or(60) as f32 / 100.);
                }
            }
            _ => {}
        }
    }

    /// Music on or off, from the folder (or the built-in chiptunes) in the chosen order.
    fn apply_music(&mut self, ctx: &egui::Context) {
        if self.cfg["music"] == true {
            if self.player.is_none() {
                match music::Player::new() {
                    Ok(player) => self.player = Some(player),
                    Err(error) => {
                        // no sound card: no music, but the viewer keeps working
                        self.toast(ctx, format!("♪ {error}"), 5.);
                        return self.set(ctx, "music", json!(false));
                    }
                }
            }
            let cfg = &self.cfg;
            let (folder, volume) = (cfg["music_dir"].as_str().unwrap_or(""), cfg["volume"].as_i64().unwrap_or(60) as f32 / 100.);
            self.player.as_mut().unwrap().play(folder, cfg["music_shuffle"] == true, volume, cfg["music_fade"] == true);
        } else if let Some(player) = &mut self.player {
            player.stop(self.cfg["music_fade"] == true);
        }
    }

    /// Keep the music going: the next track (announced by a short note), the fades, the beat for the pulse.
    /// shortcut: it runs with the window's frames; minimised, a track that ends may wait for the window to come back.
    fn music_frame(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let dt = (now - std::mem::replace(&mut self.last_frame, now)).clamp(0., 0.5) as f32;
        let Some(player) = self.player.as_mut().filter(|p| p.playing) else { return self.level = None };
        let started = player.tick(dt);
        self.level = player.level();
        ctx.request_repaint_after_secs(0.1);
        if let Some((number, count, name)) = started.filter(|_| self.cfg["music_info"] == true) {
            let stennish = || tr("Stennish – chiptune {number} of {count}").replace("{number}", &number.to_string()).replace("{count}", &count.to_string());
            self.toast(ctx, format!("♪ {}", name.unwrap_or_else(stennish)), 3.);
        }
    }

    /// Several settings as one step on the Undo list.
    fn change_all(&mut self, ctx: &egui::Context, look: &core::Cfg) {
        let mut first = true;
        for (key, value) in look {
            if self.cfg[key] != *value {
                if first {
                    self.last_change = (String::new(), -9.); // a step of its own
                    self.change(ctx, key, value.clone());
                    self.last_change = (String::new(), ctx.input(|i| i.time)); // the rest belongs to it
                    first = false;
                } else {
                    self.set(ctx, key, value.clone());
                }
            }
        }
        self.last_change = (String::new(), -9.);
    }

    /// The theme, and with it its colour ramp, unless the ramp was set by hand.
    fn set_theme(&mut self, ctx: &egui::Context, theme: &str) {
        let mut look = core::Cfg::new();
        look.insert("theme".into(), json!(theme));
        let made = THEME_RAMPS.iter().map(|t| t.1).chain(RAMPS).any(|r| json!(r) == self.cfg["ramp_colors"]);
        if let Some(own) = THEME_RAMPS.iter().find(|t| t.0 == theme).filter(|_| made) {
            look.insert("ramp_colors".into(), json!(own.1));
        }
        self.change_all(ctx, &look);
    }

    /// A look from a shared code, as one step on the Undo list. Like a rolled look it is on trial until kept (the Keep
    /// card in the sidebar, which opens for it), whether or not the sidebar was open.
    fn apply_look(&mut self, ctx: &egui::Context, mut look: core::Cfg) {
        if self.saved_look.is_none() {
            self.saved_look = Some(self.look_now());
        }
        if self.cfg["sidebar"] != true {
            self.set(ctx, "sidebar", json!(true));
        }
        if let Some(dos) = look.remove("dos_font").and_then(|v| v.as_bool()) {
            if dos != self.dos_font() {
                look.insert("font".into(), json!(if dos { core::DOS_FONT } else { core::FONT }));
            }
        }
        self.change_all(ctx, &look);
    }

    fn undo(&mut self, ctx: &egui::Context) {
        if let Some(before) = self.looks_before.pop() {
            for (key, value) in before {
                if self.cfg[&key] != value {
                    self.set(ctx, &key, value);
                }
            }
        }
    }

    // ------------------------------------------------------------ file

    fn load(&mut self, ctx: &egui::Context, path: &Path, resize: bool) {
        match decode::read(path) {
            Ok((data, name)) => (self.data, self.name) = (data, name),
            Err(error) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                return self.toast(ctx, tr("Cannot open {name}: {error}").replace("{name}", &name).replace("{error}", &error), 5.);
            }
        }
        self.path = Some(path.to_path_buf());
        (self.stamp, self.stamp_seen) = (stamp(path), None);
        // the NFO and SRR files in the folder, sorted by name: the arrow keys step through them
        let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
        self.siblings = std::fs::read_dir(&folder).into_iter().flatten().flatten().map(|e| e.path()).collect();
        let low = |p: &PathBuf| p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        self.siblings.retain(|p| BROWSE_SUFFIXES.iter().any(|s| low(p).ends_with(s)));
        self.siblings.sort_by_key(low);
        if self.cfg["remember_recent"] == true {
            let this = path.to_string_lossy().into_owned();
            let recent: Vec<Value> = [json!(this)].into_iter().chain(self.cfg["recent"].as_array().unwrap().iter().filter(|p| **p != this).cloned()).take(10).collect();
            self.set(ctx, "recent", json!(recent));
        }
        self.reload(ctx);
        self.scroll_to = Some(Vec2::ZERO);
        self.resize = resize;
        self.start_opening(ctx);
    }

    /// Decode the file again (another encoding, art or text only).
    fn reload(&mut self, ctx: &egui::Context) {
        if self.path.is_none() {
            return;
        }
        self.page = Page::fill(&self.data, self.cfg["encoding"].as_str().unwrap(), self.text_only);
        self.page_id += 1;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{} – NfoGlow", self.name)));
        if let Some(query) = self.search.clone() {
            self.page.find(&query);
        }
    }

    /// Read the file again and keep the scroll position.
    fn reread(&mut self, ctx: &egui::Context) {
        if let Some(path) = self.path.clone() {
            let top = self.offset;
            self.load(ctx, &path, false);
            self.scroll_to = Some(top);
        }
    }

    fn browse(&mut self, ctx: &egui::Context, step: isize) {
        let here = self.siblings.iter().position(|p| Some(p) == self.path.as_ref());
        // a jumping window while browsing would be annoying: no resize
        if let Some(next) = here.and_then(|i| self.siblings.get(i.checked_add_signed(step)?).cloned()) {
            self.load(ctx, &next, false);
        }
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        let dialog = rfd::FileDialog::new().add_filter(tr("NFO, ANSI, SRR and ZIP files"), &["nfo", "diz", "asc", "ans", "txt", "zip", "srr"]).add_filter(tr("All files"), &["*"]);
        if let Some(path) = dialog.pick_file() {
            self.load(ctx, &path, true);
        }
    }

    // ------------------------------------------------------------ view

    /// Start the effect chosen for opening a file (also the button next to the choice: once more, from the top).
    fn start_opening(&mut self, _ctx: &egui::Context) {
        let kind = self.cfg["opening"].as_str().unwrap_or("off");
        self.opening = (kind != "off" && !self.text_only && self.path.is_some()).then(|| (kind.to_string(), f64::NAN)); // it starts with the first picture drawn
        self.scroll_to = Some(Vec2::ZERO);
    }

    /// A new random look (theme, glow, colours, animation, tube, opening), as one step on the Undo list.
    fn roll_look(&mut self, ctx: &egui::Context) {
        if self.saved_look.is_none() {
            self.saved_look = Some(self.look_now()); // a rolled look is shown at once but not saved
        }
        self.rolls += 1;
        // the clock of the computer too: at the start the window's own clock is still at 0, and every start would roll the same
        let wall = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_micros() % 100_000);
        let seed = ctx.input(|i| i.time) * 1000. + self.rolls as f64 + wall as f64 / 7.;
        let mut n = 0.;
        let mut rnd = || {
            n += 1.;
            view::hash(&[seed, n]) as f64
        };
        fn pick<'a>(r: f64, from: &[&'a str]) -> &'a str {
            from[(r * from.len() as f64) as usize % from.len()]
        }
        let theme = pick(rnd(), &["dark", "dark", "dark", "amber", "terminal", "light", "paper", "c64"]);
        let style = pick(rnd(), &["off", "soft", "soft", "soft", "halo", "halo", "halo", "halo", "halo"]);
        let light = matches!(theme, "light" | "paper");
        let ramp = if light { RAMPS[8 + (rnd() * 4.) as usize % 4] } else { RAMPS[(rnd() * 8.) as usize % 8] }; // dark enough on a light page
        let crt = rnd() < 0.3;
        let pulse = style == "halo" && rnd() < 0.35;
        let look = json!({"theme": theme, "soft": style != "off", "glow": 2 + (rnd() * 4.) as i64 % 4, "ramp": rnd() < 0.6, "ramp_colors": ramp,
            "cycle": rnd() < 0.3, "cycle_palette": pick(rnd(), &["c64", "ramp"]), "wobble": rnd() < 0.25,
            "wobble_amount": pick(rnd(), &["low", "mid", "high"]), "stars_mode": pick(rnd(), &["drift", "warp"]), "stars": rnd() < 0.25,
            "scroller": rnd() < 0.2, "pulse": pulse, "crt": crt, "crt_live": crt && rnd() < 0.5, "text_glow": rnd() < 0.3,
            "cycle_dir": pick(rnd(), &["down", "up"]), "pulse_mode": pick(rnd(), &["beat", "breathe"]),
            "opening": if rnd() < 0.5 { "off" } else { OPENINGS[1 + (rnd() * 8.) as usize % 8].0 },
            "sheen": rnd() < 0.2, "sparkle": rnd() < 0.2, "sparkle_amount": pick(rnd(), &["few", "some", "many"]), "chroma": crt && rnd() < 0.5,
            "crt_mask": pick(rnd(), &["lines", "lines", "dots", "stripes"]),
            "smooth": rnd() < 1. / 6., "smooth_amount": pick(rnd(), &["low", "mid", "high"]), // rarely: some love their pixels
            "font": if rnd() < 0.25 { core::DOS_FONT } else { core::FONT }, // the VGA font now and then
            "neon_flicker": style == "halo" && !pulse && rnd() < 0.25, // not both: they beat the same glow
            "glow_mode": if style == "off" { self.cfg["glow_mode"].as_str().unwrap() } else { style }});
        self.change_all(ctx, look.as_object().unwrap());
    }

    fn set_text_only(&mut self, ctx: &egui::Context, on: bool) {
        self.text_only = on;
        self.opening = None; // text only: a calm page
        self.reload(ctx);
        self.scroll_to = Some(Vec2::ZERO);
    }

    fn zoom_step(&mut self, direction: i64) {
        self.fit = false; // zooming by hand ends "fit width"
        // steps of 10 %, from 30 to 250: beyond that nothing gets sharper, only bigger
        self.zoom = ((self.zoom + if direction > 0 { 10 } else { -1 }) / 10 * 10).clamp(30, 250);
    }

    fn copy(&mut self, ctx: &egui::Context) {
        let text_only = self.cfg["copy_text_only"] == true;
        let mut text = self.page.selected_text(text_only);
        if text.is_empty() {
            text = if text_only { core::clean_text(&self.page.text(), true) } else { self.page.text().join("\n") };
        }
        ctx.copy_text(text);
        self.toast(ctx, tr("Copied to the clipboard"), 1.);
    }

    /// Save the page as a picture: suffix ".png" or ".pdf".
    fn export(&mut self, ctx: &egui::Context, suffix: &str) {
        let member = self.name.rsplit(" › ").next().unwrap_or("nfo"); // of an NFO in a ZIP: its own name
        let stem = Path::new(member).file_stem().unwrap_or_default().to_string_lossy();
        let kind = if suffix == ".pdf" { "PDF" } else { "PNG" };
        let dialog = rfd::FileDialog::new().add_filter(kind, &[&suffix[1..]]).set_file_name(format!("{stem}{suffix}"));
        let Some(path) = dialog.save_file() else { return };
        let font = FontId::monospace(BASE_PX * self.zoom as f32 / 100.);
        let theme = core::theme_colors(&self.cfg);
        let made = export::render(ctx, &self.page, &self.cfg, &theme, &font, self.cell.0, self.cell.1, self.text_only);
        let png = !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        match made {
            Ok(pic) if png && pic.w.max(pic.h) > export::MAX_PNG => self.toast(ctx, tr("This NFO is too long for a PNG. Save it as a PDF instead."), 5.),
            Ok(pic) => {
                if let Err(error) = export::save(&pic, &path) {
                    self.toast(ctx, tr("Could not save the image: {error}").replace("{error}", &error), 5.);
                }
            }
            Err(size) => self.toast(ctx, tr("Could not save the image: {error}").replace("{error}", &size), 5.),
        }
    }

    fn find(&mut self, query: &str) {
        self.page.find(query);
        self.show_match();
    }

    fn step(&mut self, direction: isize) {
        let n = self.page.matches.len();
        if n > 0 {
            self.page.current = (self.page.current + n).wrapping_add_signed(direction) % n;
            self.show_match();
        }
    }

    /// Scroll the current match into the view, unless it is there already.
    fn show_match(&mut self) {
        // the positions as the view lays them out: one and a half rows above, two columns left of the page
        let Some(&(r, a, _)) = self.page.matches.get(self.page.current) else { return };
        let (cw, ch) = self.cell;
        let pos = Vec2::new((a as f32 + 2.) * cw, (r as f32 + 1.5) * ch);
        let mut to = self.offset;
        for axis in 0..2 {
            if !(self.offset[axis]..=self.offset[axis] + self.visible[axis] - ch).contains(&pos[axis]) {
                to[axis] = (pos[axis] - self.visible[axis] / 3.).max(0.);
            }
        }
        self.scroll_to = Some(to);
    }

    // ------------------------------------------------------------ keys

    fn keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let hit = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));
        let ctrl = Modifiers::COMMAND;
        if hit(ctrl, Key::O) {
            self.open_dialog(ctx);
        }
        if hit(ctrl, Key::F) && self.path.is_some() {
            self.search.get_or_insert_default();
            self.focus_search = true;
        }
        if hit(ctrl, Key::T) && self.path.is_some() {
            self.set_text_only(ctx, !self.text_only);
        }
        if hit(ctrl, Key::Z) && !typing {
            self.undo(ctx);
        }
        if hit(ctrl, Key::A) && !typing && !self.page.lines.is_empty() {
            let last = self.page.lines.len() - 1;
            self.page.sel = Some(((0, 0), (last, self.page.lines[last].len().saturating_sub(1))));
        }
        if hit(ctrl, Key::Plus) || hit(ctrl, Key::Equals) {
            self.zoom_step(1);
        }
        if hit(ctrl, Key::Minus) {
            self.zoom_step(-1);
        }
        if hit(ctrl, Key::Num0) {
            self.fit = false;
            self.zoom = self.cfg["zoom"].as_i64().unwrap_or(100);
        }
        if hit(ctrl, Key::S) && self.path.is_some() {
            self.export(ctx, ".png");
        }
        if hit(ctrl, Key::Comma) {
            self.settings = Some(sidebar::GENERAL);
        }
        if hit(ctrl, Key::M) {
            self.change(ctx, "music", json!(self.cfg["music"] != true));
        }
        if hit(Modifiers::NONE, Key::F5) || hit(ctrl, Key::R) {
            self.reread(ctx);
        }
        if hit(ctrl, Key::W) || hit(ctrl, Key::Q) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close); // one window: closing it quits
        }
        // Ctrl and ? (with Shift on most keyboards, so the modifiers are not matched exactly)
        if ctx.input_mut(|i| {
            let at = i.events.iter().position(|e| matches!(e, egui::Event::Key { key: Key::Questionmark, pressed: true, modifiers, .. } if modifiers.command));
            at.map(|at| i.events.remove(at)).is_some()
        }) {
            self.shortcuts = true;
        }
        if hit(Modifiers::NONE, Key::F9) {
            self.change(ctx, "sidebar", json!(self.cfg["sidebar"] != true));
        }
        if hit(Modifiers::NONE, Key::Escape) && self.search.take().is_some() {
            self.page.find("");
        }
        for event in ctx.input(|i| i.events.clone()) {
            match event {
                egui::Event::Copy if !typing && self.path.is_some() => self.copy(ctx),
                egui::Event::Paste(text) if !typing => match core::read_look_code(&text, core::LOOK, core::LOOK_CHOICES) {
                    Some(look) => {
                        self.apply_look(ctx, look);
                        self.toast(ctx, tr("Look applied (Ctrl+Z undoes it)"), 2.);
                    }
                    None => self.toast(ctx, tr("The clipboard holds no NfoGlow look code"), 2.),
                },
                // Ctrl and the wheel: one zoom step per notch
                egui::Event::MouseWheel { delta, modifiers, .. } if modifiers.command && delta.y != 0. => self.zoom_step(if delta.y > 0. { 1 } else { -1 }),
                _ => {}
            }
        }
        if typing || self.path.is_none() {
            return;
        }
        // any key ends the effect on opening (not the short glitch)
        if self.opening.as_ref().is_some_and(|o| o.0 != "glitch") && ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))) {
            self.opening = None;
            return;
        }
        let page = self.visible.y * 0.9;
        let ch = self.cell.1;
        let mut by = None;
        // Shift+Space comes before Space: consume_key matches "logically", so Space alone would also take Shift+Space
        for (m, k, dy) in [(Modifiers::SHIFT, Key::Space, -page), (Modifiers::NONE, Key::Space, page), (Modifiers::NONE, Key::PageDown, page),
            (Modifiers::NONE, Key::PageUp, -page), (Modifiers::NONE, Key::ArrowDown, ch * 3.), (Modifiers::NONE, Key::ArrowUp, -ch * 3.),
            (Modifiers::NONE, Key::Home, -1e9), (Modifiers::NONE, Key::End, 1e9)] {
            if hit(m, k) {
                by = Some(dy);
            }
        }
        if let Some(dy) = by {
            let bottom = (self.content.y - self.visible.y).max(0.);
            self.scroll_to = Some(Vec2::new(self.offset.x, (self.offset.y + dy).clamp(0., bottom)));
        }
        for (k, step) in [(Key::ArrowLeft, -1), (Key::ArrowRight, 1)] {
            if hit(Modifiers::NONE, k) {
                self.browse(ctx, step);
            }
        }
    }

    // ------------------------------------------------------------ the parts of the window

    fn command_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let open = self.path.is_some();
        if ui::icon_button(ui, ui::OPEN, &tr("Open (Ctrl+O)"), false).clicked() {
            self.open_dialog(&ctx);
        }
        let recent: Vec<String> = self.cfg["recent"].as_array().unwrap().iter().filter_map(|p| p.as_str().map(String::from)).collect();
        ui.add_enabled_ui(!recent.is_empty(), |ui| {
            ui.menu_button(ui::icon(ui, ui::RECENT, 16.), |ui| {
                for path in &recent {
                    let name = Path::new(path).file_name().unwrap_or_default().to_string_lossy().into_owned();
                    if ui.button(name).on_hover_text(path).clicked() {
                        self.load(&ctx, Path::new(path), true);
                    }
                }
                ui.separator();
                if ui.button(tr("Clear list")).clicked() {
                    self.set(&ctx, "recent", json!([]));
                }
            })
            .response
            .on_hover_text(tr("Recent files"));
        });
        ui.add_space(8.);
        // art or text: two halves of one button
        ui.add_enabled_ui(open, |ui| {
            ui.spacing_mut().item_spacing.x = 2.;
            for (text_only, name, hint) in [(false, "Art", "The NFO with its graphics"), (true, "Text", "Only the text, without the graphics (Ctrl+T)")] {
                let button = egui::Button::selectable(self.text_only == text_only, tr(name)).min_size(Vec2::new(64., 32.));
                if ui.add(button).on_hover_text(tr(hint)).clicked() && self.text_only != text_only {
                    self.set_text_only(&ctx, text_only);
                }
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // stays open while the zoom is changed, as the menu of NfoGlow for Linux; the other entries close it
            egui::containers::menu::MenuButton::from_button(egui::Button::new(ui::icon(ui, ui::MENU, 16.)))
                .config(egui::containers::menu::MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside))
                .ui(ui, |ui| self.main_menu(ui)).0.on_hover_text(tr("Main menu"));
            if ui::icon_button(ui, ui::EFFECTS, &tr("Effects (F9)"), self.cfg["sidebar"] == true).clicked() {
                self.change(&ctx, "sidebar", json!(self.cfg["sidebar"] != true));
            }
            ui.add_enabled_ui(open, |ui| {
                if ui::icon_button(ui, ui::SEARCH, &tr("Search (Ctrl+F)"), self.search.is_some()).clicked() {
                    if self.search.take().is_some() {
                        self.page.find("");
                    } else {
                        self.search = Some(String::new());
                        self.focus_search = true;
                    }
                }
            });
        });
    }

    fn main_menu(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let open = self.path.is_some();
        ui.horizontal(|ui| {
            if ui::icon_button(ui, ui::ZOOM_OUT, &tr("Zoom out (Ctrl+−)"), false).clicked() {
                self.zoom_step(-1);
            }
            if ui.button(format!("{} %", self.zoom)).on_hover_text(tr("Default zoom (Ctrl+0)")).clicked() {
                self.fit = false;
                self.zoom = self.cfg["zoom"].as_i64().unwrap_or(100);
            }
            if ui::icon_button(ui, ui::ZOOM_IN, &tr("Zoom in (Ctrl++)"), false).clicked() {
                self.zoom_step(1);
            }
        });
        if self.cfg["zoom"] != self.zoom && !self.fit && ui.button(tr("Save as default")).on_hover_text(tr("Open every NFO at this zoom from now on")).clicked() {
            self.set(&ctx, "zoom", json!(self.zoom));
            ui.close();
        }
        ui.separator();
        let mut fit = self.fit;
        if ui.add_enabled(open, egui::Checkbox::new(&mut fit, tr("Fit width"))).changed() {
            // on: zoom so the NFO fills the width and follow the window; off: back to the zoom before
            if fit {
                self.zoom_before_fit = Some(self.zoom);
            } else {
                self.zoom = self.zoom_before_fit.unwrap_or_else(|| self.cfg["zoom"].as_i64().unwrap_or(100));
            }
            self.fit = fit;
            ui.close();
        }
        if ui.add_enabled(open, egui::Button::new(tr("Reload"))).clicked() {
            self.reread(&ctx);
            ui.close();
        }
        let mut music = self.cfg["music"] == true;
        if ui.checkbox(&mut music, tr("Music")).on_hover_text(tr("Music on or off")).changed() {
            self.change(&ctx, "music", json!(music));
            ui.close();
        }
        egui::containers::menu::SubMenuButton::from_button(egui::Button::new(tr("Encoding")).right_text("›")).ui(ui, |ui| {
            for (key, name) in ENCODINGS {
                if ui.radio(self.cfg["encoding"] == key, tr(name)).clicked() {
                    self.change(&ctx, "encoding", json!(key));
                    ui.close();
                }
            }
        });
        ui.separator();
        egui::containers::menu::SubMenuButton::from_button(egui::Button::new(tr("Export")).right_text("›")).ui(ui, |ui| {
            if ui.add_enabled(open, egui::Button::new(tr("Copy to clipboard"))).clicked() {
                ctx.copy_text(self.page.text().join("\n"));
                self.toast(&ctx, tr("Copied to the clipboard"), 1.);
                ui.close();
            }
            for (name, suffix) in [("Save as PNG…", ".png"), ("Save as PDF…", ".pdf")] {
                if ui.add_enabled(open, egui::Button::new(tr(name))).clicked() {
                    self.export(&ctx, suffix);
                    ui.close();
                }
            }
        });
        egui::containers::menu::SubMenuButton::from_button(egui::Button::new(tr("Look")).right_text("›")).ui(ui, |ui| {
            if ui.button(tr("Copy look")).clicked() {
                ctx.copy_text(core::look_code(&self.cfg, core::LOOK));
                self.toast(&ctx, tr("Look copied to the clipboard"), 2.);
                ui.close();
            }
            if ui.button(tr("Paste look")).on_hover_text(format!("{}+V", tr("Ctrl"))).clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                ui.close();
            }
        });
        ui.separator();
        if ui.add(egui::Button::new(tr("Preferences")).shortcut_text(format!("{}+,", tr("Ctrl")))).clicked() {
            self.settings = Some(sidebar::GENERAL);
            ui.close();
        }
        if ui.add(egui::Button::new(tr("Keyboard shortcuts")).shortcut_text(format!("{}+?", tr("Ctrl")))).clicked() {
            self.shortcuts = true;
            ui.close();
        }
        if ui.button(tr("About NfoGlow")).clicked() {
            self.about.get_or_insert_default();
            ui.close();
        }
    }

    /// The open file changed on disk (an editor saved it): read it again, quietly, keeping the place. Looked at once a
    /// second; a change is read when it holds still for a moment, since editors write in several steps.
    fn watch(&mut self, ctx: &egui::Context) {
        let Some(path) = self.path.clone() else { return };
        let now = ctx.input(|i| i.time);
        if now >= self.next_look {
            let seen = stamp(&path);
            self.next_look = now + 1.;
            if seen.is_some() && seen != self.stamp {
                if seen == self.stamp_seen {
                    self.stamp = seen; // also when reading fails (too large now): one message, not one every second
                    self.reread(ctx);
                    self.opening = None; // no effect on opening when it is only read again
                } else {
                    (self.stamp_seen, self.next_look) = (seen, now + 0.25);
                }
            }
        }
        ctx.request_repaint_after_secs((self.next_look - now).max(0.05) as f32);
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.shortcuts {
            return;
        }
        let modal = egui::Modal::new(egui::Id::new("shortcuts")).show(ctx, |ui| {
            ui.set_width(460.);
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr("Keyboard shortcuts")).size(20.).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(ui::icon(ui, ui::CLOSE, 14.)).frame(false)).clicked() {
                        self.shortcuts = false;
                    }
                });
            });
            ui.add_space(8.);
            egui::ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.75).show(ui, |ui| {
                for (group, keys) in SHORTCUTS {
                    ui::heading(ui, &tr(group));
                    ui::card(ui, |ui| {
                        for (i, (combo, what)) in keys.iter().enumerate() {
                            if i > 0 {
                                ui::divider(ui);
                            }
                            egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 6)).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(tr(what)).color(ui.visuals().strong_text_color()));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        for (j, key) in combo.iter().rev().enumerate() {
                                            if j > 0 {
                                                ui.label("+");
                                            }
                                            let name = if key.chars().count() > 1 && !key.starts_with('F') { tr(key) } else { key.to_string() };
                                            egui::Frame::new().stroke(ui.visuals().widgets.inactive.bg_stroke).corner_radius(4).inner_margin(egui::Margin::symmetric(6, 1))
                                                .show(ui, |ui| ui.label(RichText::new(name).monospace()));
                                        }
                                    });
                                });
                            });
                        }
                    });
                }
            });
        });
        if modal.should_close() {
            self.shortcuts = false;
        }
    }

    fn search_bar(&mut self, ui: &mut egui::Ui) {
        let Some(mut query) = self.search.clone() else { return };
        let edit = ui.add(egui::TextEdit::singleline(&mut query).hint_text(tr("Search")).desired_width(260.));
        if std::mem::take(&mut self.focus_search) {
            edit.request_focus();
        }
        if edit.changed() {
            self.find(&query);
            self.search = Some(query.clone());
        }
        // Enter: the next match, with Shift the one before (the field keeps the focus)
        if edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            self.step(if ui.input(|i| i.modifiers.shift) { -1 } else { 1 });
            edit.request_focus();
        }
        if ui.button(tr("Previous match")).on_hover_text(tr("Previous match (Shift+Enter)")).clicked() {
            self.step(-1);
        }
        if ui.button(tr("Next match")).on_hover_text(tr("Next match (Enter)")).clicked() {
            self.step(1);
        }
        ui.weak(match (self.page.matches.len(), query.is_empty()) {
            (0, true) => String::new(),
            (0, false) => tr("No matches"),
            (n, _) => format!("{} / {n}", self.page.current + 1),
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        if self.path.is_none() {
            return;
        }
        let here = self.siblings.iter().position(|p| Some(p) == self.path.as_ref()).filter(|_| self.siblings.len() > 1);
        let position = here.map(|i| format!("{} / {}  ·  ", i + 1, self.siblings.len())).unwrap_or_default();
        ui.weak(format!("{position}{} × {}  ·  {}  ·  {:.1} KB", self.page.cols, self.page.lines.len(), self.page.label, self.data.len() as f32 / 1024.));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.weak(format!("{} %", self.zoom)));
    }

    /// The view; the C64 theme has its light blue border around it, C64_FRAME of the width on every side.
    fn view(&mut self, ui: &mut egui::Ui) {
        if self.path.is_none() || self.cfg["theme"] != "c64" {
            return self.page_view(ui, 0.);
        }
        let frame = (core::C64_FRAME * ui.available_width()).round();
        let inner = ui.available_rect_before_wrap().shrink(frame);
        let theme = core::theme_colors(&self.cfg);
        ui.painter().rect_filled(inner, 0, view::rgb(if self.page.colors.is_some() { core::VGA[0] } else { theme.bg }));
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.set_clip_rect(inner.intersect(ui.clip_rect()));
            self.page_view(ui, frame);
        });
    }

    /// frame: the width of the border already taken off the room of ui.
    fn page_view(&mut self, ui: &mut egui::Ui, frame: f32) {
        let ctx = ui.ctx().clone();
        let theme = core::theme_colors(&self.cfg);
        if self.path.is_none() {
            let logo = (self.logo_texture(&ctx).id(), Vec2::splat(128.));
            return ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() / 2. - 150.).max(16.));
                ui.add(egui::Image::new(logo));
                ui.add_space(16.);
                ui.label(RichText::new(tr("No NFO open")).size(24.).strong());
                ui.add_space(4.);
                ui.label(RichText::new(tr("Drop a file here or press Ctrl+O.")).weak());
                ui.add_space(16.);
                let open = egui::Button::new(RichText::new(tr("Open…")).color(if ui.visuals().dark_mode { Color32::BLACK } else { Color32::WHITE })).fill(ui::accent(ui)).corner_radius(16).min_size(Vec2::new(120., 34.));
                if ui.add(open).clicked() {
                    self.open_dialog(&ctx);
                }
            }).inner;
        }
        if self.fit && self.path.is_some() && !self.page.lines.is_empty() {
            // the largest zoom at which the page still fits the width (font widths do not grow exactly in proportion)
            let avail = ui.available_width();
            let width = |z: i64| {
                let cw = ui.fonts_mut(|f| f.glyph_width(&FontId::monospace(BASE_PX * z as f32 / 100.), 'M'));
                (self.page.cols as f32 * cw + 4. * cw).ceil()
            };
            let mut pct = ((self.zoom as f32 * avail / width(self.zoom)) as i64).clamp(30, 250);
            while pct < 250 && width(pct) <= avail {
                pct += 1;
            }
            while pct > 30 && width(pct) > avail {
                pct -= 1;
            }
            self.zoom = pct;
        }
        let font = FontId::monospace(BASE_PX * self.zoom as f32 / 100.);
        let (cw, ch) = ui.fonts_mut(|f| (f.glyph_width(&font, 'M'), f.row_height(&font).ceil()));
        self.cell = (cw, ch);
        let pad = Vec2::new(cw * 2., ch * 1.5);
        let size = (Vec2::new(self.page.cols as f32 * cw, self.page.lines.len() as f32 * ch) + 2. * pad).ceil();
        if std::mem::take(&mut self.resize) && self.maximize == 0 && !ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)) {
            // the window as big as the NFO, within 90 % of the screen; tiny or empty files get a minimum size
            let bars = ctx.content_rect().size() - ui.available_size() - Vec2::splat(2. * frame);
            let screen = ctx.input(|i| i.viewport().monitor_size).unwrap_or(Vec2::new(1280., 720.)) * 0.9;
            let mut want = size;
            if self.cfg["theme"] == "c64" {
                // the border is a share of the width: width = page / (1 - 2 * share), and it adds to the height twice
                want.x = (size.x / (1. - 2. * core::C64_FRAME)).floor();
                want.y += 2. * (core::C64_FRAME * want.x).round();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize((want + bars).min(screen).max(Vec2::new(600., 420.))));
            (self.settle, self.settle_until) = (SETTLE_FRAMES, ctx.input(|i| i.time) + SETTLE_SECONDS);
        }
        // the clock of everything that moves, and the effect on opening while it runs
        let now = ctx.input(|i| i.time);
        let chars = fx::total_chars(&self.page);
        if let Some(o) = self.opening.as_mut().filter(|o| o.1.is_nan()) {
            o.1 = now;
        }
        self.opening.take_if(|o| now - o.1 >= fx::opening_end(&o.0, &self.name, chars) as f64);
        let running = self.opening.clone().map(|o| (o.0, (now - o.1) as f32));
        let opening = running.as_ref().map(|o| (o.0.as_str(), o.1));
        let (art, ansi) = (!self.text_only, self.page.colors.is_some());
        let on = |key: &str| self.cfg[key] == true && art;
        let bg = if ansi { core::VGA[0] } else { theme.bg };
        let moving = opening.is_some()
            || on("scroller")
            || (on("crt") && on("crt_live"))
            || (!ansi && ["cycle", "stars", "sparkle", "wobble", "sheen"].iter().any(|k| on(k)))
            || (!ansi && on("soft") && self.cfg["glow_mode"] == "halo" && (on("pulse") || on("neon_flicker")));
        if moving {
            ctx.request_repaint_after_secs(1. / 30.); // 30 frames a second are plenty for this and spare the processor
        }
        let t = if moving { now as f32 } else { 0. };
        let frame = ui.available_rect_before_wrap();
        // what only the view's own render pass can do (post.rs); without a graphics card for it, the old ways
        let look = post::Look {
            chroma: on("chroma"),
            mask: if on("crt") { match self.cfg["crt_mask"].as_str() { Some("dots") => 1, Some("stripes") => 2, _ => 0 } } else { 0 },
            grown: match opening { Some(("crt", secs)) => fx::power_on_grown(secs), _ => 1. },
            cover: matches!(opening, Some(("modem" | "dither" | "rain", _))),
        };
        let post = self.render.is_some() && look.needed();
        let slot = ui.painter().add(egui::Shape::Noop); // the view goes here, under the scroll bars
        let into = |name: &str| ui.painter().clone().with_layer_id(post::layer(name));
        let (scene, under, cover) = if post { (into(post::SCENE), Some(into(post::UNDER)), into(post::COVER)) } else { (ui.painter().clone(), None, ui.painter().clone()) };
        let cover = if look.cover { cover } else { scene.clone() };
        if on("stars") && !ansi {
            for p in std::iter::once(&scene).chain(under.as_ref()) {
                fx::stars(&p.clone().with_clip_rect(frame.intersect(ui.clip_rect())), frame, self.cfg["stars_mode"] == "warp", theme.text, self.zoom as f32 / 100., t);
            }
        }
        // egui fades the edges of a scroll area towards the colour of the panel around it while there is more to scroll;
        // in the C64 theme that is the light border, so the top and bottom of the page lit up. NfoGlow for Linux has no
        // such fade: the page stays as it is up to its edge
        ui.spacing_mut().scroll.fade.strength = 0.;
        let mut area = egui::ScrollArea::both().auto_shrink(false);
        if let Some(to) = self.scroll_to.take() {
            area = area.scroll_offset(to);
        }
        let out = area.show_viewport(ui, |ui, visible| {
            let (rect, response) = ui.allocate_exact_size(size.max(ui.available_size()), egui::Sense::click_and_drag());
            // centred while the window is wider than the NFO
            let origin = Pos2::new(rect.left() + ((rect.width() - size.x) / 2.).max(0.) + pad.x, rect.top() + pad.y);
            let grid = view::Grid { origin, cw, ch, ppp: ctx.pixels_per_point() };
            let rows = self.page.lines.len();
            let lo = (((visible.top() - pad.y) / ch).floor().max(0.) as usize).min(rows);
            let hi = (((visible.bottom() - pad.y) / ch).ceil().max(0.) as usize).min(rows);
            let cell = |pos: Pos2| {
                let (r, c) = grid.cell_at(pos);
                (r.min(rows.saturating_sub(1)), c)
            };
            let link = response.hover_pos().map(cell).and_then(|(r, c)| self.page.link_at(r, c).map(String::from));
            if response.hovered() {
                ctx.set_cursor_icon(if link.is_some() { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Text });
            }
            match response.interact_pointer_pos().map(cell) {
                Some(at) if response.drag_started() => (self.drag_from, self.page.sel) = (Some(at), None),
                Some(at) if response.dragged() => {
                    self.page.sel = self.drag_from.map(|from| (from, at));
                    // past the top or bottom edge: keep scrolling (and selecting) while the pointer stays there
                    let (view, pos) = (ui.clip_rect(), ctx.input(|i| i.pointer.interact_pos()).unwrap_or(Pos2::ZERO));
                    let over = if pos.y < view.top() { pos.y - view.top() } else if pos.y > view.bottom() { pos.y - view.bottom() } else { 0. };
                    if over != 0. {
                        let step = (over / 2.).clamp(-60., 60.) * ctx.input(|i| i.stable_dt).min(0.1) / 0.03; // up to 60 points per 30 ms
                        let bottom = (self.content.y - self.visible.y).max(0.);
                        self.scroll_to = Some(Vec2::new(self.offset.x, (self.offset.y + step).clamp(0., bottom)));
                        ctx.request_repaint();
                    }
                }
                _ => {}
            }
            if response.clicked() {
                self.page.sel = None;
                if self.opening.as_ref().is_some_and(|o| o.0 != "glitch") {
                    self.opening = None; // a click ends the effect on opening
                } else if let Some(url) = link {
                    let url = if url.to_lowercase().starts_with("www.") { format!("https://{url}") } else { url };
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                }
            }
            let seen = ui.clip_rect();
            let moves = view::Fx { t, level: self.level, neon: opening.filter(|o| o.0 == "neon").map(|o| o.1), view: seen };
            let painter = scene.clone().with_clip_rect(ui.clip_rect());
            let cover = cover.clone().with_clip_rect(ui.clip_rect());
            let under = under.clone().map(|u| u.with_clip_rect(ui.clip_rect()));
            let mut paint = |painter: &egui::Painter, grid: view::Grid| {
                view::paint(painter, &mut self.cache, &self.page, self.page_id, &self.cfg, &theme, &font, grid, lo, hi, self.text_only, &moves);
            };
            paint(&painter, grid);
            match opening {
                Some(("modem", secs)) => {
                    let at = fx::reveal_cell(&self.page, (secs * (chars as f32 / 4.).max(2400.)) as usize);
                    fx::modem(&painter, &cover, grid, seen, at, bg, if ansi { core::VGA[7] } else { theme.text });
                }
                Some(("glitch", secs)) => {
                    // the page torn into shifted strips
                    let (strips, flash) = fx::glitch(seen, secs);
                    for (strip, dx) in strips {
                        let torn = painter.with_clip_rect(strip.intersect(seen));
                        torn.rect_filled(strip, 0, view::rgb(bg));
                        paint(&torn, view::Grid { origin: origin + Vec2::new(dx, 0.), ..grid });
                    }
                    painter.rect_filled(seen, 0, Color32::WHITE.gamma_multiply(flash));
                }
                Some(("crt", secs)) if !post => fx::power_on(&painter, seen, secs, false),
                Some(("dither", secs)) => fx::dither(&cover, &mut self.tiles, seen, secs, ch, bg),
                Some(("rain", secs)) => fx::rain(&[Some(&painter), under.as_ref()], &cover, grid, &self.page, seen, secs, &theme, bg, &font),
                _ => {}
            }
        });
        (self.offset, self.visible, self.content) = (out.state.offset, out.inner_rect.size(), out.content_size);
        let screen = out.inner_rect;
        let painter = ui.painter().with_clip_rect(screen);
        if post {
            let shape = self.post.draw(&ctx, self.render.as_ref().unwrap(), frame.intersect(ui.clip_rect()), view::rgb(bg), view::rgb(bg), look);
            ui.painter().set(slot, shape);
            if let Some(("crt", secs)) = opening {
                fx::power_on(&painter, screen, secs, true); // the black and the light over the stretched picture
            }
        }
        if on("scroller") {
            let text = self.cfg["scroller_text"].as_str().unwrap_or("").trim();
            let greeting = tr("NfoGlow presents {name} ... greetings to everyone who still draws by hand ... F9 opens the effects ... ");
            let text = if text.is_empty() { &greeting } else { text };
            // not format!(): a "{" typed by the user must not break it
            fx::scroller(&painter, screen, t, &format!("   {}   ", text.replace("{name}", &self.name).replace('\n', " ")));
        }
        if on("crt") {
            view::crt(ui, &mut self.cache, screen, look.mask == 0 || !post); // a colour mask replaces the scanlines
            if on("crt_live") {
                fx::crt_live(&painter, &mut self.tiles, screen, t);
            }
        }
        // the tape loader and the DOS boot lie over everything while they run
        match opening {
            Some(("loader", secs)) => fx::loader(&painter, screen, secs, &self.name),
            Some(("dos", secs)) => {
                let mtime = self.path.as_ref().and_then(|p| p.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
                fx::dos(&painter, screen, secs, &self.name, self.data.len() as u64, mtime as i64);
            }
            _ => {}
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(path) = ctx.input(|i| i.raw.dropped_files.first().and_then(|f| f.path.clone())) {
            self.load(&ctx, &path, true);
        }
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if self.maximize > 0 {
            // the window opens as the last one was left: shown first and maximized then. Created maximized, Windows
            // marked it so but kept its small size, and the first click on the button made it small
            self.maximize = if maximized { 0 } else { self.maximize - 1 }; // no window manager (a test): give up
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            ctx.request_repaint();
        } else if self.cfg["maximized"] != maximized {
            self.set(&ctx, "maximized", json!(maximized));
        }
        // the sidebar takes its width from the window, not from the page: opened, the window grows by it (as far as the
        // screen allows), closed, it shrinks back
        let sidebar = self.cfg["sidebar"] == true;
        if let Some(was) = self.sidebar_was.replace(sidebar).filter(|&was| was != sidebar) {
            let (info, floor) = (ctx.input(|i| i.viewport().clone()), 420.);
            if !info.maximized.unwrap_or(false) && !info.fullscreen.unwrap_or(false) {
                if let Some(inner) = info.inner_rect {
                    // closed: back to the width before, also when the screen edge let it grow by less (made narrower by
                    // hand meanwhile: just the sidebar off)
                    let narrower = inner.width() - SIDEBAR;
                    let before = self.width_before.take().filter(|&b| narrower < b && inner.width() >= b);
                    let width = if was { before.unwrap_or(narrower).max(floor) } else { inner.width() + SIDEBAR };
                    if !was {
                        self.width_before = Some(inner.width());
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(width, inner.height())));
                    (self.settle, self.settle_until) = (SETTLE_FRAMES, ctx.input(|i| i.time) + SETTLE_SECONDS);
                }
            }
        }
        // for a moment after the start and after every resize: no bigger than the screen the taskbar leaves, not over its edge
        let clock = ctx.input(|i| i.time);
        if self.settle_until < 0. {
            (self.settle, self.settle_until) = (SETTLE_FRAMES, clock + SETTLE_SECONDS); // the first frame: starting the program took its time
        }
        if self.settle > 0 && clock < self.settle_until {
            self.settle -= u8::from(fit_to_screen(&ctx)); // only a frame in which the window could be looked at counts
            ctx.request_repaint_after_secs(0.05);
        }
        if std::mem::take(&mut self.system_look) {
            self.use_system_theme(&ctx);
        }
        self.keys(&ctx);
        self.watch(&ctx);
        self.shortcuts_window(&ctx);
        self.music_frame(&ctx);
        self.settings_window(&ctx);
        self.about_window(&ctx);
        let bar = egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(egui::Margin::symmetric(8, 6));
        egui::Panel::top("bar").frame(bar).show(ui, |ui| ui.horizontal(|ui| self.command_bar(ui)));
        if self.search.is_some() {
            egui::Panel::top("search").show(ui, |ui| ui.horizontal(|ui| self.search_bar(ui)));
        }
        egui::Panel::bottom("status").show(ui, |ui| ui.horizontal(|ui| self.status_bar(ui)));
        if self.cfg["sidebar"] == true {
            egui::Panel::right("effects").resizable(false).exact_size(SIDEBAR).frame(egui::Frame::new().fill(ui.visuals().panel_fill)).show(ui, |ui| self.sidebar(ui));
        }
        let theme = core::theme_colors(&self.cfg);
        let bg = if self.page.colors.is_some() { core::VGA[0] } else { theme.bg };
        let fill = if self.path.is_none() { ui.visuals().panel_fill } else if self.cfg["theme"] == "c64" { view::rgb(core::C64_BORDER) } else { view::rgb(bg) };
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(fill)).show(ui, |ui| self.view(ui));
        if let Some((text, until)) = self.toast.clone() {
            let left = until - ctx.input(|i| i.time);
            if left > 0. {
                egui::Area::new("toast".into()).anchor(egui::Align2::CENTER_BOTTOM, [0., -48.]).show(&ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(text));
                });
                ctx.request_repaint_after_secs(left as f32);
            } else {
                self.toast = None;
            }
        }
    }
}

/// When a file was written and how big it is, to notice that it changed.
fn stamp(path: &Path) -> Option<(Option<std::time::SystemTime>, u64)> {
    path.metadata().ok().map(|m| (m.modified().ok(), m.len()))
}

/// The keyboard shortcuts, in groups: (keys, what they do); keys are joined by "+", alternatives by " / ". Key names
/// that differ between languages are translated (Ctrl is Strg in German). As in NfoGlow for Linux.
const SHORTCUTS: [(&str, &[(&[&str], &str)]); 4] = [
    ("General", &[(&["Ctrl", "O"], "Open"), (&["Ctrl", ","], "Preferences"), (&["Ctrl", "?"], "Keyboard shortcuts"),
        (&["Ctrl", "W"], "Close the window"), (&["Ctrl", "Q"], "Quit")]),
    ("View", &[(&["Ctrl", "T"], "Art or text only"), (&["F9"], "Effects sidebar"), (&["Ctrl", "Z"], "Undo the last change to the look"),
        (&["Ctrl", "M"], "Music on or off"), (&["Ctrl", "+"], "Zoom in"), (&["Ctrl", "−"], "Zoom out"), (&["Ctrl", "0"], "Default zoom"),
        (&["Ctrl", "Mouse wheel"], "Zoom"), (&["F5"], "Reload")]),
    ("Moving around", &[(&["←"], "Previous NFO in the folder"), (&["→"], "Next NFO in the folder"), (&["Space"], "Page down"),
        (&["Shift", "Space"], "Page up"), (&["↑"], "Scroll up"), (&["↓"], "Scroll down"), (&["Home"], "Top"), (&["End"], "Bottom")]),
    ("Text", &[(&["Ctrl", "F"], "Search"), (&["Enter"], "Next match"), (&["Shift", "Enter"], "Previous match"), (&["Ctrl", "C"], "Copy"),
        (&["Ctrl", "A"], "Select all"), (&["Ctrl", "S"], "Save as PNG")]),
];

const SETTLE_FRAMES: u8 = 6; // the window needs a few frames to take a new size and position
const SETTLE_SECONDS: f64 = 10.; // and gives up after this, when the window never says where it is

/// The primary screen in physical pixels, without the part the taskbar covers: (screen width and height, then the work
/// area as left, top, right, bottom). Windows only; elsewhere there is nothing to ask.
#[cfg(windows)]
fn screen_and_work_area() -> Option<([i32; 2], [i32; 4])> {
    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn SystemParametersInfoW(action: u32, param: u32, data: *mut Rect, flags: u32) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
    }
    const SPI_GETWORKAREA: u32 = 0x0030;
    let mut work = Rect { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: SPI_GETWORKAREA writes one RECT, and `work` is one
    let ok = unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work, 0) } != 0;
    // SAFETY: no pointers; 0 and 1 are SM_CXSCREEN and SM_CYSCREEN
    let screen = unsafe { [GetSystemMetrics(0), GetSystemMetrics(1)] };
    (ok && work.right > work.left && work.bottom > work.top).then_some((screen, [work.left, work.top, work.right, work.bottom]))
}

#[cfg(not(windows))]
fn screen_and_work_area() -> Option<([i32; 2], [i32; 4])> {
    None
}

/// Keep the window on the screen: no bigger than what the taskbar leaves free, and not hanging over its edges. Windows
/// puts a new window where it likes (stepping it down and to the right with every start) and a window of 860 x 900 does
/// not fit a 1366 x 768 screen. Only a window on the primary screen is touched; a window the user has put elsewhere, or
/// has maximized, stays as it is. Called for a moment after the start and after a resize, because the window reports its
/// size and position a frame or more after the command.
fn fit_to_screen(ctx: &egui::Context) -> bool {
    let info = ctx.input(|i| i.viewport().clone());
    if info.maximized.unwrap_or(false) || info.fullscreen.unwrap_or(false) {
        return true;
    }
    let Some((screen, area)) = screen_and_work_area() else { return true }; // nothing to ask: nothing to do
    let (Some(outer), Some(inner)) = (info.outer_rect, info.inner_rect) else { return false }; // the window has not said yet
    let ppp = info.native_pixels_per_point.unwrap_or_else(|| ctx.pixels_per_point()).max(0.5);
    let to_points = |x: i32, y: i32| Pos2::new(x as f32 / ppp, y as f32 / ppp);
    let primary = egui::Rect::from_min_max(Pos2::ZERO, to_points(screen[0], screen[1]));
    if !primary.contains(outer.center()) {
        return true;
    }
    let work = egui::Rect::from_min_max(to_points(area[0], area[1]), to_points(area[2], area[3]));
    // the frame around the drawing area (title bar, borders) takes its share of the free screen
    let frame = outer.size() - inner.size();
    let smallest = Vec2::new(420., 300.); // the minimum of the window
    let size = inner.size().min((work.size() - frame).max(smallest));
    if (size - inner.size()).abs().max_elem() > 0.5 {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        return true; // the position follows when the new size has arrived
    }
    let at = Pos2::new(
        outer.min.x.clamp(work.min.x, (work.max.x - outer.width()).max(work.min.x)),
        outer.min.y.clamp(work.min.y, (work.max.y - outer.height()).max(work.min.y)),
    );
    if at.distance(outer.min) > 0.5 {
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(at));
    }
    true
}

/// The same for a second window (Preferences, About), called in every frame of it: only for the first moments after it
/// opened, so that the user can move it where they like afterwards. `closed` forgets the window for its next opening.
fn fit_new_window(ctx: &egui::Context, key: &str, closed: bool) {
    let id = egui::Id::new(("settle", key));
    if closed {
        ctx.data_mut(|d| d.remove::<(u8, f64)>(id));
        return;
    }
    let now = ctx.input(|i| i.time);
    let (left, until) = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || (SETTLE_FRAMES, now + SETTLE_SECONDS)));
    if left > 0 && now < until {
        let left = left - u8::from(fit_to_screen(ctx));
        ctx.data_mut(|d| d.insert_temp(id, (left, until)));
        ctx.request_repaint_after_secs(0.05);
    }
}

/// The icon in the title bar and the taskbar.
fn window_icon() -> egui::IconData {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!("../assets/nfoglow-256.png")));
    let mut reader = decoder.read_info().expect("the built-in icon is a PNG");
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut rgba).expect("the built-in icon is a PNG");
    assert_eq!(info.color_type, png::ColorType::Rgba, "the built-in icon has an alpha channel");
    egui::IconData { rgba, width: info.width, height: info.height }
}

/// What Windows shows of the window before the graphics card's first frame reaches the screen (100 to 300 ms on a slow
/// machine, longer on a cold start) and in a strip that a resize uncovers: the background of its window class. winit
/// leaves that empty, which Windows shows white: a white flash before a dark page. So paint it in the page's colour.
/// The window is still hidden here (eframe shows it after the first frame).
#[cfg(windows)]
fn window_background(cc: &eframe::CreationContext<'_>, colour: core::Rgb) {
    use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
    use windows_sys::Win32::{Graphics::Gdi::CreateSolidBrush, UI::WindowsAndMessaging::{SetClassLongPtrW, GCLP_HBRBACKGROUND}};
    let Ok(handle) = cc.window_handle() else { return };
    let RawWindowHandle::Win32(win) = handle.as_raw() else { return };
    let [r, g, b] = colour.map(u32::from);
    // the brush stays for the life of the window class, that is of the program
    unsafe {
        let brush = CreateSolidBrush(r | g << 8 | b << 16); // COLORREF is 0x00BBGGRR
        if !brush.is_null() {
            SetClassLongPtrW(win.hwnd.get() as _, GCLP_HBRBACKGROUND, brush as isize);
        }
    }
}

/// Whether Windows is in dark mode, read where winit reads it: egui only learns it in its first frame, after this.
#[cfg(windows)]
fn windows_is_dark() -> bool {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (key, value) = (wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"), wide("AppsUseLightTheme"));
    let (mut light, mut size) = (1u32, 4u32);
    let found = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&raw mut light).cast(), &mut size) };
    found == 0 && light == 0
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, path] = &args[..] {
        if flag == "--lines" {
            // what the window would show, as JSON: for comparing with NfoGlow for Linux
            let out = match decode::read(Path::new(path)) {
                Ok((data, _)) => {
                    let page = Page::fill(&data, "auto", false);
                    json!({"label": page.label, "lines": page.text()})
                }
                Err(e) => json!({"error": e}),
            };
            println!("{out}");
            return Ok(());
        }
    }
    let languages: Vec<&str> = ["auto"].into_iter().chain(i18n::LANGUAGES.map(|l| l.0)).collect();
    let choices = [core::LOOK_CHOICES, &[("encoding", core::ENCODINGS), ("language", &languages)]].concat();
    if let [flag, from, to] = &args[..] {
        if flag == "--export" {
            // an NFO as a PNG or PDF without a window, in the look of the settings
            let cfg = core::load_settings(&choices);
            let ctx = egui::Context::default();
            set_fonts(&ctx, cfg["font"].as_str().unwrap_or(core::FONT));
            let _ = ctx.run_ui(Default::default(), |_| {}); // the fonts are there after the first frame
            let font = FontId::monospace(BASE_PX * cfg["zoom"].as_i64().unwrap_or(100) as f32 / 100.);
            let (cw, ch) = ctx.fonts_mut(|f| (f.glyph_width(&font, 'M'), f.row_height(&font).ceil()));
            let done = decode::read(Path::new(from)).and_then(|(data, _)| {
                let page = Page::fill(&data, cfg["encoding"].as_str().unwrap(), false);
                let pic = export::render(&ctx, &page, &cfg, &core::theme_colors(&cfg), &font, cw, ch, false)?;
                export::save(&pic, Path::new(to))
            });
            if let Err(error) = done {
                eprintln!("{error}");
                std::process::exit(1);
            }
            return Ok(());
        }
    }
    let first_start = !core::config_path().exists();
    let cfg = core::load_settings(&choices);
    i18n::set_language(cfg["language"].as_str().unwrap());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([860., 640.]).with_min_inner_size([420., 300.]).with_app_id("nfoglow").with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "NfoGlow",
        options,
        Box::new(move |cc| {
            let ctx = &cc.egui_ctx;
            ctx.options_mut(|o| o.zoom_with_keyboard = false); // Ctrl and + zoom the NFO, not the buttons
            let maximize = if cfg["maximized"] == true { 60 } else { 0 };
            let mut app = App {
                zoom: cfg["zoom"].as_i64().unwrap_or(100).clamp(30, 250),
                cfg,
                page: Page::default(),
                page_id: 0,
                path: None,
                data: Vec::new(),
                name: String::new(),
                siblings: Vec::new(),
                text_only: false,
                cache: view::Cache::default(),
                tiles: fx::Tiles::default(),
                opening: None,
                rolls: 0,
                section: None,
                settings: None,
                flash: None,
                asking_reset: false,
                logo: None,
                about: None,
                system_look: first_start,
                maximize,
                player: None,
                level: None,
                last_frame: 0.,
                search: None,
                focus_search: false,
                toast: None,
                scroll_to: None,
                fit: false,
                zoom_before_fit: None,
                offset: Vec2::ZERO,
                visible: Vec2::ZERO,
                content: Vec2::ZERO,
                cell: (8., 16.),
                resize: false,
                sidebar_was: None,
                width_before: None,
                settle: 0,
                settle_until: -1.,
                drag_from: None,
                looks_before: Vec::new(),
                saved_look: None,
                last_change: (String::new(), -9.),
                shortcuts: false,
                stamp: None,
                stamp_seen: None,
                next_look: 0.,
                render: cc.wgpu_render_state.clone(),
                post: post::Post::default(),
            };
            if let Some(rs) = &app.render {
                post::setup(rs);
            }
            set_fonts(ctx, app.cfg["font"].as_str().unwrap_or(core::FONT));
            if app.cfg["random_on_start"] == true {
                app.roll_look(ctx); // a surprise look for this start only: "Keep" in the effects sidebar makes it yours
            }
            set_style(ctx);
            app.apply_music(ctx);
            if let Some(path) = args.first() {
                app.load(ctx, Path::new(path), true);
            }
            #[cfg(windows)]
            window_background(cc, if app.path.is_none() {
                [windows_visuals(windows_is_dark()).panel_fill.r(); 3] // no file: the page is the window's grey (page_view)
            } else if app.cfg["theme"] == "c64" {
                core::C64_BORDER
            } else {
                core::theme_colors(&app.cfg).bg
            });
            Ok(Box::new(app))
        }),
    )
}
