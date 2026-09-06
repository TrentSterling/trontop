use super::*;

pub(super) fn install_groups(app: &mut TrontopApp) {
    let mut snapshot = fixture();
    snapshot.processes.truncate(6);
    for (index, (row, (name, value))) in snapshot
        .processes
        .iter_mut()
        .zip([
            ("Fixture.Render.exe", 1),
            ("Fixture.RenderWorker.exe", 19),
            ("Fixture.Tools.exe", 8),
            ("Fixture.ToolWorker.exe", 2),
            ("Fixture.Idle.exe", 5),
            ("Fixture.IdleWorker.exe", 0),
        ])
        .enumerate()
    {
        row.name = name.into();
        row.parent_pid = if index % 2 == 0 {
            None
        } else {
            Some(900_000 + index as u32 - 1)
        };
        row.cpu_percent = value as f32;
        row.gpu_percent = crate::gpu_activity::Usage::Measured(value as f32);
        row.memory_bytes = value * 1_048_576;
        row.read_bytes_per_sec = value as f64 * 1024.0;
        row.write_bytes_per_sec = value as f64 * 2048.0;
    }
    snapshot.processes[0].gpu_percent = crate::gpu_activity::Usage::Unreported;
    snapshot.process_count = snapshot.processes.len();
    app.page = Page::Processes;
    app.tree_mode = true;
    app.selected_pid = None;
    app.accept_sample(snapshot);
    for row in &app.snapshot.processes {
        app.tree_expansion.expand(row);
    }
    app.rebuild_visible_processes();
}

fn click_text(ctx: &egui::Context, app: &mut TrontopApp, size: Vec2, label: &str) {
    let output = frame(ctx, app, size, vec![]);
    let position = text_shapes(&output)
        .into_iter()
        .find(|(text, clip)| {
            text.galley.job.text == label && clip.contains_rect(text.visual_bounding_rect())
        })
        .unwrap_or_else(|| panic!("missing clickable label {label}"))
        .0
        .visual_bounding_rect()
        .center();
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            size,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

fn assert_root_order(ctx: &egui::Context, app: &mut TrontopApp, size: Vec2, expected: &[usize]) {
    let output = frame(ctx, app, size, vec![]);
    assert_eq!(
        app.visible_process_tree
            .iter()
            .filter(|row| row.depth == 0)
            .map(|row| row.process_index)
            .collect::<Vec<_>>(),
        expected
    );
    let texts = text_shapes(&output);
    let y = expected
        .iter()
        .map(|&index| {
            let name = format!("{}  [2]", app.snapshot.processes[index].name);
            texts
                .iter()
                .find(|(text, clip)| {
                    text.galley.job.text == name && clip.contains_rect(text.visual_bounding_rect())
                })
                .unwrap_or_else(|| panic!("missing visible sorted root {name}"))
                .0
                .pos
                .y
        })
        .collect::<Vec<_>>();
    assert!(
        y.windows(2).all(|pair| pair[0] < pair[1]),
        "paint order differs from sorted view"
    );
}

#[test]
fn headers_sort_displayed_groups_and_flat_switch_uses_individual_values() {
    let size = Vec2::new(1600.0, 900.0);
    for dark in [true, false] {
        for (column, label) in [
            (SortColumn::Cpu, "CPU"),
            (SortColumn::Gpu, "GPU"),
            (SortColumn::Memory, "MEMORY"),
            (SortColumn::ReadRate, "READ"),
            (SortColumn::WriteRate, "WRITE"),
        ] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut app = app(settings, false);
            install_groups(&mut app);
            app.sort_column = column;
            app.sort_direction = SortDirection::Descending;
            app.rebuild_visible_processes();
            for _ in 0..3 {
                frame(&ctx, &mut app, size, vec![]);
            }
            assert_root_order(&ctx, &mut app, size, &[0, 2, 4]);
            click_text(&ctx, &mut app, size, &format!("{label}  v"));
            assert_eq!(app.sort_direction, SortDirection::Ascending);
            assert_root_order(&ctx, &mut app, size, &[4, 2, 0]);
            click_text(&ctx, &mut app, size, &format!("{label}  ^"));
            assert_root_order(&ctx, &mut app, size, &[0, 2, 4]);
            click_text(&ctx, &mut app, size, "Flat list");
            assert!(!app.tree_mode);
            assert_eq!(app.visible_processes[0], 1); // Worker has the largest individual value.
            click_text(&ctx, &mut app, size, "Process tree");
            assert!(app.tree_mode);
            assert_root_order(&ctx, &mut app, size, &[0, 2, 4]);
        }
    }
}

fn press_local_key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn tab_to_text(
    ctx: &egui::Context,
    app: &mut TrontopApp,
    size: Vec2,
    label: &str,
) -> egui::FullOutput {
    // Only local egui events. Never activate another control while finding the
    // requested cell, and never execute native/platform commands from outputs.
    let mut output = egui::FullOutput::default();
    for _ in 0..120 {
        output.append(frame(ctx, app, size, vec![press_local_key(egui::Key::Tab)]));
        if let Some(response) = ctx
            .memory(|m| m.focused())
            .and_then(|id| ctx.read_response(id))
            && text_shapes(&output).iter().any(|(text, clip)| {
                text.galley.job.text == label
                    && clip.contains_rect(text.visual_bounding_rect())
                    && response.rect.contains(text.visual_bounding_rect().center())
            })
        {
            assert!(
                output.shapes.iter().any(|clipped| {
                    matches!(&clipped.shape, egui::Shape::Rect(shape)
                    if shape.rect == response.rect.expand2(Vec2::new(4.0, 0.0)).shrink(1.0) && shape.stroke.width >= 1.0)
                }),
                "focused table cell {label} has no visible outline in {:?}",
                response.rect
            );
            return output;
        }
    }
    panic!("Tab did not reach {label}");
}

#[test]
fn process_table_keyboard_focus_sort_and_selection_use_real_ui_without_native_input() {
    for dark in [true, false] {
        for (size, scale) in [
            (Vec2::new(1040.0, 640.0), 1.0),
            (Vec2::new(1280.0, 760.0), 1.0),
            (Vec2::new(1040.0, 640.0), 1.25),
            (Vec2::new(1280.0, 760.0), 1.5),
            (Vec2::new(1280.0, 760.0), 2.0),
        ] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            ctx.set_pixels_per_point(scale);
            let mut app = app(settings, false);
            install_groups(&mut app);
            app.sort_column = SortColumn::Cpu;
            app.sort_direction = SortDirection::Descending;
            app.rebuild_visible_processes();
            for _ in 0..3 {
                frame(&ctx, &mut app, size, vec![]);
            }
            tab_to_text(&ctx, &mut app, size, "CPU  v");
            frame(
                &ctx,
                &mut app,
                size,
                vec![press_local_key(egui::Key::Enter)],
            );
            assert_eq!(app.sort_direction, SortDirection::Ascending);
            assert_root_order(&ctx, &mut app, size, &[4, 2, 0]);
            tab_to_text(&ctx, &mut app, size, "Fixture.Idle.exe  [2]");
            frame(
                &ctx,
                &mut app,
                size,
                vec![press_local_key(egui::Key::Space)],
            );
            assert_eq!(app.selected_pid, Some(900_004));
            assert!(app.pending_end_task.is_none());
        }
    }
}

#[test]
fn process_table_row_padding_remains_mouse_clickable_without_duplicate_keyboard_target() {
    let size = Vec2::new(1280.0, 760.0);
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, false);
    install_groups(&mut app);
    let mut output = egui::FullOutput::default();
    for _ in 0..3 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let text = text_shapes(&output)
        .into_iter()
        .find(|(text, _)| text.galley.job.text == "Fixture.Render.exe  [2]")
        .unwrap()
        .0;
    // The row is 32 points tall; this targets its bottom inset, outside the
    // smaller child cell allocation, not the name, icon or expansion button.
    let pos = egui::pos2(
        text.pos.x + 4.0,
        text.visual_bounding_rect().center().y + 14.0,
    );
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut app,
            size,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(app.selected_pid, Some(900_000));
    assert!(app.pending_end_task.is_none());
}

#[test]
#[ignore = "offscreen keyboard-focus PNGs with fixture data, no native window or input"]
fn render_table_keyboard_focus_visual_pass() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke");
    std::fs::create_dir_all(&directory).unwrap();
    let mut renderer = offscreen::Renderer::new();
    for (name, dark, size, label) in [
        (
            "focus-header-dark",
            true,
            Vec2::new(1040.0, 640.0),
            "CPU  v",
        ),
        (
            "focus-header-light",
            false,
            Vec2::new(1280.0, 760.0),
            "CPU  v",
        ),
        ("focus-heat-dark", true, Vec2::new(1280.0, 760.0), "20.0%"),
        (
            "focus-name-light",
            false,
            Vec2::new(1040.0, 640.0),
            "Fixture.Render.exe  [2]",
        ),
    ] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, false);
        install_groups(&mut app);
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        output.append(tab_to_text(&ctx, &mut app, size, label));
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    println!("Keyboard focus: 4 offscreen PNGs; synthetic data, no native windows or OS input");
}

#[test]
fn refresh_search_and_pid_reuse_preserve_only_intended_expansion_state() {
    let mut app = app(ThemeSettings::default(), false);
    install_groups(&mut app);
    app.tree_expansion.toggle(&app.snapshot.processes[0]);
    let mut next = app.snapshot.clone();
    next.processes.reverse();
    app.accept_sample(next);
    assert!(!app.tree_expansion.pids().contains(&900_000));
    app.query = "Fixture.RenderWorker.exe".into();
    app.rebuild_visible_processes();
    assert_eq!(app.visible_process_tree.len(), 2);
    assert!(app.visible_process_tree[0].expanded); // Search context, not a stored change.
    assert!(!app.tree_expansion.pids().contains(&900_000));
    app.query.clear();
    app.rebuild_visible_processes();
    assert!(
        !app.visible_process_tree
            .iter()
            .find(|r| app.snapshot.processes[r.process_index].pid == 900_000)
            .unwrap()
            .expanded
    );

    let mut next = app.snapshot.clone();
    for row in &mut next.processes {
        if row.pid == 900_002 || row.pid == 900_003 {
            row.control.created_at_100ns = row.control.created_at_100ns.map(|time| time + 1000);
        }
    }
    app.accept_sample(next);
    assert!(!app.tree_expansion.pids().contains(&900_002));
    assert!(
        !app.visible_process_tree
            .iter()
            .find(|r| app.snapshot.processes[r.process_index].pid == 900_002)
            .unwrap()
            .expanded
    );
    assert!(app.tree_expansion.pids().contains(&900_004));
    app.accept_sample(SystemSnapshot::default());
    assert!(app.tree_expansion.pids().is_empty());
}

#[test]
fn switching_process_pages_never_leaves_an_invisible_sort_key() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), false);
    install_groups(&mut app);
    let size = Vec2::new(1600.0, 900.0);
    for page in [Page::Processes, Page::Details] {
        for column in [
            SortColumn::Name,
            SortColumn::Pid,
            SortColumn::Status,
            SortColumn::User,
            SortColumn::Cpu,
            SortColumn::Gpu,
            SortColumn::Memory,
            SortColumn::ReadRate,
            SortColumn::WriteRate,
            SortColumn::CpuTime,
        ] {
            app.page = page;
            app.sort_column = column;
            app.sort_direction = SortDirection::Ascending;
            app.rebuild_visible_processes();
            let mut output = egui::FullOutput::default();
            for _ in 0..3 {
                output = frame(&ctx, &mut app, size, vec![]);
            }
            let hidden = if page == Page::Details {
                column == SortColumn::WriteRate
            } else {
                matches!(
                    column,
                    SortColumn::Status | SortColumn::User | SortColumn::CpuTime
                )
            };
            assert_eq!(
                app.sort_column,
                if hidden { SortColumn::Cpu } else { column }
            );
            assert_eq!(
                app.sort_direction,
                if hidden {
                    SortDirection::Descending
                } else {
                    SortDirection::Ascending
                }
            );
            let marked_headers = text_shapes(&output)
                .into_iter()
                .filter(|(text, clip)| {
                    text.pos.y > 150.0
                        && clip.contains_rect(text.visual_bounding_rect())
                        && (text.galley.job.text.ends_with("  ^")
                            || text.galley.job.text.ends_with("  v"))
                })
                .count();
            assert_eq!(marked_headers, 1, "missing or ambiguous sort marker");
        }
    }
}
