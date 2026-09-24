use super::*;
use crate::theme::typography::FontChoice;

#[test]
#[ignore = "offscreen theme and font review only; creates no native window or tray"]
fn render_theme_controls_visual_pass() {
    let mut renderer = offscreen::Renderer::new();
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke/theme-controls");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, dark, frost, tab, font) in [
        ("dark-clear-palette", true, 0.0, Some(0), FontChoice::Sans),
        ("dark-frost-palette", true, 0.35, Some(0), FontChoice::Sans),
        ("light-clear-palette", false, 0.0, Some(0), FontChoice::Sans),
        (
            "light-frost-appearance",
            false,
            0.35,
            Some(1),
            FontChoice::Sans,
        ),
        ("dark-appearance", true, 0.2, Some(1), FontChoice::Sans),
        (
            "dark-rajdhani-type",
            true,
            0.2,
            Some(4),
            FontChoice::Rajdhani,
        ),
        (
            "light-bold-type",
            false,
            0.2,
            Some(4),
            FontChoice::RajdhaniBold,
        ),
        ("dark-mono-type", true, 0.2, Some(4), FontChoice::Mono),
        ("dark-clear-processes", true, 0.0, None, FontChoice::Sans),
        ("light-clear-processes", false, 0.0, None, FontChoice::Sans),
        (
            "dark-bold-overview",
            true,
            0.2,
            None,
            FontChoice::RajdhaniBold,
        ),
    ] {
        let settings = ThemeSettings {
            dark,
            frost,
            frost_light: frost,
            gradient_strength: 1.0,
            surface_tint: 0.25,
            font,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.page = if name.ends_with("overview") {
            Page::Overview
        } else {
            Page::Processes
        };
        app.inspector_visible = false;
        app.show_theme_editor = tab.is_some();
        app.theme_studio.tab = tab.unwrap_or(0);
        let size = Vec2::new(1040.0, 640.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..20 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    println!(
        "Theme controls: 11 offscreen PNGs written to {}",
        directory.display()
    );
}
