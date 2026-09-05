//! Pathological themes use production UI on an offscreen texture, never OS input.
use super::*;

pub(super) fn render_cases(renderer: &mut offscreen::Renderer, directory: &std::path::Path) {
    for (name, dark, color, history) in [
        (
            "contrast-dark-white-processes",
            true,
            [255, 255, 255],
            false,
        ),
        ("contrast-light-black-processes", false, [0, 0, 0], false),
        ("contrast-dark-black-history", true, [0, 0, 0], true),
        ("contrast-light-yellow-history", false, [255, 255, 0], true),
        ("contrast-dark-white-theme", true, [255, 255, 255], false),
        ("contrast-light-yellow-theme", false, [255, 255, 0], false),
    ] {
        let settings = ThemeSettings {
            dark,
            accent: color,
            secondary: color,
            stops: theme::Stop::palette([color; 4]),
            gradient_strength: 0.75,
            frost: 0.45,
            hover_strength: 0.30,
            column_strength: 0.16,
            zebra_strength: 0.18,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.selected_pid = Some(900_001);
        app.inspector_visible = false;
        app.page = if history {
            Page::History
        } else {
            Page::Processes
        };
        app.show_theme_editor = name.ends_with("-theme");
        let size = Vec2::new(1280.0, 760.0);
        let mut output = egui::FullOutput::default();
        // Allow egui's window-opening fade to finish before auditing steady state.
        for _ in 0..20 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
}
