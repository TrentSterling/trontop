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
                for label in ["NAME", "PID", "CPU", "GPU", "MEMORY", "READ", "WRITE"] {
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
                // Compact command bars are icon-only; every action must still be
                // drawn fully inside the bar. Theme Studio lives in the sidebar.
                let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
                for label in ["Run task", "About", "Export"] {
                    let rect = command_rect(&ctx, label)
                        .unwrap_or_else(|| panic!("missing command {label}"));
                    assert!(
                        screen.contains_rect(rect) && rect.left() > 196.0 && rect.top() >= 42.0,
                        "command {label} misplaced at {size:?}/{scale}: {rect:?}"
                    );
                }
                // Nav sidebar is a fixed 196 px column; the command bar goes
                // compact below 1100 px of what remains. With no process
                // selected, End task and Inspector have nothing to act on and
                // disappear entirely there instead of sitting disabled.
                let compact = size.x - 196.0 < 1100.0;
                if compact {
                    assert!(command_rect(&ctx, "End task").is_none());
                    assert!(command_rect(&ctx, "Inspector").is_none());
                } else {
                    for label in ["End task", "Inspector"] {
                        let rect = command_rect(&ctx, label)
                            .unwrap_or_else(|| panic!("missing command {label}"));
                        assert!(
                            screen.contains_rect(rect) && rect.left() > 196.0 && rect.top() >= 42.0,
                            "command {label} misplaced at {size:?}/{scale}: {rect:?}"
                        );
                    }
                }
                assert!(command_rect(&ctx, "Theme").is_none());
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
                let toolbar = visible_text(&output, "Flat");
                let name = visible_text(&output, "NAME");
                assert!(toolbar.bottom() < name.top(), "toolbar overlaps table");
                // The process count is shown once: in the footer only.
                assert!(
                    !text_shapes(&output)
                        .iter()
                        .any(|(text, _)| text.galley.job.text
                            == format!("{} processes", app.snapshot.process_count)),
                    "process count repeated in the toolbar"
                );
                let footer = format!(
                    "{} processes \u{b7} {} rows",
                    app.snapshot.process_count,
                    app.visible_process_tree.len()
                );
                let footer = visible_text(&output, &footer);
                assert!(footer.bottom() < size.y - 4.0);
                // Below the compact breakpoint, Inspector only shows once a
                // process is selected; at full width it stays put (disabled).
                let compact = size.x - 196.0 < 1100.0;
                if compact && !selected {
                    assert!(command_rect(&ctx, "Inspector").is_none());
                } else {
                    assert!(command_rect(&ctx, "Inspector").is_some());
                }
            }
        }
    }
}

#[test]
fn processes_table_shows_at_least_eleven_rows_at_1000x580() {
    // Removing the CPU/MEMORY/GPU/UPTIME badge row and folding the view toggle
    // into one 30 px toolbar line should hand the table enough of a 580 px tall
    // window to clear 11 visible rows (32 px each, per process_table_inner).
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    app.page = Page::Processes;
    app.tree_mode = false;
    app.selected_pid = None;
    let size = Vec2::new(1000.0, 580.0);
    let mut output = egui::FullOutput::default();
    for _ in 0..5 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    // Every fixture PID is a distinct 6-digit number that appears nowhere else
    // on the page, so a fully unclipped one marks one visible table row.
    let visible_rows = text_shapes(&output)
        .into_iter()
        .filter(|(text, clip)| {
            let job = &text.galley.job.text;
            job.len() == 6
                && job.bytes().all(|b| b.is_ascii_digit())
                && clip.contains_rect(text.visual_bounding_rect())
        })
        .count();
    assert!(
        visible_rows >= 11,
        "expected at least 11 process rows visible at 1000x580, found {visible_rows}"
    );
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

pub(super) fn long_name() -> String {
    format!("{}.exe", "Fixture.Worker.LongCompilationProcess".repeat(6))
}

pub(super) fn render_cases(renderer: &mut offscreen::Renderer, directory: &std::path::Path) {
    for dark in [true, false] {
        for (kind, label) in [
            "end",
            "priority",
            "affinity",
            "priority-confirm",
            "affinity-confirm",
            "service-confirm",
            "history",
        ]
        .into_iter()
        .enumerate()
        {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut app = app(settings, true);
            app.page = Page::Processes;
            app.inspector_visible = false;
            app.process_actions =
                crate::process_actions::Controller::with_backend(ctx.clone(), |_| Ok(()));
            if kind == 6 {
                app.page = Page::History;
                for row in &mut app.snapshot.processes {
                    if row.pid % 2 == 0 {
                        row.name = long_name();
                    }
                    row.total_read_bytes = 17_426_700_000;
                }
            } else {
                if kind == 5 {
                    fixture_service_controls(&mut app, &ctx);
                }
                stage_dialog(&mut app, kind, &long_name());
            }
            let size = Vec2::new(1040.0, 640.0);
            let mut output = egui::FullOutput::default();
            for _ in 0..20 {
                output.append(frame(&ctx, &mut app, size, vec![]));
            }
            let mode = if dark { "dark" } else { "light" };
            renderer.save(
                &ctx,
                output,
                size,
                &directory.join(format!("compact-{label}-{mode}.png")),
            );
        }
    }
}

/// Real control windows, but no sampler or native command backend.
pub(super) fn stage_dialog(app: &mut TrontopApp, kind: usize, name: &str) -> &'static str {
    app.selected_pid = Some(900_001);
    app.snapshot.processes[1].name = name.into();
    let process = &mut app.snapshot.processes[1];
    process.control.system_affinity_mask = usize::MAX;
    process.control.affinity_mask = usize::MAX;
    let identity = process.identity().unwrap();
    match kind {
        0 => {
            app.pending_end_task = Some(PendingEndTask {
                identity,
                name: name.into(),
            });
            "End process"
        }
        1 => {
            app.show_priority_editor = true;
            "High"
        }
        2 => {
            app.show_affinity_editor = true;
            app.affinity_draft = usize::MAX >> 1;
            "Review change"
        }
        3 => {
            app.pending_control_action = Some(PendingControlAction::Priority {
                identity,
                priority: PriorityClass::High,
            });
            "Apply priority"
        }
        4 => {
            app.pending_control_action = Some(PendingControlAction::Affinity {
                identity,
                affinity_mask: 1,
            });
            "Apply affinity"
        }
        5 => {
            let service = &app.snapshot.services[0];
            app.pending_service = Some(crate::service_control::Request {
                name: service.name.clone(),
                display_name: name.into(),
                action: crate::service_control::Action::Stop,
                expected: service.status,
                staged_at: std::time::Instant::now(),
            });
            "Confirm command"
        }
        _ => unreachable!(),
    }
}

#[test]
fn compact_dialog_names_cannot_displace_or_clip_controls() {
    for dark in [true, false] {
        for size in [Vec2::new(1040.0, 640.0), Vec2::new(1280.0, 760.0)] {
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let settings = ThemeSettings {
                    dark,
                    ..Default::default()
                };
                let ctx = egui::Context::default();
                theme::install(&ctx, settings);
                ctx.set_pixels_per_point(scale);
                for kind in 0..6 {
                    let mut app = app(settings, false);
                    app.accept_sample(fixture());
                    let mut anchor = None;
                    for name in [
                        "Editor.exe".into(),
                        long_name(),
                        "\u{7f16}\u{8f91}\u{5668}".repeat(60),
                    ] {
                        let label = stage_dialog(&mut app, kind, &name);
                        let mut output = frame(&ctx, &mut app, size, vec![]);
                        for _ in 0..5 {
                            output = frame(&ctx, &mut app, size, vec![]);
                        }
                        let rect = visible_text(&output, label);
                        assert!(
                            egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(rect)
                        );
                        if let Some(previous) = anchor {
                            assert_eq!(
                                rect, previous,
                                "dialog {kind} action shifted with target name"
                            );
                        }
                        anchor = Some(rect);
                        assert!(!app.process_actions.busy());
                    }
                }
            }
        }
    }
}

#[test]
fn compact_affinity_all_processor_bits_keep_review_and_cancel_visible() {
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let size = Vec2::new(1040.0, 640.0);
    let mut app = app(settings, true);
    stage_dialog(&mut app, 2, "Editor.exe");
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..20 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    for label in ["Review change", "Cancel", "Select all", "Current mask"] {
        let rect = visible_text(&output, label);
        assert!(
            rect.bottom() < size.y - 8.0,
            "clipped footer: {label} {rect:?}"
        );
    }
}

fn scroll_local(
    ctx: &egui::Context,
    app: &mut TrontopApp,
    size: Vec2,
    position: egui::Pos2,
) -> egui::FullOutput {
    let mut output = frame(
        ctx,
        app,
        size,
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -1500.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..30 {
        output = frame(ctx, app, size, vec![]);
    }
    output
}

#[test]
fn compact_affinity_scroll_and_tiles_preserve_high_bit_review_and_zero_mask_guard() {
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let size = Vec2::new(1040.0, 640.0);
    let mut app = app(settings, true);
    stage_dialog(&mut app, 2, &long_name());
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..6 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let footer = visible_text(&output, "Review change");
    let position = visible_text(&output, "CPU 00").center();
    let output = scroll_local(&ctx, &mut app, size, position);
    assert_eq!(visible_text(&output, "Review change"), footer);
    let last = format!("CPU {:02}", usize::BITS - 1);
    visible_text(&output, &last);
    click_local_text(&ctx, &mut app, size, &last);
    assert_eq!(app.affinity_draft, usize::MAX);
    click_local_text(&ctx, &mut app, size, &last);
    assert_eq!(app.affinity_draft, usize::MAX >> 1);
    click_local_text(&ctx, &mut app, size, "Review change");
    match app.pending_control_action.take().unwrap() {
        PendingControlAction::Affinity {
            identity,
            affinity_mask,
        } => {
            assert_eq!(Some(identity), app.snapshot.processes[1].identity());
            assert_eq!(affinity_mask, usize::MAX >> 1);
        }
        _ => panic!("wrong control"),
    }
    assert!(!app.show_affinity_editor);
    assert!(!app.process_actions.busy());
    app.pending_control_action = Some(PendingControlAction::Affinity {
        identity: app.snapshot.processes[1].identity().unwrap(),
        affinity_mask: 1 | (1_usize << (usize::BITS - 1)),
    });
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..6 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    visible_text(&output, "Requested CPUs");
    visible_text(&output, &format!("0, {}", usize::BITS - 1));
    visible_text(&output, "2 selected in this group");
    click_local_text(&ctx, &mut app, size, "Cancel");
    assert!(app.pending_control_action.is_none());
    stage_dialog(&mut app, 2, &long_name());
    app.affinity_draft = 0;
    click_local_text(&ctx, &mut app, size, "Review change");
    assert!(app.pending_control_action.is_none());
    assert!(app.show_affinity_editor);
    click_local_text(&ctx, &mut app, size, "Current mask");
    assert_eq!(app.affinity_draft, usize::MAX);
    click_local_text(&ctx, &mut app, size, "Cancel");
    assert!(!app.show_affinity_editor);
}

#[test]
fn compact_history_long_names_keep_numeric_columns_aligned() {
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.page = Page::History;
        for (index, row) in app.snapshot.processes.iter_mut().enumerate() {
            row.name = if index % 2 == 0 {
                long_name()
            } else {
                "Worker.exe".into()
            };
            row.accumulated_cpu_millis = (10_000 + index as u64) * 3_600_000;
            row.total_read_bytes = 1_234_000_000_000_000;
        }
        app.rebuild_visible_processes();
        let size = Vec2::new(1040.0, 640.0);
        for _ in 0..5 {
            frame(&ctx, &mut app, size, vec![]);
        }
        // The taller, more legible section header leaves the last of 12 rows
        // just past the scroll area's edge; scroll it fully into view like a
        // real user would before checking column alignment.
        frame(
            &ctx,
            &mut app,
            size,
            vec![
                egui::Event::PointerMoved(egui::pos2(size.x * 0.5, size.y * 0.6)),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -400.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let mut output = frame(&ctx, &mut app, size, vec![]);
        for _ in 0..19 {
            output = frame(&ctx, &mut app, size, vec![]);
        }
        let mut right_edge = None;
        let mut previous_y: Option<f32> = None;
        for &index in &app.history_processes {
            let row = &app.snapshot.processes[index];
            let pid = visible_text(&output, &row.pid.to_string());
            let cpu = visible_text(&output, &format::millis(row.accumulated_cpu_millis));
            assert!(
                (pid.center().y - cpu.center().y).abs() < 1.0,
                "history baseline mismatch"
            );
            if let Some(right) = right_edge {
                assert_eq!(cpu.right(), right);
            }
            if let Some(y) = previous_y {
                assert!(
                    (cpu.center().y - y - 28.0_f32).abs() < 2.0,
                    "history rows must stay a uniform 28 px"
                );
            }
            right_edge = Some(cpu.right());
            previous_y = Some(cpu.center().y);
        }
    }
}

#[test]
fn compact_history_largest_counters_and_pid_are_not_truncated() {
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    app.page = Page::History;
    let row = &mut app.snapshot.processes[0];
    row.pid = u32::MAX;
    row.name = long_name();
    row.accumulated_cpu_millis = u64::MAX;
    row.total_read_bytes = u64::MAX;
    row.total_write_bytes = 1;
    app.rebuild_visible_processes();
    let size = Vec2::new(1040.0, 640.0);
    let mut output = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..5 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let pid = visible_text(&output, &u32::MAX.to_string());
    let cpu = visible_text(&output, &format::millis(u64::MAX));
    let io = visible_text(&output, &format::bytes(u64::MAX));
    assert!(pid.right() < cpu.left() && cpu.right() < io.left());
}

#[test]
fn compact_dialog_full_name_is_available_on_hover_without_moving_action() {
    let settings = ThemeSettings::default();
    let size = Vec2::new(1040.0, 640.0);
    let name = long_name();
    for kind in 0..6 {
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        ctx.global_style_mut(|style| style.interaction.tooltip_delay = 0.0);
        let mut app = app(settings, false);
        app.accept_sample(fixture());
        let action = stage_dialog(&mut app, kind, &name);
        let mut output = frame(&ctx, &mut app, size, vec![]);
        for _ in 0..6 {
            output = frame(&ctx, &mut app, size, vec![]);
        }
        let before = visible_text(&output, action);
        let (title, clip) = text_shapes(&output)
            .into_iter()
            .find(|(text, _)| {
                text.galley.job.text == name
                    && text.galley.job.sections[0].format.font_id.size == 17.0
            })
            .expect("dialog identity missing");
        assert!(title.galley.elided);
        assert_eq!(title.galley.rows.len(), 1);
        assert!(clip.contains_rect(title.visual_bounding_rect()));
        let pos = title.visual_bounding_rect().center();
        for _ in 0..6 {
            output = frame(&ctx, &mut app, size, vec![egui::Event::PointerMoved(pos)]);
        }
        assert!(
            text_shapes(&output)
                .into_iter()
                .any(|(text, clip)| text.galley.job.text == name
                    && !text.galley.elided
                    && clip.expand(0.5).contains_rect(text.visual_bounding_rect())),
            "full target missing from dialog {kind} hover"
        );
        assert_eq!(visible_text(&output, action), before);
        assert!(!app.process_actions.busy());
    }
}
