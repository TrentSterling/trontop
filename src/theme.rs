use eframe::egui;
use egui::{Color32, FontFamily, FontId, Stroke, TextStyle, Theme, Visuals};

pub const STORAGE_KEY: &str = "trontop.theme.v3";
pub const LEGACY_STORAGE_KEY: &str = "trontop.theme.v2";

mod contrast;
mod gradient;
mod storage;
#[cfg(test)]
pub(crate) use contrast::ratio as contrast_ratio;
pub use contrast::{ink, readable_text, surface as text_surface};
pub use gradient::{Stop, paint_gradient};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeSettings {
    pub dark: bool,
    pub accent: [u8; 3],
    pub secondary: [u8; 3],
    pub stops: [Stop; 4],
    pub gradient_enabled: bool,
    pub gradient_angle: f32,
    pub gradient_strength: f32,
    pub frost: f32,
    pub roundness: f32,
    pub zebra_strength: f32,
    pub column_strength: f32,
    pub hover_strength: f32,
    pub high_contrast: bool,
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
            stops: Stop::palette([
                [168, 85, 247],
                [96, 119, 250],
                [46, 230, 215],
                [43, 127, 185],
            ]),
            gradient_enabled: true,
            gradient_angle: 132.0,
            gradient_strength: 0.34,
            frost: 0.80,
            roundness: 8.0,
            zebra_strength: 0.075,
            column_strength: 0.05,
            hover_strength: 0.18,
            high_contrast: false,
        }
    }

    pub fn demigod() -> Self {
        Self {
            accent: [245, 91, 48],
            secondary: [255, 181, 66],
            stops: Stop::palette([[245, 91, 48], [174, 63, 86], [220, 131, 55], [255, 181, 66]]),
            gradient_angle: 158.0,
            ..Self::tront_stack()
        }
    }

    pub fn monke_portal() -> Self {
        Self {
            accent: [46, 230, 215],
            secondary: [111, 78, 255],
            stops: Stop::palette([
                [46, 230, 215],
                [32, 133, 190],
                [93, 77, 188],
                [111, 78, 255],
            ]),
            gradient_angle: 28.0,
            ..Self::tront_stack()
        }
    }

    pub fn copper_legacy() -> Self {
        Self {
            accent: [224, 104, 52],
            secondary: [200, 146, 78],
            stops: Stop::palette([
                [224, 104, 52],
                [129, 75, 54],
                [174, 109, 54],
                [200, 146, 78],
            ]),
            gradient_angle: 116.0,
            ..Self::tront_stack()
        }
    }

    pub fn presets() -> [(&'static str, Self); 8] {
        [
            ("TrontStack", Self::tront_stack()),
            ("Demigod", Self::demigod()),
            ("Monke Portal", Self::monke_portal()),
            ("Copper Legacy", Self::copper_legacy()),
            (
                "Porcelain",
                Self {
                    dark: false,
                    accent: [25, 101, 226],
                    secondary: [90, 67, 185],
                    stops: Stop::palette([
                        [124, 169, 226],
                        [162, 189, 222],
                        [190, 175, 218],
                        [145, 204, 211],
                    ]),
                    gradient_strength: 0.15,
                    frost: 0.94,
                    ..Self::default()
                },
            ),
            (
                "Carbon",
                Self {
                    accent: [238, 175, 89],
                    secondary: [196, 147, 90],
                    stops: Stop::palette([
                        [128, 96, 58],
                        [84, 79, 70],
                        [111, 89, 66],
                        [156, 113, 68],
                    ]),
                    gradient_strength: 0.20,
                    frost: 0.92,
                    ..Self::default()
                },
            ),
            (
                "Phosphor",
                Self {
                    accent: [69, 221, 152],
                    secondary: [120, 239, 181],
                    stops: Stop::palette([[20, 93, 67], [23, 70, 60], [44, 114, 70], [24, 86, 57]]),
                    roundness: 3.0,
                    high_contrast: true,
                    ..Self::default()
                },
            ),
            (
                "Vector",
                Self {
                    accent: [47, 217, 235],
                    secondary: [240, 100, 146],
                    stops: Stop::palette([
                        [25, 131, 166],
                        [70, 55, 148],
                        [125, 50, 130],
                        [206, 75, 111],
                    ]),
                    roundness: 5.0,
                    ..Self::default()
                },
            ),
        ]
    }

    pub fn normalized(mut self) -> Self {
        let bounded = |v: f32, fallback, min, max| {
            if v.is_finite() {
                v.clamp(min, max)
            } else {
                fallback
            }
        };
        self.gradient_angle =
            bounded(self.gradient_angle, 132.0, -36000.0, 36000.0).rem_euclid(360.0);
        self.gradient_strength = bounded(self.gradient_strength, 0.34, 0.0, 0.75);
        self.frost = bounded(self.frost, 0.80, 0.45, 1.0);
        self.roundness = bounded(self.roundness, 8.0, 0.0, 18.0);
        self.zebra_strength = bounded(self.zebra_strength, 0.075, 0.0, 0.18);
        self.column_strength = bounded(self.column_strength, 0.05, 0.0, 0.16);
        self.hover_strength = bounded(self.hover_strength, 0.18, 0.06, 0.30);
        gradient::normalize(&mut self.stops);
        self
    }
}

#[derive(Clone, Copy)]
pub struct Tokens {
    pub dark: bool,
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
    pub column_strength: f32,
}

impl Tokens {
    pub fn surface(self, color: Color32) -> Color32 {
        text_surface(color, self.dark)
    }

    pub fn ink(self, color: Color32) -> Color32 {
        ink(color, self.dark)
    }
}

pub fn tokens(settings: ThemeSettings) -> Tokens {
    let accent = rgb(settings.accent);
    let secondary = rgb(settings.secondary);
    let mut result = if settings.dark {
        Tokens {
            dark: true,
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
            column_strength: settings.column_strength,
        }
    } else {
        Tokens {
            dark: false,
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
            column_strength: settings.column_strength,
        }
    };
    if settings.high_contrast {
        result.text = if settings.dark {
            Color32::WHITE
        } else {
            Color32::from_rgb(12, 12, 18)
        };
        result.text_muted = result.text;
        result.border = mix(result.border, result.text, 0.25);
    }
    result.row_hover = result.surface(result.row_hover);
    result.accent_dim = result.surface(result.accent_dim);
    result
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
    style.visuals.weak_text_color = Some(t.text_muted);
    style.visuals.extreme_bg_color = t.graph_bg;
    let signal_blend = mix(t.accent, t.secondary, 0.48);
    style.visuals.faint_bg_color =
        t.surface(mix(t.panel_raised, signal_blend, settings.zebra_strength));
    style.visuals.selection.bg_fill = t.surface(mix(t.accent_dim, signal_blend, 0.24));
    style.visuals.selection.stroke = Stroke::new(1.0, t.text);
    style.visuals.widgets.noninteractive.fg_stroke.color = t.text;
    // Strong fill is used by slider rails and handles; keep it visible on cards.
    style.visuals.widgets.inactive.bg_fill = t.border;
    style.visuals.slider_trailing_fill = true;
    style.visuals.widgets.inactive.fg_stroke.color = t.text_muted;
    style.visuals.widgets.inactive.weak_bg_fill = t.panel_raised;
    style.visuals.widgets.hovered.bg_fill = t.row_hover;
    style.visuals.widgets.hovered.weak_bg_fill =
        t.surface(mix(t.row_hover, signal_blend, settings.hover_strength));
    style.visuals.widgets.hovered.fg_stroke.color = t.text;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, t.ink(t.secondary));
    style.visuals.widgets.active.bg_fill = t.accent_dim;
    style.visuals.widgets.active.weak_bg_fill = t.accent_dim;
    style.visuals.widgets.active.fg_stroke.color = t.text;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, t.ink(t.accent));
    style.visuals.widgets.open = style.visuals.widgets.active;
    style.visuals.hyperlink_color = t.ink(t.secondary);
    style.visuals.error_fg_color = t.ink(t.danger);
    style.visuals.warn_fg_color = t.ink(Color32::from_rgb(230, 160, 50));
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
    paint_gradient(
        &ctx.layer_painter(egui::LayerId::background()),
        rect,
        settings.stops,
        settings.gradient_angle,
        |color| backdrop(settings, color),
    );
}

/// Keep backdrop luminance bounded independently of the user's four raw colors.
/// Text on transparent panels must remain readable even at maximum intensity.
pub fn backdrop(settings: ThemeSettings, color: Color32) -> Color32 {
    let t = tokens(settings);
    let raw = mix(t.bg, color, settings.gradient_strength);
    t.surface(raw)
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

    #[test]
    fn arbitrary_gradient_colors_keep_backdrop_text_readable() {
        fn luminance(c: Color32) -> f32 {
            let linear = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
        }
        for dark in [false, true] {
            let s = ThemeSettings {
                dark,
                gradient_strength: 0.75,
                frost: 0.45,
                ..Default::default()
            };
            let t = tokens(s);
            for red in [0, 64, 128, 192, 255] {
                for green in [0, 64, 128, 192, 255] {
                    for blue in [0, 64, 128, 192, 255] {
                        let bg = backdrop(s, Color32::from_rgb(red, green, blue));
                        for fg in [t.text, t.text_muted] {
                            let (a, b) = (luminance(fg), luminance(bg));
                            assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
                        }
                    }
                }
            }
        }
    }
}
