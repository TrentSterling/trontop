//! The generated logo supplies shape/alpha only; live theme colors supply ink.
//! Build-time decoding keeps PNG libraries and filesystem reads out of the UI.
use crate::theme::{self, Stop, ThemeSettings};
use eframe::egui::{self, Color32};
use std::sync::Arc;

const SMALL: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/logo-alpha-64.bin"));
const LARGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/logo-alpha-128.bin"));

#[derive(Clone, PartialEq)]
struct Palette {
    dark: bool,
    stops: [Stop; 4],
    angle: f32,
    gradient: bool,
    accent: [u8; 3],
}
impl From<ThemeSettings> for Palette {
    fn from(s: ThemeSettings) -> Self {
        Self {
            dark: s.dark,
            stops: s.stops,
            angle: s.gradient_angle,
            gradient: s.gradient_enabled,
            accent: s.accent,
        }
    }
}
#[derive(Clone)]
struct Cached {
    palette: Palette,
    texture: egui::TextureHandle,
}

fn pixels(s: ThemeSettings, alpha: &[u8], size: u32) -> Vec<u8> {
    let angle = s.gradient_angle.to_radians();
    let (dy, dx) = angle.sin_cos();
    let span = (dx.abs() + dy.abs()).max(0.001);
    // Lift dark palettes (or darken pale palettes in light mode) only in the
    // rendered mark. The envelope includes every composited Trontop surface,
    // so its silhouette and transparent internal cuts remain distinguishable.
    // A small ramp avoids doing the contrast search for every pixel.
    let ramp: [Color32; 512] = std::array::from_fn(|i| {
        let raw = if s.gradient_enabled {
            s.gradient_color(i as f32 / 511.0)
        } else {
            Color32::from_rgb(s.accent[0], s.accent[1], s.accent[2])
        };
        theme::ink(raw, s.dark)
    });
    let mut rgba = Vec::with_capacity(alpha.len() * 4);
    for (i, &a) in alpha.iter().enumerate() {
        let x = (i as u32 % size) as f32 / (size - 1) as f32 - 0.5;
        let y = (i as u32 / size) as f32 / (size - 1) as f32 - 0.5;
        let phase = (0.5 + (x * dx + y * dy) / span).clamp(0.0, 1.0);
        let color = ramp[(phase * 511.0).round() as usize];
        rgba.extend_from_slice(&[color.r(), color.g(), color.b(), a]);
    }
    rgba
}

pub fn window_icon(s: ThemeSettings) -> Arc<egui::IconData> {
    Arc::new(egui::IconData {
        rgba: pixels(s, SMALL, 64),
        width: 64,
        height: 64,
    })
}

/// Owned by the native app, so restoring egui memory cannot disable icon updates.
#[derive(Default)]
pub struct NativeIcon {
    enabled: bool,
    palette: Option<Palette>,
}
impl NativeIcon {
    pub fn new() -> Self {
        Self {
            enabled: true,
            palette: None,
        }
    }
    pub fn sync(&mut self, ctx: &egui::Context, s: ThemeSettings) {
        let palette = Palette::from(s);
        if self.enabled && self.palette.as_ref() != Some(&palette) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(window_icon(s))));
            self.palette = Some(palette);
        }
    }
}

pub fn install(ctx: &egui::Context, s: ThemeSettings) {
    let id = egui::Id::new("trontop.live-logo");
    let palette = Palette::from(s);
    if ctx
        .data(|data| data.get_temp::<Cached>(id))
        .is_some_and(|old| old.palette == palette)
    {
        return;
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([128, 128], &pixels(s, LARGE, 128));
    let texture = ctx.load_texture("trontop-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, Cached { palette, texture }));
}

pub fn show(ui: &mut egui::Ui, s: ThemeSettings, size: f32) {
    install(ui.ctx(), s);
    let cached = ui
        .ctx()
        .data(|data| data.get_temp::<Cached>(egui::Id::new("trontop.live-logo")))
        .unwrap();
    ui.add(egui::Image::new((
        cached.texture.id(),
        egui::vec2(size, size),
    )));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_and_extreme_logo_palettes_stay_readable_without_changing_the_theme() {
        for dark in [true, false] {
            for seed in 0..48 {
                let mut s = ThemeSettings {
                    dark,
                    ..Default::default()
                };
                theme::magic::randomize(
                    &mut s,
                    seed,
                    theme::magic::Flavor::ALL[seed as usize % theme::magic::Flavor::ALL.len()],
                );
                if seed < 3 {
                    s.stops = Stop::palette([[seed as u8 * 127; 3]; 4]);
                }
                let before = s;
                let icon = window_icon(s);
                let worst_surface = Color32::from_gray(if dark { 64 } else { 205 });
                for (pixel, &alpha) in icon.rgba.chunks_exact(4).zip(SMALL) {
                    assert_eq!(pixel[3], alpha, "logo cuts/alpha changed");
                    if alpha == 255 {
                        let color = Color32::from_rgb(pixel[0], pixel[1], pixel[2]);
                        assert!(
                            theme::contrast_ratio(color, worst_surface) >= 4.5,
                            "dark={dark} seed={seed}: {color:?} on {worst_surface:?}"
                        );
                    }
                }
                assert_eq!(s, before);
            }
        }
        let dark = ThemeSettings::default();
        assert_ne!(
            window_icon(dark).rgba,
            window_icon(ThemeSettings {
                dark: false,
                ..dark
            })
            .rgba
        );
    }

    #[test]
    fn logo_retains_generated_alpha_and_tracks_palette_angle_and_solid_mode() {
        let s = ThemeSettings::default();
        let original = window_icon(s);
        for changed in [
            ThemeSettings::demigod(),
            ThemeSettings {
                gradient_angle: 0.0,
                ..s
            },
            ThemeSettings {
                gradient_enabled: false,
                accent: [255, 0, 0],
                ..s
            },
        ] {
            let icon = window_icon(changed);
            assert_ne!(icon.rgba, original.rgba);
            for (pixel, alpha) in icon.rgba.chunks_exact(4).zip(SMALL) {
                assert_eq!(pixel[3], *alpha);
            }
            if !changed.gradient_enabled {
                let ink = theme::ink(Color32::from_rgb(255, 0, 0), s.dark);
                assert!(
                    icon.rgba
                        .chunks_exact(4)
                        .all(|pixel| pixel[..3] == [ink.r(), ink.g(), ink.b()])
                );
            }
        }
        assert!(SMALL.contains(&0));
        assert!(SMALL.iter().any(|&a| a > 240));
        assert_eq!(
            window_icon(ThemeSettings {
                frost: 0.0,
                gradient_strength: 1.0,
                ..s
            })
            .rgba,
            original.rgba
        );
    }

    #[test]
    fn ordinary_theme_edits_do_not_reupload_logo_or_queue_icon_updates() {
        let ctx = egui::Context::default();
        let mut native = NativeIcon::new();
        let s = ThemeSettings::default();
        install(&ctx, s);
        native.sync(&ctx, s);
        let id = ctx
            .data(|data| data.get_temp::<Cached>(egui::Id::new("trontop.live-logo")))
            .unwrap()
            .texture
            .id();
        let output = ctx.run_ui(Default::default(), |_| {});
        assert!(output.viewport_output.values().any(|v| {
            v.commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Icon(_)))
        }));
        install(
            &ctx,
            ThemeSettings {
                frost: 0.0,
                surface_tint: 0.9,
                ..s
            },
        );
        native.sync(&ctx, s);
        assert_eq!(
            ctx.data(|data| data.get_temp::<Cached>(egui::Id::new("trontop.live-logo")))
                .unwrap()
                .texture
                .id(),
            id
        );
        let output = ctx.run_ui(Default::default(), |_| {});
        assert!(
            output
                .viewport_output
                .values()
                .all(|v| v.commands.is_empty())
        );
        // Async preference restore replaces egui memory. Native icon ownership
        // belongs to the app, so a restored palette still reaches Windows once.
        ctx.memory_mut(|memory| *memory = egui::Memory::default());
        native.sync(&ctx, ThemeSettings::demigod());
        let output = ctx.run_ui(Default::default(), |_| {});
        assert_eq!(
            output
                .viewport_output
                .values()
                .flat_map(|v| &v.commands)
                .filter(|c| matches!(c, egui::ViewportCommand::Icon(_)))
                .count(),
            1
        );
    }
}
