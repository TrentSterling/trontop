//! Responsive layout tests use local egui events, never native input or windows.
use super::*;

fn visible_text(output: &egui::FullOutput, label: &str) -> egui::Rect {
    let (text, clip) = text_shapes(output)
        .into_iter()
        .find(|(text, _)| text.galley.job.text == label)
        .unwrap_or_else(|| panic!("missing text: {label}"));
    let rect = text.visual_bounding_rect();
    assert!(
        clip.expand(0.5).contains_rect(rect),
        "clipped {label}: {rect:?} in {clip:?}"
    );
    assert!(!text.galley.elided, "truncated {label}");
    rect
}

#[test]
fn compact_layout_keeps_process_headers_and_actions_visible_without_inspector() {
    for dark in [true, false] {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for size in [Vec2::new(1040.0, 640.0), Vec2::new(1280.0, 760.0)] {
                let settings = ThemeSettings {
                    dark,
                    ..Default::default()
                };
                let ctx = egui::Context::default();
                theme::install(&ctx, settings);
                ctx.set_pixels_per_point(scale);
                let mut app = app(settings, true);
                // Empty inspector must not reserve any width, even when enabled.
                assert!(app.inspector_visible);
                assert!(app.selected_pid.is_none());
                for _ in 0..4 {
                    frame(&ctx, &mut app, size, vec![]);
                }
                let output = frame(&ctx, &mut app, size, vec![]);
                let mut header_y = None;
                for label in ["NAME", "PID", "CPU  v", "GPU", "MEMORY", "READ", "WRITE"] {
                    // GPU/MEMORY also occur in the rail. Find the table occurrence
                    // by its shared NAME header baseline before checking clipping.
                    let table_y =
                        *header_y.get_or_insert_with(|| visible_text(&output, "NAME").center().y);
                    let (text, clip) = text_shapes(&output)
                        .into_iter()
                        .find(|(text, _)| {
                            text.galley.job.text == label
                                && (text.visual_bounding_rect().center().y - table_y).abs() < 1.0
                        })
                        .unwrap_or_else(|| panic!("missing aligned table header {label}"));
                    assert!(
                        clip.expand(0.5).contains_rect(text.visual_bounding_rect()),
                        "clipped table {label} at {size:?}/{scale}"
                    );
                    assert!(!text.galley.elided);
                }
                for label in [
                    "Run task",
                    "End task",
                    "Inspector",
                    "Theme",
                    "About",
                    "Export",
                ] {
                    visible_text(&output, label);
                }
                assert!(
                    !text_shapes(&output)
                        .iter()
                        .any(|(text, _)| text.galley.job.text == "INSPECTOR")
                );
            }
        }
    }
}

#[test]
fn compact_layout_metric_values_and_footer_fit_with_or_without_inspector() {
    for dark in [true, false] {
        for size in [Vec2::new(1040.0, 640.0), Vec2::new(1280.0, 760.0)] {
            for selected in [false, true] {
                let settings = ThemeSettings {
                    dark,
                    ..Default::default()
                };
                let ctx = egui::Context::default();
                theme::install(&ctx, settings);
                let mut app = app(settings, true);
                app.selected_pid = selected.then_some(900_001);
                app.snapshot.uptime_seconds = 365 * 86_400 + 23 * 3600 + 59 * 60;
                for _ in 0..5 {
                    frame(&ctx, &mut app, size, vec![]);
                }
                let output = frame(&ctx, &mut app, size, vec![]);
                let uptime = visible_text(&output, "365d 23h 59m");
                let name = visible_text(&output, "NAME");
                assert!(uptime.bottom() < name.top(), "cards overlap table");
                let footer = format!(
                    "{} table rows | {} matching of {} processes",
                    app.visible_process_tree.len(),
                    app.visible_processes.len(),
                    app.snapshot.process_count
                );
                let footer = visible_text(&output, &footer);
                assert!(footer.bottom() < size.y - 4.0);
                visible_text(&output, "Inspector");
            }
        }
    }
}

#[test]
fn compact_layout_inspector_toggle_reclaims_width_without_losing_process_state() {
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let size = Vec2::new(1040.0, 640.0);
    let mut app = app(settings, true);
    app.selected_pid = Some(900_001);
    let original = app.selected_process().unwrap().identity();
    app.query = "Fixture".into();
    app.rebuild_visible_processes();
    let expansion = app.visible_process_tree.clone();
    click_local_text(&ctx, &mut app, size, "Inspector");
    assert!(!app.inspector_visible);
    let output = frame(&ctx, &mut app, size, vec![]);
    visible_text(&output, "WRITE");
    assert_eq!(app.selected_process().unwrap().identity(), original);
    assert_eq!(app.query, "Fixture");
    assert_eq!(app.visible_process_tree.len(), expansion.len());
    click_local_text(&ctx, &mut app, size, "Inspector");
    assert!(app.inspector_visible);
    let output = frame(&ctx, &mut app, size, vec![]);
    visible_text(&output, "INSPECTOR");
    assert_eq!(app.selected_process().unwrap().identity(), original);
    assert!(app.pending_end_task.is_none());
    assert!(app.pending_control_action.is_none());
}

#[test]
fn compact_layout_inspector_identity_has_fixed_height_for_long_names_and_accounts() {
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let size = Vec2::new(1040.0, 640.0);
        let mut app = app(settings, true);
        app.selected_pid = Some(900_001);
        let mut cpu_anchor = None;
        for (name, account) in [
            ("Editor.exe".into(), "TestAccount".into()),
            (
                "Very.Long.Executable.Name.Without.A.Break.exe".repeat(4),
                "DOMAIN\\LongAccountName".repeat(5),
            ),
            ("应用程序編集器".repeat(30), String::new()),
        ] {
            app.snapshot.processes[1].name = name;
            app.snapshot.processes[1].user = account;
            for _ in 0..4 {
                frame(&ctx, &mut app, size, vec![]);
            }
            let output = frame(&ctx, &mut app, size, vec![]);
            let inspector_left = visible_text(&output, "INSPECTOR").left();
            let (cpu, clip) = text_shapes(&output)
                .into_iter()
                .find(|(text, _)| {
                    text.galley.job.text == "CPU"
                        && text.visual_bounding_rect().left() >= inspector_left
                })
                .unwrap();
            let bounds = cpu.visual_bounding_rect();
            assert!(clip.contains_rect(bounds));
            if let Some(previous) = cpu_anchor {
                assert_eq!(bounds, previous);
            }
            cpu_anchor = Some(bounds);
            visible_text(&output, "Account");
            assert!(app.pending_end_task.is_none());
        }
    }
}
