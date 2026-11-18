// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! The settings window, as the preferences of NfoGlow for Linux: four pages (General, Effects, Animation, Music),
//! chosen at the left as in the settings of Windows 11, each with its cards of rows. Every row changes its setting at
//! once.
use crate::i18n::{self, tr};
use crate::sidebar::{Asks, ANIMATION, EFFECTS, GENERAL};
use crate::ui::{self, Icon};
use crate::{core, App, CUSTOM_COLORS, ENCODINGS, THEMES, THEME_RAMPS};
use eframe::egui::{self, RichText, Ui, Vec2};
use serde_json::{json, Value};

const PAGES: [(Icon, &str); 4] = [(ui::SETTINGS, "General"), (ui::EFFECTS, "Effects"), (ui::PLAY, "Animation"), (ui::SOUND, "Music")];

/// The options of a list, translated.
fn options<'a>(list: &[(&'a str, &str)]) -> Vec<(&'a str, String)> {
    list.iter().map(|(k, n)| (*k, tr(n))).collect()
}

/// A colour button; the new colour as "#rrggbb" when one was picked.
fn colour(ui: &mut Ui, hex: &Value) -> Option<String> {
    let mut c = core::hex(hex.as_str().unwrap_or("#000000"));
    ui.color_edit_button_srgb(&mut c).changed().then(|| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
}

impl App {
    /// The settings window, while it is open (self.settings holds its page).
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context) {
        let Some(page) = self.settings else { return };
        let builder = egui::ViewportBuilder::default().with_title(tr("Preferences")).with_inner_size([780., 640.]).with_min_inner_size([560., 400.]).with_icon(crate::window_icon());
        let mut asks = Asks::default();
        let mut close = false;
        ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("settings"), builder, |ui, _class| {
            close = ui.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape));
            egui::Panel::left("pages").resizable(false).exact_size(210.).frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(12)).show(ui, |ui| {
                ui.add_space(8.);
                for (i, (sign, name)) in PAGES.iter().enumerate() {
                    let label = ui::labeled(ui, *sign, &tr(name), 14.);
                    // as in the settings of Windows 11: the chosen page on a grey ground, with a short line of the accent colour
                    let chosen = page == i;
                    let grey = ui.visuals().widgets.hovered.weak_bg_fill;
                    let button = egui::Button::new(label).fill(if chosen { grey } else { egui::Color32::TRANSPARENT }).frame_when_inactive(chosen).min_size(Vec2::new(ui.available_width(), 36.));
                    let response = ui.add(button);
                    if chosen {
                        let bar = egui::Rect::from_center_size(response.rect.left_center() + Vec2::new(2., 0.), Vec2::new(3., 16.));
                        ui.painter().rect_filled(bar, 2, ui::accent(ui));
                    }
                    if response.clicked() {
                        asks.settings = Some(i);
                    }
                    ui.add_space(2.);
                }
            });
            egui::CentralPanel::default().frame(egui::Frame::new().fill(ui.visuals().panel_fill)).show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    egui::Frame::new().inner_margin(egui::Margin { left: 24, right: 24, top: 12, bottom: 24 }).show(ui, |ui| {
                        ui.set_max_width(ui.available_width().min(680.)); // wide windows: the rows do not stretch over the whole screen
                        ui.label(RichText::new(tr(PAGES[page].1)).size(24.).strong());
                        ui.add_space(4.);
                        match page {
                            GENERAL => self.page_general(ui, &mut asks),
                            EFFECTS => self.page_effects(ui, &mut asks),
                            ANIMATION => self.page_animation(ui, &mut asks),
                            _ => self.page_music(ui, &mut asks),
                        }
                    });
                });
            });
            self.confirm_reset_window(ui);
        });
        if close {
            self.settings = None;
            self.asking_reset = false;
        }
        self.answer(ctx, asks);
    }

    fn page_general(&mut self, ui: &mut Ui, asks: &mut Asks) {
        let cfg = &self.cfg;
        ui::heading(ui, &tr("Behavior"));
        ui::card(ui, |ui| {
            if let Some(on) = ui::switch_row(ui, &tr("Copy text only"), Some(&tr("Selecting and copying leave out block and line graphics")), cfg["copy_text_only"] == true, true, |_| {}) {
                asks.changes.push(("copy_text_only", json!(on)));
            }
            ui::divider(ui);
            if let Some(on) = ui::switch_row(ui, &tr("Remember recent files"), Some(&tr("Turning it off also clears the list")), cfg["remember_recent"] == true, true, |_| {}) {
                asks.changes.push(("remember_recent", json!(on)));
                if !on {
                    asks.changes.push(("recent", json!([])));
                }
            }
            ui::divider(ui);
            if let Some(key) = ui::combo_row(ui, &tr("Encoding"), None, true, cfg["encoding"].as_str().unwrap_or("auto"), &options(&ENCODINGS)) {
                asks.changes.push(("encoding", json!(key)));
            }
            ui::divider(ui);
            // Japanese and Chinese by their English names while their own script has no font loaded
            let cjk = matches!(i18n::current(), "ja" | "zh_CN");
            let mut languages = vec![("auto", tr("System default"))];
            languages.extend(i18n::LANGUAGES.iter().map(|(code, name, _)| (*code, match *code {
                "ja" if !cjk => "Japanese".to_string(),
                "zh_CN" if !cjk => "Chinese (Simplified)".to_string(),
                _ => name.to_string(),
            })));
            if let Some(code) = ui::combo_row(ui, &tr("Language"), None, true, cfg["language"].as_str().unwrap_or("auto"), &languages) {
                asks.changes.push(("language", json!(code)));
            }
        });
        ui::heading(ui, &tr("Appearance"));
        ui::card(ui, |ui| {
            if let Some(theme) = ui::combo_row(ui, &tr("Theme"), None, true, cfg["theme"].as_str().unwrap_or("light"), &options(&THEMES)) {
                asks.theme = Some(THEMES.iter().find(|t| t.0 == theme).unwrap().0);
            }
            ui::divider(ui);
            ui::row(ui, &tr("Custom colors"), Some(&tr("Picking a color switches to the Custom theme")), true, |_| {});
            for (key, name) in CUSTOM_COLORS {
                ui::row(ui, &format!("      {}", tr(name)), None, true, |ui| {
                    if let Some(hex) = colour(ui, &cfg["custom"][key]) {
                        let mut custom = cfg["custom"].clone();
                        custom[key] = json!(hex);
                        asks.changes.push(("custom", custom));
                        asks.theme = Some("custom");
                    }
                });
            }
            ui::row(ui, &format!("      {}", tr("Back to the default colors")), None, true, |ui| {
                if ui.button(tr("Reset")).clicked() {
                    asks.changes.push(("custom", core::defaults()["custom"].clone()));
                }
            });
            ui::divider(ui);
            // the DOS font is its own switch (Retro); here the font for when it is off
            let fonts: Vec<(&str, String)> = crate::installed_fonts().into_iter().map(|f| (f, f.to_string())).collect();
            let now = cfg["font"].as_str().filter(|f| fonts.iter().any(|o| o.0 == *f)).unwrap_or(core::FONT);
            if let Some(font) = ui::combo_row(ui, &tr("Font"), Some(&tr("Monospace fonts only")), cfg["font"] != core::DOS_FONT, now, &fonts) {
                asks.changes.push(("font", json!(font)));
            }
            ui::divider(ui);
            let mut zoom = cfg["zoom"].as_i64().unwrap_or(100);
            ui::row(ui, &tr("Default zoom"), Some(&tr("In percent; Ctrl+0 goes back to it")), true, |ui| {
                if ui.add(egui::DragValue::new(&mut zoom).range(30..=250).speed(1).suffix(" %")).changed() {
                    asks.changes.push(("zoom", json!(zoom / 10 * 10)));
                }
            });
        });
        ui.add_space(16.);
        ui::card(ui, |ui| {
            ui::row(ui, &tr("Reset settings"), Some(&tr("Back to the default values.")), true, |ui| {
                let red = egui::Button::new(RichText::new(tr("Reset")).color(egui::Color32::WHITE)).fill(egui::Color32::from_rgb(0xc4, 0x2b, 0x1c));
                if ui.add(red).clicked() {
                    self.asking_reset = true;
                }
            });
        });
    }

    /// The question before everything is reset.
    fn confirm_reset_window(&mut self, ui: &mut Ui) {
        if !self.asking_reset {
            return;
        }
        let ctx = ui.ctx().clone();
        let modal = egui::Modal::new(egui::Id::new("reset")).show(&ctx, |ui| {
            ui.set_width(420.);
            ui.label(RichText::new(tr("Reset all settings?")).size(18.).strong());
            ui.add_space(8.);
            ui.add(egui::Label::new(format!("{}\n\n{}", tr("The look, the language and everything else go back to their defaults."), tr("Are you really sure?"))).wrap());
            ui.add_space(12.);
            ui.horizontal(|ui| {
                if ui.button(tr("Cancel")).clicked() {
                    self.asking_reset = false;
                }
                let red = egui::Button::new(RichText::new(tr("Reset")).color(egui::Color32::WHITE)).fill(egui::Color32::from_rgb(0xc4, 0x2b, 0x1c));
                if ui.add(red).clicked() {
                    self.asking_reset = false;
                    self.reset(&ctx);
                }
            });
        });
        if modal.should_close() {
            self.asking_reset = false;
        }
    }

    fn page_effects(&mut self, ui: &mut Ui, asks: &mut Asks) {
        let cfg = &self.cfg;
        let (glow, halo) = (cfg["soft"] == true, self.glow_style() == "halo");
        let switch = |ui: &mut Ui, asks: &mut Asks, key: &'static str, title: &str, subtitle: &str, enabled: bool| {
            if let Some(on) = ui::switch_row(ui, &tr(title), Some(&tr(subtitle)), cfg[key] == true, enabled, |_| {}) {
                asks.changes.push((key, json!(on)));
            }
        };
        ui::heading(ui, &tr("Glow"));
        ui::card(ui, |ui| {
            switch(ui, asks, "soft", "Glow", "Light on the block graphics", true);
            ui::divider(ui);
            if let Some(mode) = ui::combo_row(ui, &tr("Style"), None, glow, cfg["glow_mode"].as_str().unwrap_or("halo"), &options(&[("soft", "Soft edges"), ("halo", "Neon")])) {
                asks.changes.push(("glow_mode", json!(mode)));
            }
            ui::divider(ui);
            let mut strength = cfg["glow"].as_i64().unwrap_or(3);
            ui::row(ui, &tr("Strength"), None, glow, |ui| {
                if ui.add(egui::Slider::new(&mut strength, 1..=5).show_value(false)).changed() {
                    asks.changes.push(("glow", json!(strength)));
                }
            });
            ui::divider(ui);
            switch(ui, asks, "text_glow", "Text glow", "Soft light around the text, like phosphor on an old monitor", true);
            ui::divider(ui);
            switch(ui, asks, "neon_flicker", "Flicker", "Flickers now and then, like an old sign; needs Neon", halo);
            ui::divider(ui);
            switch(ui, asks, "pulse", "Pulse", "The neon glow beats to the music, or breathes without it", halo);
            ui::divider(ui);
            if let Some(mode) = ui::combo_row(ui, &tr("Pulse rhythm"), None, halo && cfg["pulse"] == true, cfg["pulse_mode"].as_str().unwrap_or("beat"), &options(&[("beat", "To the music"), ("breathe", "Breathing")])) {
                asks.changes.push(("pulse_mode", json!(mode)));
            }
        });
        ui::heading(ui, &tr("Colors"));
        ui::card(ui, |ui| {
            switch(ui, asks, "ramp", "Color ramp", "Each block of art runs from the top to the bottom color", true);
            let ramp = cfg["ramp"] == true;
            for (i, name) in ["Top", "Middle", "Bottom"].iter().enumerate() {
                ui::row(ui, &format!("      {}", tr(name)), None, ramp, |ui| {
                    if let Some(hex) = colour(ui, &cfg["ramp_colors"][i]) {
                        let mut colors = cfg["ramp_colors"].clone();
                        colors[i] = json!(hex);
                        asks.changes.push(("ramp_colors", colors));
                    }
                });
            }
            ui::row(ui, &format!("      {}", tr("Back to the theme’s colors")), None, ramp, |ui| {
                if ui.button(tr("Reset")).clicked() {
                    let own = THEME_RAMPS.iter().find(|t| cfg["theme"] == t.0).map_or(core::defaults()["ramp_colors"].clone(), |t| json!(t.1));
                    asks.changes.push(("ramp_colors", own));
                }
            });
        });
        ui::heading(ui, &tr("Retro"));
        ui::card(ui, |ui| {
            switch(ui, asks, "crt", "CRT screen", "Scanlines and dark corners, like an old monitor", true);
            ui::divider(ui);
            let masks = [("lines", "Scanlines"), ("dots", "Shadow mask"), ("stripes", "Aperture grille")];
            if let Some(value) = ui::combo_row(ui, &tr("Pattern"), None, cfg["crt"] == true, cfg["crt_mask"].as_str().unwrap_or("lines"), &options(&masks)) {
                asks.changes.push(("crt_mask", json!(value)));
            }
            ui::divider(ui);
            switch(ui, asks, "crt_live", "Live CRT", "A roll bar, flicker, noise and round corners", cfg["crt"] == true);
            ui::divider(ui);
            switch(ui, asks, "chroma", "Color fringe", "Red and blue edges, like an old tube or a VHS tape", true);
            ui::divider(ui);
            if let Some(on) = ui::switch_row(ui, &tr("DOS font"), Some(&tr("The IBM VGA font, as NFOs looked under DOS")), self.dos_font(), true, |_| {}) {
                asks.font = Some(on);
            }
        });
        ui::heading(ui, &tr("Random look"));
        ui::card(ui, |ui| {
            switch(ui, asks, "random_on_start", "A new look at every start",
                "Each start shows a surprise look, only for this time; “Keep” in the effects sidebar makes it your look", true);
        });
    }

    fn page_animation(&mut self, ui: &mut Ui, asks: &mut Asks) {
        let cfg = &self.cfg;
        let switch = |ui: &mut Ui, asks: &mut Asks, key: &'static str, title: &str, subtitle: &str| {
            if let Some(on) = ui::switch_row(ui, &tr(title), Some(&tr(subtitle)), cfg[key] == true, true, |_| {}) {
                asks.changes.push((key, json!(on)));
            }
        };
        let combo = |ui: &mut Ui, asks: &mut Asks, key: &'static str, title: &str, parent: &str, list: &[(&'static str, &str)]| {
            if let Some(value) = ui::combo_row(ui, &tr(title), None, cfg[parent] == true, cfg[key].as_str().unwrap_or(""), &options(list)) {
                asks.changes.push((key, json!(value)));
            }
        };
        ui::heading(ui, &tr("Colors in motion"));
        ui.label(RichText::new(tr("Like the demos of the C64 days")).weak());
        ui.add_space(4.);
        ui::card(ui, |ui| {
            switch(ui, asks, "cycle", "Color cycling", "Colors move through the block graphics");
            ui::divider(ui);
            combo(ui, asks, "cycle_palette", "Cycling colors", "cycle", &[("c64", "C64 colors"), ("ramp", "Color ramp colors")]);
            ui::divider(ui);
            combo(ui, asks, "cycle_dir", "Cycling direction", "cycle", &[("down", "Down"), ("up", "Up")]);
        });
        ui::heading(ui, &tr("Logo and stars"));
        ui::card(ui, |ui| {
            switch(ui, asks, "wobble", "Wobble", "The logo sways in a wave; the text stays still");
            ui::divider(ui);
            combo(ui, asks, "wobble_amount", "Wobble strength", "wobble", &[("low", "Gentle"), ("mid", "Medium"), ("high", "Strong")]);
            ui::divider(ui);
            switch(ui, asks, "stars", "Starfield", "Stars behind the NFO");
            ui::divider(ui);
            combo(ui, asks, "stars_mode", "Stars", "stars", &[("drift", "Drifting"), ("warp", "Warp flight")]);
            ui::divider(ui);
            switch(ui, asks, "sheen", "Gleam", "A light sweeps across the art now and then");
            ui::divider(ui);
            switch(ui, asks, "sparkle", "Sparkle", "Single blocks of the art glint");
            ui::divider(ui);
            combo(ui, asks, "sparkle_amount", "How much sparkle", "sparkle", &[("few", "Few"), ("some", "Some"), ("many", "Many")]);
        });
        ui::heading(ui, &tr("Scroller"));
        ui::card(ui, |ui| {
            switch(ui, asks, "scroller", "Sine scroller", "A wavy running text along the bottom");
            ui::divider(ui);
            egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                ui.add_enabled_ui(cfg["scroller"] == true, |ui| {
                    ui.label(RichText::new(tr("Scroller text ({name} becomes the file name; empty: a greeting)")).small().weak());
                    let mut text = cfg["scroller_text"].as_str().unwrap_or("").to_string();
                    if ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY)).changed() {
                        asks.changes.push(("scroller_text", json!(text)));
                    }
                });
            });
        });
        ui::heading(ui, &tr("On opening"));
        ui::card(ui, |ui| self.opening_row(ui, asks, true));
    }

    fn page_music(&mut self, ui: &mut Ui, asks: &mut Asks) {
        let cfg = &self.cfg;
        let on = cfg["music"] == true;
        ui::heading(ui, &tr("Music"));
        ui.label(RichText::new(tr("Supported formats: MOD, XM, S3M, IT, OGG, MP3, FLAC, WAV")).weak());
        ui.add_space(4.);
        ui::card(ui, |ui| {
            if let Some(value) = ui::switch_row(ui, &tr("Music"), Some(&tr("Plays while NfoGlow is open")), on, true, |_| {}) {
                asks.changes.push(("music", json!(value)));
            }
            ui::divider(ui);
            let dir = cfg["music_dir"].as_str().unwrap_or("");
            let shown = if dir.is_empty() { tr("None: the built-in chiptunes by Stennish") } else { dir.to_string() };
            ui::row(ui, &tr("Music folder"), Some(&shown), on, |ui| {
                if !dir.is_empty() && ui::icon_button(ui, ui::CLEAR, &tr("Back to the built-in chiptunes"), false).clicked() {
                    asks.changes.push(("music_dir", json!("")));
                }
                if ui::icon_button(ui, ui::FOLDER, &tr("Music folder"), false).clicked() {
                    asks.folder = true;
                }
            });
            ui::divider(ui);
            let order = if cfg["music_shuffle"] == true { "random" } else { "order" };
            if let Some(o) = ui::combo_row(ui, &tr("Order"), None, on, order, &options(&[("random", "Random"), ("order", "In order")])) {
                asks.changes.push(("music_shuffle", json!(o == "random")));
            }
            ui::divider(ui);
            let mut volume = cfg["volume"].as_i64().unwrap_or(60);
            ui::row(ui, &tr("Volume"), None, on, |ui| {
                if ui.add(egui::Slider::new(&mut volume, 0..=100).show_value(false)).changed() {
                    asks.changes.push(("volume", json!(volume)));
                }
            });
            ui::divider(ui);
            if let Some(value) = ui::switch_row(ui, &tr("Show the track name"), Some(&tr("A short note at the bottom when a new track starts")), cfg["music_info"] == true, on, |_| {}) {
                asks.changes.push(("music_info", json!(value)));
            }
            ui::divider(ui);
            if let Some(value) = ui::switch_row(ui, &tr("Fade in and out"), Some(&tr("The music starts quietly, and every track ends softly")), cfg["music_fade"] == true, on, |_| {}) {
                asks.changes.push(("music_fade", json!(value)));
            }
        });
    }

    /// Everything back to the defaults, the list of recent files emptied too, the theme as Windows is set (as NfoGlow
    /// for Linux does).
    pub(crate) fn reset(&mut self, ctx: &egui::Context) {
        self.cfg = core::defaults();
        self.looks_before.clear();
        i18n::set_language("auto");
        crate::set_fonts(ctx, self.cfg["font"].as_str().unwrap_or(core::FONT));
        self.use_system_theme(ctx); // saves the settings
        self.apply_music(ctx);
        self.zoom = 100;
    }
}
