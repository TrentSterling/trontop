//! Local egui input and offscreen rendering only. No native input or actions.
use super::*;

fn chrome(ctx: &egui::Context, title: &str) -> (egui::Rect, egui::Id, egui::Rect) {
    ctx.data(|data| data.get_temp(egui::Id::new(("dialog-chrome", title))))
        .unwrap_or_else(|| panic!("Missing chrome for {title}"))
}

fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

fn stage(app: &mut TrontopApp, kind: usize) -> &'static str {
    match kind {
        0 => {
            app.show_diagnostics = true;
            "About Trontop"
        }
        1 => {
            app.show_theme_editor = true;
            "Theme Studio"
        }
        2 => {
            app.show_export = true;
            "Export snapshot"
        }
        3 => {
            app.show_run_task = true;
            "Run new task"
        }
        _ => {
            compact_layout::stage_dialog(app, kind - 4, "Fixture.Editor.exe");
            [
                "Confirm end task",
                "Process priority",
                "CPU affinity",
                "Confirm process control",
                "Confirm process control",
                "Confirm service command",
            ][kind - 4]
        }
    }
}

fn is_open(app: &TrontopApp, kind: usize) -> bool {
    match kind {
        0 => app.show_diagnostics,
        1 => app.show_theme_editor,
        2 => app.show_export,
        3 => app.show_run_task,
        4 => app.pending_end_task.is_some(),
        5 => app.show_priority_editor,
        6 => app.show_affinity_editor,
        7 | 8 => app.pending_control_action.is_some(),
        _ => app.pending_service.is_some(),
    }
}

#[test]
fn polish_every_dialog_moves_from_its_header_and_close_cancels_without_an_action() {
    let size = Vec2::new(1280.0, 900.0);
    for kind in 0..10 {
        let settings = ThemeSettings::default();
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        let title = stage(&mut app, kind);
        for _ in 0..6 {
            frame(&ctx, &mut app, size, vec![]);
        }
        let (before, _, _) = chrome(&ctx, title);
        let start = before.center() - Vec2::new(40.0, 0.0);
        frame(&ctx, &mut app, size, pointer(start, true));
        let delta = Vec2::new(86.0, 44.0);
        frame(
            &ctx,
            &mut app,
            size,
            vec![egui::Event::PointerMoved(start + delta)],
        );
        frame(&ctx, &mut app, size, pointer(start + delta, false));
        for _ in 0..3 {
            frame(&ctx, &mut app, size, vec![]);
        }
        let (moved, _, close) = chrome(&ctx, title);
        assert!(
            (moved.min - before.min - delta).length() < 2.0,
            "{title} did not follow header drag: {before:?} -> {moved:?}"
        );
        assert!(is_open(&app, kind));
        frame(&ctx, &mut app, size, pointer(close.center(), true));
        frame(&ctx, &mut app, size, pointer(close.center(), false));
        assert!(!is_open(&app, kind), "{title} did not close");
        assert!(!app.process_actions.busy());
        assert!(!app.service_controller.busy());
        stage(&mut app, kind);
        for _ in 0..3 {
            frame(&ctx, &mut app, size, vec![]);
        }
        assert!(
            (chrome(&ctx, title).0.min - moved.min).length() < 1.0,
            "{title} lost its position on reopen"
        );
    }
}

#[test]
fn polish_close_outline_has_contrast_and_keyboard_activation() {
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            accent: [0, 0, 0],
            secondary: [255, 255, 255],
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.show_diagnostics = true;
        let size = Vec2::new(1000.0, 580.0);
        for _ in 0..5 {
            frame(&ctx, &mut app, size, vec![]);
        }
        let (_, id, close) = chrome(&ctx, "About Trontop");
        let output = frame(
            &ctx,
            &mut app,
            size,
            vec![egui::Event::PointerMoved(close.center())],
        );
        let stroke = output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect) if rect.rect == close && rect.stroke.width == 1.5 => {
                    Some(rect.stroke)
                }
                _ => None,
            })
            .expect("hover outline missing");
        assert!(
            theme::contrast_ratio(
                stroke.color,
                ctx.global_style().visuals.widgets.hovered.weak_bg_fill
            ) >= 4.5
        );
        ctx.memory_mut(|memory| memory.request_focus(id));
        let output = frame(&ctx, &mut app, size, vec![egui::Event::PointerGone]);
        assert!(output.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == close && r.stroke.width == 1.5)
        ));
        frame(
            &ctx,
            &mut app,
            size,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!app.show_diagnostics, "focused X ignored Enter");
    }
}

#[test]
fn polish_chart_choice_is_shared_and_survives_ui_memory_roundtrip() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = graphs::populated(settings);
    let size = Vec2::new(1000.0, 580.0);
    let key = egui::Id::new(widgets::CHART_BARS_KEY);
    for page in [
        Page::Overview,
        Page::Graphs,
        Page::Performance,
        Page::Sensors,
    ] {
        app.page = page;
        click_local_text(&ctx, &mut app, size, "Bars");
        assert_eq!(ctx.data_mut(|d| d.get_persisted::<bool>(key)), Some(true));
        click_local_text(&ctx, &mut app, size, "Lines");
        assert_eq!(ctx.data_mut(|d| d.get_persisted::<bool>(key)), Some(false));
    }
    click_local_text(&ctx, &mut app, size, "Bars");
    let encoded = ctx.memory(|m| ron::to_string(m).unwrap());
    let restored = egui::Context::default();
    restored.memory_mut(|m| *m = ron::from_str(&encoded).unwrap());
    assert_eq!(
        restored.data_mut(|d| d.get_persisted::<bool>(key)),
        Some(true)
    );
    app.page = Page::Overview;
    frame(&restored, &mut app, size, vec![]);
    assert_eq!(
        restored.data_mut(|d| d.get_persisted::<bool>(key)),
        Some(true)
    );
}

#[test]
fn polish_bar_history_preserves_gaps_and_zero_samples() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    ctx.data_mut(|d| d.insert_persisted(egui::Id::new(widgets::CHART_BARS_KEY), true));
    let rect = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), Vec2::new(100.0, 30.0));
    let output = ctx.run_ui(Default::default(), |ui| {
        widgets::sparkline_range(
            ui.painter(),
            rect,
            [Some(5.0), None, Some(0.0), Some(f32::NAN), Some(10.0)].into_iter(),
            0.0,
            10.0,
            Color32::WHITE,
            theme::tokens(settings),
        );
    });
    let bars: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Rect(r) if rect.contains_rect(r.rect) => Some(r.rect),
            _ => None,
        })
        .collect();
    assert_eq!(bars.len(), 3, "missing/NaN samples must remain gaps");
    assert_eq!(bars[0].height(), 15.0);
    assert_eq!(bars[1].height(), 1.0, "a measured zero must remain visible");
    assert_eq!(bars[2].height(), 30.0);
    assert!(bars[1].left() - bars[0].right() > 20.0);
    assert!(bars[2].left() - bars[1].right() > 20.0);
}

#[test]
fn polish_inspector_end_task_stays_above_scroll_and_requires_confirmation() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    app.page = Page::Processes;
    app.selected_pid = Some(900_001);
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    app.process_actions =
        crate::process_actions::Controller::with_backend(ctx.clone(), move |_| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        });
    let size = Vec2::new(1040.0, 640.0);
    let end_rect = |out: &egui::FullOutput| {
        text_shapes(out)
            .into_iter()
            .filter(|(t, clip)| {
                t.galley.job.text == "End task" && clip.contains_rect(t.visual_bounding_rect())
            })
            .map(|(t, _)| t.visual_bounding_rect())
            .max_by(|a, b| a.center().y.total_cmp(&b.center().y))
            .unwrap()
    };
    let mut out = frame(&ctx, &mut app, size, vec![]);
    for _ in 0..5 {
        out = frame(&ctx, &mut app, size, vec![]);
    }
    let before = end_rect(&out);
    assert!(before.bottom() < 280.0, "End task is buried");
    let at = egui::pos2(size.x - 100.0, size.y - 100.0);
    for _ in 0..10 {
        out = frame(
            &ctx,
            &mut app,
            size,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -400.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(end_rect(&out), before, "End task scrolled away");
    frame(&ctx, &mut app, size, pointer(before.center(), true));
    frame(&ctx, &mut app, size, pointer(before.center(), false));
    assert!(app.pending_end_task.is_some());
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "End task bypassed confirmation"
    );
}

#[test]
#[ignore = "offscreen polish review; no native windows, OS input or actions"]
fn render_polish_visual_pass() {
    let mut renderer = offscreen::Renderer::new();
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke/polish-alpha40");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, dark, kind, bars) in [
        ("about-dark-hover", true, Some(0), false),
        ("about-light-hover", false, Some(0), false),
        ("theme-dark", true, Some(1), false),
        ("theme-light", false, Some(1), false),
        ("export-dark", true, Some(2), false),
        ("run-light", false, Some(3), false),
        ("end-dark", true, Some(4), false),
        ("priority-light", false, Some(5), false),
        ("affinity-dark", true, Some(6), false),
        ("service-light", false, Some(9), false),
        ("overview-lines", true, None, false),
        ("overview-bars", true, None, true),
        ("overview-light-bars", false, None, true),
        ("inspector", true, None, false),
    ] {
        let settings = ThemeSettings {
            dark,
            gradient_strength: 1.0,
            frost: 0.12,
            frost_light: 0.12,
            surface_tint: 0.2,
            text_strength: 1.0,
            ..ThemeSettings::monke_portal()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        ctx.data_mut(|d| d.insert_persisted(egui::Id::new(widgets::CHART_BARS_KEY), bars));
        let mut app = graphs::populated(settings);
        app.page = Page::Overview;
        if let Some(kind) = kind {
            stage(&mut app, kind);
        }
        if name == "inspector" {
            app.page = Page::Processes;
            app.selected_pid = Some(900_001);
            app.inspector_visible = true;
            app.process_actions =
                crate::process_actions::Controller::with_backend(ctx.clone(), |_| Ok(()));
        }
        let size = Vec2::new(1040.0, 640.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..12 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        if name.ends_with("hover") {
            let close = chrome(&ctx, "About Trontop").2;
            output.append(frame(
                &ctx,
                &mut app,
                size,
                vec![egui::Event::PointerMoved(close.center())],
            ));
        }
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    render_logo_palettes(&mut renderer, &directory);
    println!("Polish: 16 offscreen PNGs in {}", directory.display());
}

fn render_logo_palettes(renderer: &mut offscreen::Renderer, directory: &std::path::Path) {
    for dark in [true, false] {
        let ctx = egui::Context::default();
        theme::install(
            &ctx,
            ThemeSettings {
                dark,
                ..Default::default()
            },
        );
        let size = Vec2::new(960.0, 740.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output.append(ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    ..Default::default()
                },
                |ui| {
                    for row in 0..7 {
                        ui.horizontal(|ui| {
                            for column in 0..2 {
                                let index = row * 2 + column;
                                let mut settings = ThemeSettings {
                                    dark,
                                    frost: 0.0,
                                    frost_light: 0.0,
                                    gradient_strength: 1.0,
                                    ..Default::default()
                                };
                                let label = if index < 2 {
                                    settings.stops = theme::Stop::palette(
                                        [[if index == 0 { 0 } else { 255 }; 3]; 4],
                                    );
                                    if index == 0 {
                                        "Black palette".to_owned()
                                    } else {
                                        "White palette".to_owned()
                                    }
                                } else {
                                    let flavor = theme::magic::Flavor::ALL
                                        [(index - 2) % theme::magic::Flavor::ALL.len()];
                                    theme::magic::randomize(
                                        &mut settings,
                                        index as u64 * 83,
                                        flavor,
                                    );
                                    format!("{} / seed {}", flavor.label(), index * 83)
                                };
                                let (rect, _) =
                                    ui.allocate_exact_size(Vec2::new(462.0, 94.0), Sense::hover());
                                theme::paint_gradient(
                                    ui.painter(),
                                    rect,
                                    settings.stops,
                                    settings.gradient_angle,
                                    |color| theme::composed_panel(settings, color),
                                );
                                let t = theme::tokens(settings);
                                ui.scope_builder(
                                    egui::UiBuilder::new()
                                        .max_rect(rect.shrink(12.0))
                                        .layout(Layout::left_to_right(Align::Center)),
                                    |ui| {
                                        widgets::tront_mark(ui, settings, 64.0);
                                        widgets::tront_mark(ui, settings, 32.0);
                                        widgets::tront_mark(ui, settings, 20.0);
                                        ui.label(RichText::new(label).color(t.text).size(14.0));
                                    },
                                );
                            }
                        });
                    }
                },
            ));
        }
        renderer.save(
            &ctx,
            output,
            size,
            &directory.join(if dark {
                "logo-palettes-dark.png"
            } else {
                "logo-palettes-light.png"
            }),
        );
    }
}
