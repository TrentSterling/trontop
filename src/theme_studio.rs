use crate::{
    theme::{self, ThemeSettings, Tokens},
    widgets,
};
use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};

pub const LIBRARY_KEY: &str = "trontop.theme-library.v1";
const MAX_SAVED: usize = 12;

#[derive(Clone)]
struct Saved {
    name: String,
    theme: ThemeSettings,
}

#[derive(Default)]
pub struct Studio {
    revision: u64,
    pub(crate) tab: usize,
    selected: usize,
    baseline: Option<ThemeSettings>,
    hex: [String; 6],
    colors: Option<[[u8; 3]; 6]>,
    transfer: String,
    name: String,
    saved: Vec<Saved>,
    notice: Option<(String, bool)>,
    flavor: theme::magic::Flavor,
    rolls: Vec<theme::magic::Palette>,
    roll_sequence: u64,
}

/// The one-sentence Theme Studio intro.
const HERO: &str = "Your palette, live across Trontop.";

impl Studio {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        settings: &mut ThemeSettings,
        open: &mut bool,
        editable: bool,
    ) {
        if !*open {
            self.baseline = None;
            return;
        }
        if editable {
            self.baseline.get_or_insert(*settings);
        } else {
            // Opening the editor before async load must not make the temporary
            // defaults the later Revert session target.
            self.baseline = None;
        }
        let before = *settings;
        self.sync_hex(*settings);
        let t = theme::tokens(*settings);
        let mut close = false;
        let width = 568.0_f32.min(ctx.content_rect().width() - 48.0);
        egui::Window::new("Trontop Theme Studio")
            .title_bar(false)
            .frame(egui::Frame::window(&ctx.global_style()).inner_margin(0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(width)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(width);
                if widgets::dialog_header(ui, "Theme Studio", t) {
                    close = true;
                }
                widgets::dialog_body(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 7.0;
                widgets::hover_label(ui, RichText::new(HERO).size(13.0).color(t.text))
                    .on_hover_text("Four color pegs make the background gradient; the two accent colors are independent. Changes apply live.");
                ui.add_enabled_ui(editable, |ui| {
                  ui.horizontal(|ui| {
                    if widgets::action_button(ui, "Randomize", Vec2::new(112.0, 28.0), t.accent_dim, t)
                        .on_hover_text("ColorMagic: four related pegs and matching accents. Keeps light/dark mode, spacing, gradient intensity and layout.").clicked() {
                        self.randomize(settings);
                    }
                    egui::ComboBox::from_id_salt("magic_flavor").width(110.0).selected_text(self.flavor.label()).show_ui(ui, |ui| {
                        for flavor in theme::magic::Flavor::ALL { ui.selectable_value(&mut self.flavor, flavor, flavor.label()); }
                    });
                    if ui.add_enabled(!self.rolls.is_empty(), egui::Button::new("Undo roll")).on_hover_text("Restore the previous palette. Later layout edits are preserved. Remembers 12 rolls.").clicked() {
                        self.undo_roll(settings);
                    }
                  });
                  ui.horizontal(|ui| {
                    // Fixed tab slots: the selected pill never shifts its neighbors.
                    for (i, name) in ["Palette", "Appearance", "Presets", "My themes"].iter().enumerate() {
                        if widgets::stable_tab(ui, self.tab == i, name).clicked() {
                            self.tab = i;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::stable_tab(ui, !settings.dark, "Light").clicked() {
                            settings.dark = false;
                        }
                        if widgets::stable_tab(ui, settings.dark, "Dark").clicked() {
                            settings.dark = true;
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt(("theme_body", self.tab))
                    .max_height((ctx.content_rect().height() - 290.0).clamp(160.0, 471.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.tab {
                        0 => self.palette(ui, settings, t),
                        1 => self.appearance(ui, settings, t),
                        2 => self.presets(ui, settings, t),
                        _ => self.library(ui, settings, t),
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.add_enabled(Some(*settings) != self.baseline, egui::Button::new("Revert session")).on_hover_text("Restore the theme from when this editor opened. Changes apply live, so use this before closing to undo them.").clicked() {
                        *settings = self.baseline.unwrap_or_default();
                        self.notice = None;
                    }
                    if ui.button("Reset to TrontStack").clicked() { *settings = ThemeSettings::default(); }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::action_button(ui, "Done", Vec2::new(74.0, 28.0), t.accent_dim, t).clicked() { close = true; }
                    });
                });
                });
                if !editable {
                    widgets::hover_label(ui, RichText::new("Loading saved settings. Theme editing waits so your saved palettes cannot be overwritten.").size(11.0).color(t.text_muted));
                }
                });
            });
        if close {
            *open = false;
        }
        if !*open {
            self.baseline = None;
        }
        *settings = settings.normalized();
        if *settings != before {
            theme::install(ctx, *settings);
            ctx.request_repaint();
        }
    }

    fn sync_hex(&mut self, s: ThemeSettings) {
        let colors = [
            s.stops[0].color,
            s.stops[1].color,
            s.stops[2].color,
            s.stops[3].color,
            s.accent,
            s.secondary,
        ];
        for (i, &color) in colors.iter().enumerate() {
            if self.colors.is_none_or(|old| old[i] != color) {
                self.hex[i] = hex(color);
            }
        }
        self.colors = Some(colors);
    }

    fn randomize(&mut self, settings: &mut ThemeSettings) {
        self.roll_sequence = self.roll_sequence.wrapping_add(1);
        let clock = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        let seed = clock ^ self.roll_sequence.wrapping_mul(0x9e3779b97f4a7c15);
        if self.rolls.len() == MAX_SAVED {
            self.rolls.remove(0);
        }
        self.rolls.push(theme::magic::Palette::capture(settings));
        theme::magic::randomize(settings, seed, self.flavor);
        self.sync_hex(*settings);
    }

    fn undo_roll(&mut self, settings: &mut ThemeSettings) {
        if let Some(palette) = self.rolls.pop() {
            palette.apply(settings);
        }
        self.sync_hex(*settings);
    }

    fn palette(&mut self, ui: &mut egui::Ui, s: &mut ThemeSettings, t: Tokens) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut s.gradient_enabled, "Background gradient");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Even spacing").clicked() {
                    s.evenly_space();
                }
                if ui.button("Reverse").clicked() {
                    s.reverse_gradient();
                    self.selected = 3 - self.selected;
                }
            });
        });
        ramp(ui, s, &mut self.selected, t);
        widgets::hover_label(
            ui,
            RichText::new("Drag a peg, or edit its position below. Accent colors are independent.")
                .size(11.0)
                .color(t.text_muted),
        );
        for i in 0..4 {
            ui.push_id(("peg_row", i), |ui| {
                widgets::hover_frame(ui, widgets::surface(ui, t, i % 2 == 1), |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.selected == i, format!("Peg {}", i + 1))
                            .clicked()
                        {
                            self.selected = i;
                        }
                        color_input(ui, &mut s.stops[i].color, &mut self.hex[i]);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let mut percent = s.stops[i].position * 100.0;
                            let min = if i == 0 {
                                0.0
                            } else {
                                s.stops[i - 1].position * 100.0 + 1.0
                            };
                            let max = if i == 3 {
                                100.0
                            } else {
                                s.stops[i + 1].position * 100.0 - 1.0
                            };
                            if ui
                                .add(
                                    egui::DragValue::new(&mut percent)
                                        .speed(0.2)
                                        .range(min..=max)
                                        .suffix(" %")
                                        .max_decimals(1),
                                )
                                .on_hover_text(format!(
                                    "Peg {} position; neighbors stay at least 1% apart",
                                    i + 1
                                ))
                                .changed()
                            {
                                s.move_stop(i, percent / 100.0);
                            }
                            ui.label(RichText::new("Position").size(11.0).color(t.text_muted));
                        });
                    });
                });
            });
        }
        ui.columns(2, |columns| {
            for (i, label) in ["Primary accent", "Secondary accent"]
                .into_iter()
                .enumerate()
            {
                let frame = widgets::surface(&columns[i], t, false);
                widgets::hover_frame(&mut columns[i], frame, |ui| {
                    widgets::hover_label(ui, RichText::new(label).size(11.0).color(t.text_muted));
                    ui.horizontal(|ui| {
                        color_input(
                            ui,
                            if i == 0 {
                                &mut s.accent
                            } else {
                                &mut s.secondary
                            },
                            &mut self.hex[i + 4],
                        )
                    });
                });
            }
        });
        slider(
            ui,
            "Direction",
            &mut s.gradient_angle,
            0.0..=359.0,
            " deg",
            false,
            t,
        );
        slider(
            ui,
            "Intensity",
            &mut s.gradient_strength,
            0.0..=0.75,
            "",
            true,
            t,
        );
    }

    fn appearance(&mut self, ui: &mut egui::Ui, s: &mut ThemeSettings, t: Tokens) {
        slider(ui, "Panel opacity", &mut s.frost, 0.45..=1.0, "", false, t);
        slider(
            ui,
            "Roundness",
            &mut s.roundness,
            0.0..=18.0,
            " px",
            true,
            t,
        );
        slider(
            ui,
            "Row stripes",
            &mut s.zebra_strength,
            0.0..=0.18,
            "",
            false,
            t,
        );
        slider(
            ui,
            "Column bands",
            &mut s.column_strength,
            0.0..=0.16,
            "",
            true,
            t,
        );
        slider(
            ui,
            "Hover tint",
            &mut s.hover_strength,
            0.06..=0.30,
            "",
            false,
            t,
        );
        widgets::control_row(ui, "Readability", true, t, |ui| {
            ui.checkbox(&mut s.high_contrast, "Extra text contrast");
        });
        widgets::hover_label(ui, RichText::new("Opacity mixes panels with the backdrop; it is not desktop blur. Background brightness is bounded to protect text contrast.").size(11.0).color(t.text_muted));
        preview(ui, *s);
    }

    fn presets(&mut self, ui: &mut egui::Ui, s: &mut ThemeSettings, t: Tokens) {
        widgets::hover_label(
            ui,
            RichText::new(
                "A complete palette and surface setup. Customize any preset after loading.",
            )
            .size(11.0)
            .color(t.text_muted),
        );
        let presets = ThemeSettings::presets();
        for row in presets.chunks(2) {
            ui.columns(2, |columns| {
                for ((name, preset), ui) in row.iter().zip(columns) {
                    if preset_card(ui, name, *preset, *s == *preset, t).clicked() {
                        *s = *preset;
                    }
                }
            });
        }
        preview(ui, *s);
    }

    fn library(&mut self, ui: &mut egui::Ui, s: &mut ThemeSettings, t: Tokens) {
        widgets::hover_label(
            ui,
            RichText::new(format!("Saved themes  {}/{}", self.saved.len(), MAX_SAVED))
                .strong()
                .color(t.text),
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .hint_text("Name this theme")
                    .char_limit(32)
                    .desired_width(260.0),
            );
            let exists = self.saved.iter().any(|p| p.name == self.name.trim());
            let valid = valid_name(self.name.trim()) && (exists || self.saved.len() < MAX_SAVED);
            if ui
                .add_enabled(
                    valid,
                    egui::Button::new(if exists {
                        "Replace saved"
                    } else {
                        "Save current"
                    }),
                )
                .clicked()
            {
                self.save_named(*s);
            }
        });
        let mut remove = None;
        for (i, saved) in self.saved.iter().enumerate() {
            widgets::hover_frame(ui, widgets::surface(ui, t, i % 2 == 1), |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    widgets::hover_label(ui, RichText::new(&saved.name).color(t.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Delete")
                            .on_hover_text(
                                "Remove this named preset; the active theme stays unchanged.",
                            )
                            .clicked()
                        {
                            remove = Some(i);
                        }
                        if ui.button("Load").clicked() {
                            *s = saved.theme;
                        }
                    });
                });
            });
        }
        if let Some(index) = remove {
            self.saved.remove(index);
            self.revision += 1;
        }
        ui.separator();
        widgets::hover_label(ui, RichText::new("Share a theme").strong().color(t.text));
        widgets::hover_label(ui, RichText::new("Paste a Trontop theme below. Import only changes appearance; invalid data leaves your theme untouched.").size(11.0).color(t.text_muted));
        ui.add(
            egui::TextEdit::multiline(&mut self.transfer)
                .id_salt("theme_transfer")
                .font(egui::TextStyle::Monospace)
                .desired_rows(5)
                .desired_width(f32::INFINITY)
                .char_limit(16384)
                .hint_text("Paste theme JSON or a legacy theme code..."),
        );
        ui.horizontal(|ui| {
            if ui.button("Copy current theme").clicked() {
                self.transfer = s.encode();
                ui.ctx().copy_text(self.transfer.clone());
                self.notice = Some((
                    "Theme copied. No machine or process data is included.".into(),
                    false,
                ));
            }
            if ui
                .add_enabled(
                    !self.transfer.trim().is_empty(),
                    egui::Button::new("Import theme"),
                )
                .clicked()
            {
                self.import(s);
            }
        });
        if let Some((notice, error)) = &self.notice {
            widgets::hover_label(
                ui,
                RichText::new(notice).size(11.0).color(if *error {
                    t.ink(t.danger)
                } else {
                    t.text
                }),
            );
        }
    }

    fn import(&mut self, s: &mut ThemeSettings) {
        if let Some(theme) = ThemeSettings::decode(&self.transfer) {
            *s = theme;
            self.notice = Some((
                "Theme imported. Revert session restores your previous appearance.".into(),
                false,
            ));
        } else {
            self.notice = Some(("Invalid theme. Expected a complete v3 theme or legacy code, at most 16 KiB. Nothing changed.".into(), true));
        }
    }

    fn save_named(&mut self, theme: ThemeSettings) {
        let name = self.name.trim();
        if !valid_name(name) {
            return;
        }
        if let Some(saved) = self.saved.iter_mut().find(|p| p.name == name) {
            saved.theme = theme.normalized();
        } else if self.saved.len() < MAX_SAVED {
            self.saved.push(Saved {
                name: name.into(),
                theme: theme.normalized(),
            });
        }
        self.revision += 1;
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn encode_library(&self) -> String {
        serde_json::json!({"version": 1, "themes": self.saved.iter().map(|p| serde_json::json!({"name": p.name, "theme": p.theme.encode()})).collect::<Vec<_>>()}).to_string()
    }

    pub fn load_library(&mut self, text: &str) -> bool {
        if text.len() > 128 * 1024 {
            return false;
        }
        let parse = || -> Option<Vec<Saved>> {
            let value: serde_json::Value = serde_json::from_str(text).ok()?;
            if value["version"].as_u64()? != 1 {
                return None;
            }
            let rows = value["themes"].as_array()?;
            if rows.len() > MAX_SAVED {
                return None;
            }
            let mut saved: Vec<Saved> = Vec::new();
            for row in rows {
                let name = row["name"].as_str()?;
                if !valid_name(name) || saved.iter().any(|p| p.name == name) {
                    return None;
                }
                saved.push(Saved {
                    name: name.into(),
                    theme: ThemeSettings::decode(row["theme"].as_str()?)?,
                });
            }
            Some(saved)
        };
        if let Some(saved) = parse() {
            self.saved = saved;
            self.revision += 1;
            true
        } else {
            false
        }
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().count() <= 32 && !name.chars().any(char::is_control)
}
fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}
fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let text = text.trim().strip_prefix('#').unwrap_or(text.trim());
    if text.len() != 6 || !text.is_ascii() {
        return None;
    }
    Some([
        u8::from_str_radix(&text[0..2], 16).ok()?,
        u8::from_str_radix(&text[2..4], 16).ok()?,
        u8::from_str_radix(&text[4..6], 16).ok()?,
    ])
}

fn color_input(ui: &mut egui::Ui, color: &mut [u8; 3], text: &mut String) {
    if ui.color_edit_button_srgb(color).changed() {
        *text = hex(*color);
    }
    let response = ui.add(
        egui::TextEdit::singleline(text)
            .font(egui::TextStyle::Monospace)
            .desired_width(86.0)
            .char_limit(7),
    );
    if response.changed()
        && let Some(parsed) = parse_hex(text)
    {
        *color = parsed;
    }
    let valid = parse_hex(text).is_some();
    if !valid {
        ui.painter().rect_stroke(
            response.rect,
            3.0,
            Stroke::new(1.0, ui.visuals().error_fg_color),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_text(if valid {
        "RGB hex color; six digits, with optional #"
    } else {
        "Enter six hexadecimal digits. The last valid color is retained."
    });
}

fn slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
    banded: bool,
    t: Tokens,
) {
    widgets::control_row(ui, label, banded, t, |ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 95.0).clamp(80.0, 340.0);
        ui.add(egui::Slider::new(value, range).suffix(suffix));
    });
}

fn ramp(
    ui: &mut egui::Ui,
    s: &mut ThemeSettings,
    selected: &mut usize,
    t: Tokens,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 66.0),
        Sense::click_and_drag(),
    );
    let rail = egui::Rect::from_min_max(
        rect.min + Vec2::new(12.0, 4.0),
        rect.right_bottom() - Vec2::new(12.0, 26.0),
    );
    if (response.clicked()
        || (response.is_pointer_button_down_on()
            && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))))
        && let Some(p) = response.interact_pointer_pos()
    {
        *selected = (0..4)
            .min_by(|&a, &b| {
                ((rail.left() + rail.width() * s.stops[a].position) - p.x)
                    .abs()
                    .total_cmp(&((rail.left() + rail.width() * s.stops[b].position) - p.x).abs())
            })
            .unwrap_or(0);
        response.request_focus();
    }
    if response.dragged()
        && let Some(p) = response.interact_pointer_pos()
    {
        s.move_stop(*selected, (p.x - rail.left()) / rail.width());
    }
    if response.has_focus() {
        ui.input(|input| {
            let step = if input.modifiers.shift { 0.05 } else { 0.01 };
            if input.key_pressed(egui::Key::ArrowLeft) {
                s.move_stop(*selected, s.stops[*selected].position - step);
            }
            if input.key_pressed(egui::Key::ArrowRight) {
                s.move_stop(*selected, s.stops[*selected].position + step);
            }
            if input.key_pressed(egui::Key::ArrowUp) {
                *selected = selected.saturating_sub(1);
            }
            if input.key_pressed(egui::Key::ArrowDown) {
                *selected = (*selected + 1).min(3);
            }
        });
    }
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, 5.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    theme::paint_gradient(ui.painter(), rail, s.stops, 0.0, |c| c);
    ui.painter().rect_stroke(
        rail,
        0.0,
        Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );
    for (i, peg) in s.stops.iter().enumerate() {
        let x = rail.left() + rail.width() * peg.position;
        let color = Color32::from_rgb(peg.color[0], peg.color[1], peg.color[2]);
        let peg_rect = egui::Rect::from_center_size(
            egui::pos2(x, rail.bottom() + 13.0),
            Vec2::new(18.0, 20.0),
        );
        ui.painter().line_segment(
            [egui::pos2(x, rail.bottom()), peg_rect.center_top()],
            Stroke::new(1.0, t.text),
        );
        ui.painter().rect_filled(peg_rect, 4.0, color);
        ui.painter().rect_stroke(
            peg_rect,
            4.0,
            Stroke::new(
                if i == *selected { 2.0 } else { 1.0 },
                theme::readable_text(if i == *selected { t.text } else { t.border }, color),
            ),
            egui::StrokeKind::Inside,
        );
        let ink = theme::readable_text(t.text, color);
        ui.painter().text(
            peg_rect.center(),
            Align2::CENTER_CENTER,
            (i + 1).to_string(),
            FontId::monospace(11.0),
            ink,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Four color gradient pegs")
    });
    response.on_hover_text("Drag the nearest peg. Keyboard: left/right moves it 1%, Shift moves 5%; up/down selects another peg.")
}

fn preset_card(
    ui: &mut egui::Ui,
    name: &str,
    preset: ThemeSettings,
    selected: bool,
    t: Tokens,
) -> egui::Response {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 70.0), Sense::click());
    let rect = response.rect;
    let fill = if response.hovered() {
        t.row_hover
    } else if selected {
        t.accent_dim
    } else {
        t.panel_raised
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().rect_stroke(
        rect,
        6.0,
        Stroke::new(
            1.0,
            if selected || response.has_focus() {
                t.ink(t.secondary)
            } else {
                t.border
            },
        ),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.min + Vec2::new(10.0, 14.0),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(13.0),
        t.text,
    );
    ui.painter().text(
        rect.right_top() + Vec2::new(-10.0, 14.0),
        Align2::RIGHT_CENTER,
        if preset.dark { "DARK" } else { "LIGHT" },
        FontId::monospace(9.0),
        t.text_muted,
    );
    let rail = egui::Rect::from_min_max(
        rect.min + Vec2::new(10.0, 32.0),
        rect.max - Vec2::new(10.0, 10.0),
    );
    theme::paint_gradient(ui.painter(), rail, preset.stops, 0.0, |c| c);
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, name));
    response.on_hover_text(format!(
        "Load {name}: colors, gradient, surfaces and readability settings"
    ))
}

fn preview(ui: &mut egui::Ui, s: ThemeSettings) {
    let t = theme::tokens(s);
    widgets::hover_frame(ui, widgets::surface(ui, t, false), |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            widgets::tront_mark(ui, t.accent, t.secondary, 28.0);
            widgets::hover_label(
                ui,
                RichText::new("Component preview").strong().color(t.text),
            );
            widgets::status_pill(ui, "Live", t.good);
        });
        for i in 0..2 {
            widgets::hover_frame(ui, widgets::surface(ui, t, i % 2 == 1), |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    widgets::hover_label(
                        ui,
                        RichText::new(if i == 0 {
                            "trontop.exe"
                        } else {
                            "Background process"
                        })
                        .color(t.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(if i == 0 { "84.6 MiB" } else { "12.8 MiB" })
                                .monospace()
                                .color(t.text),
                        );
                    });
                });
            });
        }
    });
}

#[cfg(test)]
mod tests;
