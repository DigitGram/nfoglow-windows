// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 DigitGram
//! The building blocks of the window in the look of Windows 11, laid out like the GNOME version: switches, rows with
//! a title and a grey line under it and their control at the right, cards of rows, sections that fold, the colour
//! swatches of the themes, buttons in a grid, and icons.
use eframe::egui::{self, Align, Color32, CornerRadius, FontFamily, FontId, Layout, Rect, Response, RichText, Sense, Stroke, Ui, Vec2};

/// An icon: its sign in the icon font of Windows (Segoe Fluent Icons on Windows 11, Segoe MDL2 Assets on Windows 10:
/// the same code points), and a sign of the normal fonts when that font is missing (developing on Linux).
#[derive(Clone, Copy)]
pub struct Icon(pub char, pub char);

pub const OPEN: Icon = Icon('\u{E8E5}', '↥');
pub const RECENT: Icon = Icon('\u{E81C}', '⟲');
pub const SEARCH: Icon = Icon('\u{E721}', '◎');
pub const EFFECTS: Icon = Icon('\u{E790}', '◐');
pub const MENU: Icon = Icon('\u{E700}', '☰');
pub const SETTINGS: Icon = Icon('\u{E713}', '⚙');
pub const UNDO: Icon = Icon('\u{E7A7}', '↶');
pub const SHUFFLE: Icon = Icon('\u{E8B1}', '⇄');
pub const PLAY: Icon = Icon('\u{E768}', '▶');
pub const GLOW: Icon = Icon('\u{E706}', '☀');
pub const EDGES: Icon = Icon('\u{E7A8}', '◇');
pub const RETRO: Icon = Icon('\u{E7F4}', '▣');
pub const SOUND: Icon = Icon('\u{E8D6}', '♪');
pub const DOWN: Icon = Icon('\u{E70D}', '⌄');
pub const UP: Icon = Icon('\u{E70E}', '⌃');
pub const FOLDER: Icon = Icon('\u{E8B7}', '▤');
pub const CLEAR: Icon = Icon('\u{E894}', '⌫');
pub const ZOOM_IN: Icon = Icon('\u{E8A3}', '+');
pub const ZOOM_OUT: Icon = Icon('\u{E71F}', '−');
pub const RIGHT: Icon = Icon('\u{E76C}', '›');
pub const BACK: Icon = Icon('\u{E72B}', '‹');
pub const LINK: Icon = Icon('\u{E8A7}', '↗');
pub const CLOSE: Icon = Icon('\u{E711}', '✕');

pub fn icon_family() -> FontFamily {
    FontFamily::Name("icons".into())
}

/// The icon as text of the given size.
pub fn icon(ui: &Ui, icon: Icon, size: f32) -> RichText {
    let font = ui.fonts(|f| f.families().contains(&icon_family()));
    if font {
        RichText::new(icon.0).font(FontId::new(size, icon_family()))
    } else {
        RichText::new(icon.1).font(FontId::proportional(size))
    }
}

/// An icon and a text in one line, each in its own font (the icon's sign is not in the text font).
pub fn labeled(ui: &Ui, sign: Icon, text: &str, size: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for part in [icon(ui, sign, size), RichText::new(format!("   {text}")).size(size)] {
        part.append_to(&mut job, ui.style(), egui::FontSelection::Default, Align::Center);
    }
    job
}

/// A flat button with only an icon (and its explanation when the mouse rests on it).
pub fn icon_button(ui: &mut Ui, sign: Icon, hint: &str, on: bool) -> Response {
    let text = icon(ui, sign, 16.);
    ui.add(egui::Button::selectable(on, text).min_size(Vec2::new(36., 32.)).frame(on).frame_when_inactive(on)).on_hover_text(hint)
}

/// The accent colour of the window (the blue of Windows 11).
pub fn accent(ui: &Ui) -> Color32 {
    ui.visuals().selection.bg_fill
}

/// The switch of Windows 11: a pill with a knob, filled in the accent colour while it is on.
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let size = Vec2::new(40., 20.);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, ""));
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_with_time(response.id, *on, 0.12);
        let v = ui.visuals();
        let (fill, stroke, knob) = if *on {
            (accent(ui), Stroke::NONE, if v.dark_mode { Color32::BLACK } else { Color32::WHITE })
        } else {
            (Color32::TRANSPARENT, Stroke::new(1., v.weak_text_color()), v.text_color())
        };
        let painter = ui.painter();
        painter.rect(rect, CornerRadius::same(10), fill, stroke, egui::StrokeKind::Inside);
        let grow = if response.hovered() { 1. } else { 0. };
        let x = egui::lerp(rect.left() + 10.0..=rect.right() - 10., t);
        painter.circle_filled(egui::pos2(x, rect.center().y), 5. + grow + t, knob);
    }
    response
}

/// A card: rows on a lighter ground with rounded corners, as the settings of Windows 11 show them.
pub fn card<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    let v = ui.visuals();
    let frame = egui::Frame::new().fill(v.window_fill).stroke(Stroke::new(1., v.widgets.noninteractive.bg_stroke.color)).corner_radius(6);
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.;
        body(ui)
    })
    .inner
}

/// A thin line between two rows of a card.
pub fn divider(ui: &mut Ui) {
    let rect = ui.available_rect_before_wrap();
    let y = rect.top();
    ui.painter().hline(rect.x_range(), y, Stroke::new(1., ui.visuals().widgets.noninteractive.bg_stroke.color));
    ui.add_space(1.);
}

/// A heading over a card.
pub fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(10.);
    ui.label(RichText::new(text).strong());
    ui.add_space(4.);
}

/// A row: its title and a grey line under it at the left, its control at the right. enabled: grey and unusable
/// while the switch it belongs to is off.
pub fn row<R>(ui: &mut Ui, title: &str, subtitle: Option<&str>, enabled: bool, control: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(28.);
        ui.add_enabled_ui(enabled, |ui| {
            // a row of its own: its height is that of its content, so the control sits beside the title, not below
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let r = control(ui);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 2.;
                        ui.add(egui::Label::new(RichText::new(title).color(ui.visuals().strong_text_color())).wrap());
                        if let Some(sub) = subtitle {
                            ui.add(egui::Label::new(RichText::new(sub).small().weak()).wrap());
                        }
                    });
                    r
                })
                .inner
            })
            .inner
        })
        .inner
    })
    .inner
}

/// A row with a switch. Returns the new value when it was switched.
pub fn switch_row(ui: &mut Ui, title: &str, subtitle: Option<&str>, on: bool, enabled: bool, extra: impl FnOnce(&mut Ui)) -> Option<bool> {
    let mut value = on;
    let changed = row(ui, title, subtitle, enabled, |ui| {
        let changed = switch(ui, &mut value).changed();
        extra(ui); // e.g. a gear to its settings
        changed
    });
    changed.then_some(value)
}

/// A row with a list to pick from. Returns the key picked.
pub fn combo_row<'a>(ui: &mut Ui, title: &str, subtitle: Option<&str>, enabled: bool, now: &str, options: &[(&'a str, String)]) -> Option<&'a str> {
    let shown = options.iter().find(|o| o.0 == now).map_or(String::new(), |o| o.1.clone());
    row(ui, title, subtitle, enabled, |ui| {
        let mut picked = None;
        egui::ComboBox::from_id_salt(title).selected_text(shown).width(150.).show_ui(ui, |ui| {
            for (key, name) in options {
                if ui.selectable_label(*key == now, name).clicked() {
                    picked = Some(*key);
                }
            }
        });
        picked
    })
}

/// A section of the effects that folds: its icon, title and what in it is on; the rows when it is open.
/// Returns true when its title was clicked.
pub fn section_head(ui: &mut Ui, sign: Icon, title: &str, summary: &str, open: bool) -> bool {
    let under = ui.painter().add(egui::Shape::Noop); // the hover light goes under the text, known only after it
    let response = egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(icon(ui, sign, 16.));
            ui.add_space(4.);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(icon(ui, if open { UP } else { DOWN }, 12.).weak());
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.;
                    ui.add(egui::Label::new(RichText::new(title).color(ui.visuals().strong_text_color())).truncate());
                    ui.add(egui::Label::new(RichText::new(summary).small().weak()).truncate());
                });
            });
        });
    });
    let response = ui.interact(response.response.rect, ui.id().with(title), Sense::click());
    if response.hovered() {
        ui.painter().set(under, egui::epaint::RectShape::filled(response.rect, 6, ui.visuals().widgets.hovered.weak_bg_fill));
    }
    response.clicked()
}

/// A row that leads somewhere: its title, and at the right a sign of where (a page, a web link). The whole row is
/// the button.
pub fn nav_row(ui: &mut Ui, title: &str, sign: Icon) -> Response {
    let under = ui.painter().add(egui::Shape::Noop); // the hover light goes under the text
    let inner = egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).color(ui.visuals().strong_text_color()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(icon(ui, sign, 12.).weak()));
        });
    });
    let response = ui.interact(inner.response.rect, ui.id().with(title), Sense::click());
    if response.hovered() {
        ui.painter().set(under, egui::epaint::RectShape::filled(response.rect, 6, ui.visuals().widgets.hovered.weak_bg_fill));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// Buttons in a grid of equal width: one of them is chosen. Returns the key clicked.
pub fn choice_grid<'a>(ui: &mut Ui, columns: usize, options: &[(&'a str, String, bool)]) -> Option<&'a str> {
    let gap = 6.;
    let width = ((ui.available_width() - gap * (columns - 1) as f32) / columns as f32).floor();
    let mut picked = None;
    for chunk in options.chunks(columns) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.spacing_mut().button_padding.x = 4.; // a long name ("Demoszene") still fits a third of the sidebar
            for (key, name, on) in chunk {
                let button = egui::Button::selectable(*on, egui::Atom::from(name.as_str())).frame_when_inactive(true).wrap_mode(egui::TextWrapMode::Truncate).min_size(Vec2::new(width, 32.));
                if ui.add_sized([width, 32.], button).clicked() {
                    picked = Some(*key);
                }
            }
        });
        ui.add_space(gap);
    }
    picked
}

/// A round colour swatch of a theme: its background, a little block of art and two lines of text.
pub fn swatch(ui: &mut Ui, size: f32, bg: Color32, art: Color32, text: Color32, chosen: bool, name: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size + 6.), Sense::click());
    let response = response.on_hover_text(name);
    let painter = ui.painter();
    let c = rect.center();
    let r = size / 2.;
    if chosen || response.hovered() {
        let ring = if chosen { accent(ui) } else { ui.visuals().weak_text_color() };
        painter.circle_stroke(c, r + 2.5, Stroke::new(2., ring));
    }
    painter.circle(c, r - 0.5, bg, Stroke::new(1., Color32::from_gray(128).gamma_multiply(0.55)));
    let at = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(c + Vec2::new((x - 0.5) * size, (y - 0.5) * size), Vec2::new(w * size, h * size));
    painter.rect_filled(at(0.26, 0.3, 0.2, 0.4), 0, art);
    for y in [0.36, 0.56] {
        painter.rect_filled(at(0.52, y, 0.24, 0.08), 0, text);
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, chosen, name));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(run: impl FnMut(&mut Ui)) -> egui::Context {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(Default::default(), run);
        ctx
    }

    #[test]
    fn a_switch_turns_when_clicked() {
        // two frames: the press and the release over the switch
        let ctx = egui::Context::default();
        let mut on = false;
        let mut rect = Rect::NOTHING;
        let _ = ctx.run_ui(Default::default(), |ui| rect = switch(ui, &mut on).rect);
        let at = rect.center();
        for pressed in [true, false] {
            let input = egui::RawInput {
                events: vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }],
                ..Default::default()
            };
            let _ = ctx.run_ui(input, |ui| {
                switch(ui, &mut on);
            });
        }
        assert!(on);
    }

    #[test]
    fn rows_and_grids_fit_their_width() {
        let mut widths = Vec::new();
        frame(|ui| {
            ui.set_width(300.);
            let picked = choice_grid(ui, 3, &[("a", "A".into(), true), ("b", "B".into(), false), ("c", "C".into(), false), ("d", "D".into(), false)]);
            assert_eq!(picked, None);
            widths.push(ui.min_rect().width());
            card(ui, |ui| {
                switch_row(ui, "A title so long that it has to wrap over two lines in a narrow sidebar", Some("and a grey line"), true, true, |_| {});
            });
            widths.push(ui.min_rect().width());
        });
        assert!(widths.iter().all(|w| *w <= 300.5), "{widths:?}");
    }
}
