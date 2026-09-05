use eframe::egui;
use egui::{Color32, FontFamily, FontId, Stroke, TextStyle, Theme, Visuals};

pub const BG: Color32 = Color32::from_rgb(15, 14, 13);
pub const PANEL: Color32 = Color32::from_rgb(22, 20, 18);
pub const PANEL_RAISED: Color32 = Color32::from_rgb(29, 26, 23);
pub const ROW_HOVER: Color32 = Color32::from_rgb(43, 34, 28);
pub const COPPER: Color32 = Color32::from_rgb(224, 104, 52);
pub const COPPER_DIM: Color32 = Color32::from_rgb(121, 62, 38);
pub const TEXT: Color32 = Color32::from_rgb(234, 229, 222);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(146, 137, 128);
pub const BORDER: Color32 = Color32::from_rgb(54, 47, 42);
pub const GOOD: Color32 = Color32::from_rgb(86, 180, 130);
pub const DANGER: Color32 = Color32::from_rgb(225, 80, 66);

pub fn install(ctx: &egui::Context) {
    ctx.set_theme(Theme::Dark);
    let mut style = (*ctx.style_of(Theme::Dark)).clone();
    style.visuals = Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = Color32::from_rgb(10, 9, 8);
    style.visuals.faint_bg_color = PANEL_RAISED;
    style.visuals.selection.bg_fill = COPPER_DIM;
    style.visuals.selection.stroke = Stroke::new(1.0, COPPER);
    style.visuals.widgets.noninteractive.fg_stroke.color = TEXT;
    style.visuals.widgets.inactive.bg_fill = PANEL_RAISED;
    style.visuals.widgets.inactive.fg_stroke.color = TEXT_MUTED;
    style.visuals.widgets.inactive.weak_bg_fill = PANEL_RAISED;
    style.visuals.widgets.hovered.bg_fill = ROW_HOVER;
    style.visuals.widgets.hovered.fg_stroke.color = TEXT;
    style.visuals.widgets.active.bg_fill = COPPER_DIM;
    style.visuals.widgets.active.fg_stroke.color = TEXT;
    style.visuals.window_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(11.0, 6.0);
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(24.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.set_style_of(Theme::Dark, style);
}
