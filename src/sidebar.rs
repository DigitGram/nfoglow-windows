// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! The effects sidebar beside the NFO, as in NfoGlow for Linux: the themes as colour swatches, the styles, the effects
//! in sections that fold (one open at a time, under each title what in it is on), and at the bottom, always in view,
//! the random look and the way to the settings.
use crate::i18n::tr;
use crate::ui::{self, Icon};
use crate::view::rgb;
use crate::{core, styles, App, OPENINGS, THEMES};
use eframe::egui::{self, Align, Layout, RichText, Ui};
use serde_json::{json, Value};

/// The pages of the settings window.
pub const GENERAL: usize = 0;
pub const EFFECTS: usize = 1;
pub const ANIMATION: usize = 2;
pub const MUSIC: usize = 3;

/// The switches that have more settings: the page of the settings where they are and the first of those settings (a gear
/// beside the switch opens the page there and lights the row up, as in NfoGlow for Linux).
pub fn more(key: &str) -> Option<(usize, &'static str)> {
    Some(match key {
        "ramp" => (EFFECTS, "ramp"),
        "crt" => (EFFECTS, "crt_mask"),
        "pulse" => (EFFECTS, "pulse_mode"),
        "smooth" => (EFFECTS, "smooth_amount"),
        "cycle" => (ANIMATION, "cycle_palette"),
        "wobble" => (ANIMATION, "wobble_amount"),
        "stars" => (ANIMATION, "stars_mode"),
        "scroller" => (ANIMATION, "scroller_text"),
        "sparkle" => (ANIMATION, "sparkle_amount"),
        "music" => (MUSIC, "music_dir"),
        _ => return None,
    })
}

pub const GLOW_STYLES: [(&str, &str); 3] = [("off", "Off"), ("soft", "Soft"), ("halo", "Neon")];

/// What is asked of the app by a click in the sidebar or the settings.
#[derive(Default)]
pub struct Asks {
    pub changes: Vec<(&'static str, Value)>,
    pub theme: Option<&'static str>,
    pub look: Option<core::Cfg>,
    pub settings: Option<usize>,
    pub show: Option<&'static str>, // with it: the setting to light up there
    pub roll: bool,
    pub replay: bool,
    pub undo: bool,
    pub font: Option<bool>, // the DOS font on or off
    pub folder: bool,       // pick a music folder
    pub keep: bool,         // the look on trial becomes the settings
    pub back: bool,         // back to the look in the settings
}

impl App {
    /// "off", "soft" or "halo": the glow as one choice.
    pub(crate) fn glow_style(&self) -> &'static str {
        match (self.cfg["soft"] == true, self.cfg["glow_mode"] == "halo") {
            (false, _) => "off",
            (true, false) => "soft",
            (true, true) => "halo",
        }
    }

    /// The glow as one choice, back into its two settings.
    pub(crate) fn glow_look(style: &str) -> core::Cfg {
        let mut look = core::Cfg::new();
        look.insert("soft".into(), json!(style != "off"));
        if style != "off" {
            look.insert("glow_mode".into(), json!(style));
        }
        look
    }

    /// Carry out what the sidebar or the settings asked for.
    pub(crate) fn answer(&mut self, ctx: &egui::Context, asks: Asks) {
        for (key, value) in asks.changes {
            self.change(ctx, key, value);
        }
        if let Some(theme) = asks.theme {
            self.set_theme(ctx, theme);
        }
        if let Some(look) = asks.look {
            self.change_all(ctx, &look);
        }
        if let Some(on) = asks.font {
            self.change(ctx, "font", json!(if on { core::DOS_FONT } else { core::FONT }));
        }
        if asks.settings.is_some() {
            self.settings = asks.settings;
            self.flash = asks.show.map(|key| (key, std::time::Instant::now()));
        }
        if asks.roll {
            self.roll_look(ctx);
        }
        if asks.replay {
            self.start_opening(ctx);
        }
        if asks.undo {
            self.undo(ctx);
        }
        if asks.keep {
            self.keep_look();
        }
        if asks.back {
            self.back_look(ctx);
        }
        if asks.folder {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                self.change(ctx, "music_dir", json!(dir.to_string_lossy()));
            }
        }
    }

    /// A switch row of the sidebar, with a gear to its settings while it is on.
    fn effect(&self, ui: &mut Ui, asks: &mut Asks, key: &'static str, title: &str, subtitle: Option<&str>, enabled: bool) {
        let on = self.cfg[key] == true;
        let gear = |ui: &mut Ui| {
            if let Some((page, show)) = more(key).filter(|_| on && enabled) {
                if ui::icon_button(ui, ui::SETTINGS, &tr("More settings"), false).clicked() {
                    asks.settings = Some(page);
                    asks.show = Some(show);
                }
            }
        };
        let mut picked = None;
        if let Some(value) = ui::switch_row(ui, &tr(title), subtitle.map(tr).as_deref(), on, enabled, gear) {
            picked = Some(value);
        }
        if let Some(value) = picked {
            asks.changes.push((key, json!(value)));
        }
    }

    pub(crate) fn sidebar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let mut asks = Asks::default();
        // the footer stays at the bottom, in view, while the effects above it scroll
        let fill = ui.visuals().panel_fill;
        egui::Panel::bottom("sidebar-footer").frame(egui::Frame::new().fill(fill).inner_margin(12)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let width = ui.available_width() - 36. - ui.spacing().item_spacing.x;
                let label = ui::labeled(ui, ui::SHUFFLE, &tr("Random look"), 14.);
                if ui.add_sized([width, 32.], egui::Button::new(label)).on_hover_text(tr("Roll a new look (not saved yet)")).clicked() {
                    asks.roll = true;
                }
                if ui::icon_button(ui, ui::SETTINGS, &tr("Colors and more…"), false).clicked() {
                    asks.settings = Some(GENERAL);
                }
            });
            // only while the look differs from the saved one (a roll, or any change here): keep it, or go back
            if self.trial() {
                ui.add_space(10.);
                ui::card(ui, |ui| {
                    egui::Frame::new().inner_margin(10).show(ui, |ui| {
                        ui.vertical_centered(|ui| ui.add(egui::Label::new(tr("This look is not saved yet.")).wrap()));
                        ui.add_space(8.);
                        ui.horizontal(|ui| {
                            let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.;
                            let keep = egui::Button::new(RichText::new(tr("Keep")).color(if ui.visuals().dark_mode { egui::Color32::BLACK } else { egui::Color32::WHITE })).fill(ui::accent(ui));
                            if ui.add_sized([width, 30.], keep).on_hover_text(tr("Save this look as your settings")).clicked() {
                                asks.keep = true;
                            }
                            if ui.add_sized([width, 30.], egui::Button::new(tr("Back"))).on_hover_text(tr("Back to the look in your settings")).clicked() {
                                asks.back = true;
                            }
                        });
                    });
                });
            } else {
                self.saved_look = None; // nothing differs any more: the next change starts a new trial
            }
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(fill)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 14, top: 4, bottom: 12 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    self.sidebar_body(ui, &mut asks);
                });
            });
        });
        self.answer(&ctx, asks);
    }

    fn sidebar_body(&mut self, ui: &mut Ui, asks: &mut Asks) {
        let cfg = &self.cfg;
        // the themes as round swatches, all eight in one row: the colours say more than the names
        ui::heading(ui, &tr("Theme"));
        let size = ((ui.available_width() / THEMES.len() as f32) - 8.).clamp(18., 28.);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = (ui.available_width() - (size + 6.) * THEMES.len() as f32) / (THEMES.len() - 1) as f32;
            for (key, name) in THEMES {
                let mut look = cfg.clone();
                look.insert("theme".into(), json!(key));
                let t = core::theme_colors(&look);
                if ui::swatch(ui, size, rgb(t.bg), rgb(t.art), rgb(t.text), cfg["theme"] == key, &tr(name)).clicked() {
                    asks.theme = Some(key);
                }
            }
        });
        let now = THEMES.iter().find(|t| cfg["theme"] == t.0).map_or("", |t| t.1);
        ui.vertical_centered(|ui| ui.label(RichText::new(tr(now)).weak()));

        ui::heading(ui, &tr("Style"));
        let options: Vec<_> = styles().into_iter().map(|(name, look)| {
            // its button lights up while every effect matches it
            let on = look.as_object().unwrap().iter().all(|(k, v)| cfg[k] == *v);
            (name, tr(name), on)
        }).collect();
        if let Some(name) = ui::choice_grid(ui, 3, &options) {
            asks.look = styles().into_iter().find(|s| s.0 == name).and_then(|s| s.1.as_object().cloned());
        }

        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Effects and sound")).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // the undo of the whole look sits with the effects it mostly undoes
                ui.add_enabled_ui(!self.looks_before.is_empty(), |ui| {
                    if ui::icon_button(ui, ui::UNDO, &tr("Undo: back to the look before your last change"), false).clicked() {
                        asks.undo = true;
                    }
                });
            });
        });
        ui.add_space(4.);
        let names = |keys: &[(&str, &str)]| -> Vec<String> { keys.iter().filter(|k| cfg[k.0] == true).map(|k| tr(k.1)).collect() };
        let glow = self.glow_style();
        let halo = glow == "halo";
        let opening = OPENINGS.iter().find(|o| cfg["opening"] == o.0).unwrap_or(&OPENINGS[0]);
        // Animation leads with the opening effect (nothing when "None"), then the switches that are on
        let animation = [if opening.0 == "off" { vec![] } else { vec![tr(opening.1)] },
            names(&[("cycle", "Color cycling"), ("wobble", "Wobble"), ("stars", "Starfield"), ("scroller", "Sine scroller"),
                ("sheen", "Gleam"), ("sparkle", "Sparkle")])].concat();
        let sections: [(Icon, &str, Vec<String>); 4] = [
            (ui::GLOW, "Glow", [vec![tr(GLOW_STYLES.iter().find(|g| g.0 == glow).unwrap().1)],
                names(&[("ramp", "Color ramp"), ("text_glow", "Text glow"), ("neon_flicker", "Flicker"), ("pulse", "Pulse")])].concat()),
            (ui::PLAY, "Animation", animation),
            (ui::RETRO, "Retro", [names(&[("crt", "CRT screen"), ("crt_live", "Live CRT"), ("chroma", "Color fringe")]), if self.dos_font() { vec![tr("DOS font")] } else { vec![] }].concat()),
            (ui::MORE, "Other", names(&[("smooth", "Smooth edges"), ("music", "Music")])),
        ];
        ui::card(ui, |ui| {
            for (i, (sign, title, on)) in sections.iter().enumerate() {
                if i > 0 {
                    ui::divider(ui);
                }
                let open = self.section == Some(i);
                let summary = if on.is_empty() { tr("Off") } else { on.join(", ") };
                if ui::section_head(ui, *sign, &tr(title), &summary, open) {
                    self.section = if open { None } else { Some(i) }; // one open at a time
                }
                if !open {
                    continue;
                }
                ui::divider(ui);
                match i {
                    0 => {
                        egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                            let options: Vec<_> = GLOW_STYLES.iter().map(|(k, n)| (*k, tr(n), glow == *k)).collect();
                            if let Some(style) = ui::choice_grid(ui, 3, &options) {
                                asks.look = Some(Self::glow_look(style));
                            }
                        });
                        let mut strength = self.cfg["glow"].as_i64().unwrap_or(3);
                        ui::row(ui, &tr("Strength"), None, glow != "off", |ui| {
                            if ui.add(egui::Slider::new(&mut strength, 1..=5).show_value(false)).changed() {
                                asks.changes.push(("glow", json!(strength)));
                            }
                        });
                        self.effect(ui, asks, "ramp", "Color ramp", None, true);
                        self.effect(ui, asks, "text_glow", "Text glow", None, true);
                        self.effect(ui, asks, "neon_flicker", "Flicker", Some("Flickers now and then, like an old sign; needs Neon"), halo);
                        self.effect(ui, asks, "pulse", "Pulse", Some("Neon glow to the beat; needs Neon"), halo);
                    }
                    1 => {
                        self.opening_row(ui, asks, false);
                        self.effect(ui, asks, "cycle", "Color cycling", None, true);
                        self.effect(ui, asks, "wobble", "Wobble", None, true);
                        self.effect(ui, asks, "stars", "Starfield", None, true);
                        self.effect(ui, asks, "scroller", "Sine scroller", None, true);
                        self.effect(ui, asks, "sheen", "Gleam", Some("A light sweeps across the art now and then"), true);
                        self.effect(ui, asks, "sparkle", "Sparkle", Some("Single blocks of the art glint"), true);
                    }
                    2 => {
                        self.effect(ui, asks, "crt", "CRT screen", None, true);
                        self.effect(ui, asks, "crt_live", "Live CRT", Some("A roll bar, flicker, noise and round corners"), self.cfg["crt"] == true);
                        self.effect(ui, asks, "chroma", "Color fringe", Some("Red and blue edges, like an old tube or a VHS tape"), true);
                        let dos = self.dos_font();
                        if let Some(on) = ui::switch_row(ui, &tr("DOS font"), Some(&tr("The IBM VGA font, as NFOs looked under DOS")), dos, true, |_| {}) {
                            asks.font = Some(on);
                        }
                    }
                    _ => {
                        self.effect(ui, asks, "smooth", "Smooth edges", Some("Rounds the pixel steps of the block art, with anti-aliasing"), true);
                        self.effect(ui, asks, "music", "Music", None, true);
                    }
                }
            }
        });
    }

    /// The choice of the effect on opening and a button to play it now. describe: its description under the title (in
    /// the narrow sidebar the choice says enough).
    pub(crate) fn opening_row(&self, ui: &mut Ui, asks: &mut Asks, describe: bool) {
        let now = OPENINGS.iter().find(|o| self.cfg["opening"] == o.0).unwrap_or(&OPENINGS[0]);
        let options: Vec<_> = OPENINGS.iter().map(|o| (o.0, tr(o.1))).collect();
        let info = tr(now.2);
        let title = tr("Opening effect");
        let play = now.0 != "off" && self.path.is_some() && !self.text_only;
        let picked = ui::row(ui, &title, describe.then_some(info.as_str()), true, |ui| {
            if ui.add_enabled_ui(play, |ui| ui::icon_button(ui, ui::PLAY, &tr("Play the effect now"), false)).inner.clicked() {
                asks.replay = true;
            }
            let mut picked = None;
            egui::ComboBox::from_id_salt("opening").selected_text(tr(now.1)).width(if describe { 170. } else { 130. }).show_ui(ui, |ui| {
                for (key, name) in &options {
                    if ui.selectable_label(*key == now.0, name).clicked() {
                        picked = Some(*key);
                    }
                }
            });
            picked
        });
        if let Some(key) = picked {
            asks.changes.push(("opening", json!(key)));
        }
    }
}
