use eframe::egui;
use egui::{Color32, FontFamily, FontId, Stroke, TextStyle, Theme, Visuals};

pub const STORAGE_KEY: &str = "trontop.theme.v2";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeSettings {
    pub dark: bool,
    pub accent: [u8; 3],
    pub secondary: [u8; 3],
    pub gradient_enabled: bool,
    pub gradient_angle: f32,
    pub gradient_strength: f32,
    pub frost: f32,
    pub roundness: f32,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self::tront_stack()
    }
}

impl ThemeSettings {
    pub fn tront_stack() -> Self {
        Self {
            dark: true,
            accent: [168, 85, 247],
            secondary: [46, 230, 215],
            gradient_enabled: true,
            gradient_angle: 132.0,
            gradient_strength: 0.34,
            frost: 0.80,
            roundness: 8.0,
        }
    }

    pub fn demigod() -> Self {
        Self {
            accent: [245, 91, 48],
            secondary: [255, 181, 66],
            gradient_angle: 158.0,
            ..Self::tront_stack()
        }
    }

    pub fn monke_portal() -> Self {
        Self {
            accent: [46, 230, 215],
            secondary: [111, 78, 255],
            gradient_angle: 28.0,
            ..Self::tront_stack()
        }
    }

    pub fn copper_legacy() -> Self {
        Self {
            accent: [224, 104, 52],
            secondary: [200, 146, 78],
            gradient_angle: 116.0,
            ..Self::tront_stack()
        }
    }

    pub fn encode(self) -> String {
        format!(
            "{};{},{},{};{},{},{};{};{:.2};{:.3};{:.3};{:.2}",
            u8::from(self.dark),
            self.accent[0],
            self.accent[1],
            self.accent[2],
            self.secondary[0],
            self.secondary[1],
            self.secondary[2],
            u8::from(self.gradient_enabled),
            self.gradient_angle,
            self.gradient_strength,
            self.frost,
            self.roundness,
        )
    }

    pub fn decode(value: &str) -> Option<Self> {
        let fields = value.split(';').collect::<Vec<_>>();
        if fields.len() != 8 {
            return None;
        }
        Some(
            Self {
                dark: fields[0] == "1",
                accent: parse_rgb(fields[1])?,
                secondary: parse_rgb(fields[2])?,
                gradient_enabled: fields[3] == "1",
                gradient_angle: fields[4].parse().ok()?,
                gradient_strength: fields[5].parse().ok()?,
                frost: fields[6].parse().ok()?,
                roundness: fields[7].parse().ok()?,
            }
            .normalized(),
        )
    }

    pub fn normalized(mut self) -> Self {
        self.gradient_angle = self.gradient_angle.rem_euclid(360.0);
        self.gradient_strength = self.gradient_strength.clamp(0.0, 0.75);
        self.frost = self.frost.clamp(0.45, 1.0);
        self.roundness = self.roundness.clamp(0.0, 18.0);
        self
    }
}

#[derive(Clone, Copy)]
pub struct Tokens {
    pub bg: Color32,
    pub panel: Color32,
    pub panel_raised: Color32,
    pub row_hover: Color32,
    pub accent: Color32,
    pub secondary: Color32,
    pub accent_dim: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub border: Color32,
    pub good: Color32,
    pub danger: Color32,
    pub graph_bg: Color32,
}

pub fn tokens(settings: ThemeSettings) -> Tokens {
    let accent = rgb(settings.accent);
    let secondary = rgb(settings.secondary);
    if settings.dark {
        Tokens {
            bg: Color32::from_rgb(9, 9, 13),
            panel: Color32::from_rgb(16, 16, 23),
            panel_raised: Color32::from_rgb(25, 24, 34),
            row_hover: mix(Color32::from_rgb(30, 29, 42), accent, 0.14),
            accent,
            secondary,
            accent_dim: mix(Color32::from_rgb(22, 19, 31), accent, 0.38),
            text: Color32::from_rgb(240, 238, 247),
            text_muted: Color32::from_rgb(184, 179, 198),
            border: Color32::from_rgb(63, 59, 78),
            good: Color32::from_rgb(75, 215, 151),
            danger: Color32::from_rgb(232, 75, 85),
            graph_bg: Color32::from_rgb(8, 8, 13),
        }
    } else {
        Tokens {
            bg: Color32::from_rgb(236, 234, 242),
            panel: Color32::from_rgb(247, 246, 250),
            panel_raised: Color32::WHITE,
            row_hover: mix(Color32::WHITE, accent, 0.10),
            accent,
            secondary,
            accent_dim: mix(Color32::WHITE, accent, 0.24),
            text: Color32::from_rgb(28, 25, 35),
            text_muted: Color32::from_rgb(78, 72, 91),
            border: Color32::from_rgb(185, 179, 196),
            good: Color32::from_rgb(24, 145, 90),
            danger: Color32::from_rgb(199, 47, 59),
            graph_bg: Color32::from_rgb(229, 226, 236),
        }
    }
}

pub fn install(ctx: &egui::Context, settings: ThemeSettings) {
    let settings = settings.normalized();
    let t = tokens(settings);
    let theme = if settings.dark {
        Theme::Dark
    } else {
        Theme::Light
    };
    ctx.set_theme(theme);
    let mut style = (*ctx.style_of(theme)).clone();
    style.visuals = if settings.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    style.visuals.panel_fill = Color32::TRANSPARENT;
    style.visuals.window_fill = t.panel;
    style.visuals.extreme_bg_color = t.graph_bg;
    let signal_blend = mix(t.accent, t.secondary, 0.48);
    style.visuals.faint_bg_color = mix(t.panel_raised, signal_blend, 0.075);
    style.visuals.selection.bg_fill = mix(t.accent_dim, signal_blend, 0.24);
    style.visuals.selection.stroke = Stroke::new(1.0, t.text);
    style.visuals.widgets.noninteractive.fg_stroke.color = t.text;
    style.visuals.widgets.inactive.bg_fill = t.panel_raised;
    style.visuals.widgets.inactive.fg_stroke.color = t.text_muted;
    style.visuals.widgets.inactive.weak_bg_fill = t.panel_raised;
    style.visuals.widgets.hovered.bg_fill = t.row_hover;
    style.visuals.widgets.hovered.weak_bg_fill = mix(t.row_hover, signal_blend, 0.18);
    style.visuals.widgets.hovered.fg_stroke.color = t.text;
    style.visuals.widgets.active.bg_fill = t.accent_dim;
    style.visuals.widgets.active.fg_stroke.color = t.text;
    style.visuals.window_stroke = Stroke::new(1.0, t.border);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, t.border);
    style.visuals.window_corner_radius = settings.roundness.into();
    style.visuals.menu_corner_radius = settings.roundness.into();
    for widgets in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widgets.corner_radius = (settings.roundness * 0.65).into();
    }
    style.interaction.selectable_labels = false;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(11.0, 6.0);
    style.spacing.scroll.floating = false;
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
    ctx.set_style_of(theme, style);
}

pub fn paint_background(ctx: &egui::Context, settings: ThemeSettings) {
    let settings = settings.normalized();
    let t = tokens(settings);
    let rect = ctx.content_rect();
    if !settings.gradient_enabled || rect.width() <= 0.0 || rect.height() <= 0.0 {
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(rect, 0.0, t.bg);
        return;
    }
    let angle = settings.gradient_angle.to_radians();
    let direction = egui::vec2(angle.cos(), angle.sin());
    let center = rect.center();
    let half =
        (rect.width() * 0.5 * direction.x.abs() + rect.height() * 0.5 * direction.y.abs()).max(1.0);
    const GRID: usize = 10;
    let mut mesh = egui::Mesh::default();
    for y in 0..=GRID {
        for x in 0..=GRID {
            let pos = egui::pos2(
                egui::lerp(rect.left()..=rect.right(), x as f32 / GRID as f32),
                egui::lerp(rect.top()..=rect.bottom(), y as f32 / GRID as f32),
            );
            let phase = (((pos - center).dot(direction) / half) * 0.5 + 0.5).clamp(0.0, 1.0);
            let color = mix(
                t.bg,
                mix(t.accent, t.secondary, phase),
                settings.gradient_strength,
            );
            mesh.colored_vertex(pos, color);
        }
    }
    let width = (GRID + 1) as u32;
    for y in 0..GRID as u32 {
        for x in 0..GRID as u32 {
            let i = y * width + x;
            mesh.add_triangle(i, i + 1, i + width);
            mesh.add_triangle(i + 1, i + width + 1, i + width);
        }
    }
    ctx.layer_painter(egui::LayerId::background())
        .add(egui::Shape::mesh(mesh));
}

pub fn panel_color(settings: ThemeSettings) -> Color32 {
    let t = tokens(settings);
    let alpha = (settings.frost.clamp(0.45, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(t.panel.r(), t.panel.g(), t.panel.b(), alpha)
}

pub fn raised_color(settings: ThemeSettings) -> Color32 {
    let t = tokens(settings);
    let alpha = (settings.frost.clamp(0.45, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(
        t.panel_raised.r(),
        t.panel_raised.g(),
        t.panel_raised.b(),
        alpha,
    )
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let channel = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
        channel(a.a(), b.a()),
    )
}

fn rgb(value: [u8; 3]) -> Color32 {
    Color32::from_rgb(value[0], value[1], value[2])
}

fn parse_rgb(value: &str) -> Option<[u8; 3]> {
    let values = value
        .split(',')
        .map(str::parse)
        .collect::<Result<Vec<u8>, _>>()
        .ok()?;
    Some([*values.first()?, *values.get(1)?, *values.get(2)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let settings = ThemeSettings::monke_portal();
        assert_eq!(ThemeSettings::decode(&settings.encode()), Some(settings));
    }

    #[test]
    fn malformed_settings_are_rejected() {
        assert!(ThemeSettings::decode("broken").is_none());
    }
}
