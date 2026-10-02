//! System specs page. Synthetic TEST DATA sections only: no specs workers,
//! native queries, windows or OS input.
use super::*;
use crate::specs::{SectionId, fixtures};

fn populated(settings: ThemeSettings) -> TrontopApp {
    let mut app = app(settings, true);
    app.page = Page::System;
    app.specs_view = fixtures::snapshot();
    app
}

fn render(ctx: &egui::Context, app: &mut TrontopApp, size: Vec2) -> egui::FullOutput {
    let mut output = frame(ctx, app, size, vec![]);
    for _ in 0..3 {
        output = frame(ctx, app, size, vec![]);
    }
    output
}

fn visible<'a>(output: &'a egui::FullOutput, text: &str) -> Option<&'a egui::epaint::TextShape> {
    text_shapes(output)
        .into_iter()
        .find(|(shape, clip)| {
            shape.galley.job.text == text && clip.contains_rect(shape.visual_bounding_rect())
        })
        .map(|(shape, _)| shape)
}

/// Laid out, possibly scrolled out of view.
fn present(output: &egui::FullOutput, text: &str) -> bool {
    text_shapes(output)
        .iter()
        .any(|(shape, _)| shape.galley.job.text == text)
}

#[test]
fn system_page_renders_every_section_with_live_values_and_masked_private_rows() {
    for dark in [true, false] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        for size in [
            Vec2::new(1040.0, 640.0),
            Vec2::new(1280.0, 760.0),
            Vec2::new(1920.0, 1080.0),
        ] {
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut app = populated(settings);
            for id in SectionId::ALL {
                app.system_section = id;
                let output = render(&ctx, &mut app, size);
                let texts = text_shapes(&output);
                assert!(visible(&output, "System").is_some(), "{id:?} heading");
                for nav in SectionId::ALL {
                    assert!(
                        texts
                            .iter()
                            .any(|(text, _)| text.galley.job.text == nav.title()
                                && text.pos.x > 196.0),
                        "missing sub-navigation entry {nav:?} on {id:?}"
                    );
                }
                for (text, _) in &texts {
                    let value = &text.galley.job.text;
                    assert!(
                        !value.contains(fixtures::PRIVATE_SERIAL)
                            && !value.contains(fixtures::DRIVE_INTERFACE),
                        "private fixture text rendered on {id:?}"
                    );
                    assert!(text.pos.is_finite() && text.galley.size().is_finite());
                }
                match id {
                    SectionId::Summary => {
                        // The CPU headline prefers the sampler's own load
                        // percentage (a real measurement) over a package
                        // temperature that many machines cannot supply.
                        let load_text = format::percent(app.snapshot.cpu_percent);
                        assert!(
                            texts
                                .iter()
                                .any(|(text, _)| text.galley.job.text.contains(&load_text)),
                            "CPU headline load percentage {load_text}"
                        );
                        assert!(
                            visible(
                                &output,
                                "Fixture processor with a long descriptive model name"
                            )
                            .is_some()
                        );
                        assert!(present(&output, "Fixture NVMe drive (test data), 2 TB"));
                        // Storage temperature resolves from the sampler's drive fixture.
                        assert!(present(&output, "44 °C"));
                        assert!(present(
                            &output,
                            "Reading now. Nothing is shown until the first read completes."
                        ));
                    }
                    SectionId::Cpu => {
                        assert!(visible(&output, "36 MB").is_some());
                        assert!(visible(&output, "Unavailable").is_some());
                        assert!(visible(&output, "88 °C").is_some());
                    }
                    SectionId::Motherboard => {
                        assert!(visible(&output, "Hidden").is_some());
                        assert!(visible(&output, "FX-900").is_some());
                    }
                    SectionId::Graphics => {
                        assert!(
                            visible(&output, "Fixture monitor EDID could not be read").is_some()
                        );
                        assert!(visible(&output, "2857 MHz").is_some());
                    }
                    _ => {}
                }
            }
            assert!(
                app.specs.is_none(),
                "headless tests must never start workers"
            );
        }
    }
}

#[test]
fn system_reveal_copy_and_sub_navigation_use_only_local_input() {
    let ctx = egui::Context::default();
    let mut app = populated(ThemeSettings::default());
    theme::install(&ctx, app.theme);
    let size = Vec2::new(1280.0, 900.0);
    click_local_text(&ctx, &mut app, size, "Motherboard");
    assert_eq!(app.system_section, SectionId::Motherboard);
    // The two privacy checkboxes live behind a "Privacy" toolbar dropdown now;
    // one click opens it and both checkbox toggles happen while it stays open.
    click_local_text(&ctx, &mut app, size, "Privacy");
    click_local_text(&ctx, &mut app, size, "Reveal private values");
    assert!(app.reveal_private);
    let output = render(&ctx, &mut app, size);
    assert!(visible(&output, fixtures::PRIVATE_SERIAL).is_some());
    click_local_text(&ctx, &mut app, size, "Reveal private values");
    assert!(!app.reveal_private);
    let output = render(&ctx, &mut app, size);
    let position = visible(&output, "Copy all")
        .expect("copy control")
        .visual_bounding_rect()
        .center();
    let event = |pressed| {
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    frame(&ctx, &mut app, size, event(true));
    // Only inspect the copy command. Never forward it to the real OS clipboard.
    let output = checked_frame(&ctx, &mut app, size, event(false), true);
    let [egui::OutputCommand::CopyText(report)] = output.platform_output.commands.as_slice() else {
        panic!("expected exactly one copy command");
    };
    assert!(report.contains("Private values: hidden"));
    assert!(report.contains("Serial number: [hidden]"));
    assert!(report.contains("Package temperature: 88 °C"));
    assert!(
        report.contains(
            "Package power: Unavailable (not exposed by Windows without a kernel driver)"
        )
    );
    assert!(!report.contains(fixtures::PRIVATE_SERIAL));
    assert!(!report.contains(fixtures::DRIVE_INTERFACE));
    assert!(app.specs.is_none());
}

#[test]
fn system_specs_save_uses_the_export_worker_with_private_values_off() {
    let (send, receive) = std::sync::mpsc::channel();
    let ctx = egui::Context::default();
    let mut app = populated(ThemeSettings::default());
    theme::install(&ctx, app.theme);
    app.exporter = crate::export::Exporter::with_backend(move |capture, _| {
        let _ = send.send((
            capture.options.format,
            capture.options.private_details,
            capture.specs.is_some(),
        ));
        crate::export::Outcome::Saved {
            path: std::path::PathBuf::from(r"C:\fixture\trontop-system-specs.txt"),
            bytes: 2048,
            sequence: capture.snapshot.sequence,
        }
    });
    let size = Vec2::new(1280.0, 900.0);
    // The privacy checkboxes live behind a "Privacy" toolbar dropdown now.
    click_local_text(&ctx, &mut app, size, "Privacy");
    click_local_text(&ctx, &mut app, size, "Private values in saved files");
    assert!(app.specs_export_private);
    click_local_text(&ctx, &mut app, size, "Save text");
    let (format, private, has_specs) = receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("save job");
    assert_eq!(format, crate::export::Format::SpecsText);
    assert!(private && has_specs);
    // Private values are opt-in per save.
    assert!(!app.specs_export_private);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.specs_export_pending && std::time::Instant::now() < deadline {
        render(&ctx, &mut app, size);
    }
    let (message, error) = app.message.clone().expect("save outcome message");
    assert!(
        message.starts_with("System specs saved:") && !error,
        "{message}"
    );
    click_local_text(&ctx, &mut app, size, "Save JSON");
    let (format, private, _) = receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("second save job");
    assert_eq!(format, crate::export::Format::SpecsJson);
    assert!(!private, "private values default off");
    assert!(app.specs.is_none());
}

#[test]
fn system_summary_and_sections_stay_inside_small_windows() {
    for size in [Vec2::new(900.0, 600.0), Vec2::new(1040.0, 640.0)] {
        let ctx = egui::Context::default();
        let mut app = populated(ThemeSettings::default());
        theme::install(&ctx, app.theme);
        for id in [SectionId::Summary, SectionId::Cpu, SectionId::Storage] {
            app.system_section = id;
            let output = render(&ctx, &mut app, size);
            for (shape, _) in text_shapes(&output) {
                let bounds = shape.visual_bounding_rect();
                assert!(
                    bounds.right() <= size.x + 0.5,
                    "{id:?} at {size:?}: {:?} overflows to {}",
                    shape.galley.job.text,
                    bounds.right()
                );
            }
            for label in ["Refresh", "Copy all", "Save text", "Save JSON"] {
                assert!(visible(&output, label).is_some(), "{label} at {size:?}");
            }
            // The left navigation's own entry for the active page, not the
            // page heading: fully inside its clip and above the stats footer.
            let nav = text_shapes(&output)
                .into_iter()
                .find(|(shape, _)| shape.galley.job.text == "System" && shape.pos.x < 196.0)
                .expect("System navigation entry");
            let bounds = nav.0.visual_bounding_rect();
            assert!(
                nav.1.contains_rect(bounds),
                "System nav entry clipped at {size:?}: {bounds:?} in {:?}",
                nav.1
            );
            let footer = visible(&output, "CPU").expect("footer CPU meter");
            assert!(
                bounds.bottom() < footer.visual_bounding_rect().top(),
                "System nav entry overlaps the footer at {size:?}"
            );
        }
    }
}

#[test]
#[ignore = "offscreen GPU visual QA with REAL read-only specs and sampler data; writes PNGs to TRONTOP_SPECS_SHOTS or target/ui-smoke/specs, never opens a window"]
fn render_system_specs_visual_pass() {
    let directory = std::env::var_os("TRONTOP_SPECS_SHOTS").map_or_else(
        || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke/specs"),
        std::path::PathBuf::from,
    );
    std::fs::create_dir_all(&directory).unwrap();
    // Real, read-only collection on this test thread and the sampler's workers.
    let specs = crate::specs::fixtures::collect_now();
    let sampler = crate::sampler::Sampler::spawn(egui::Context::default(), None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut snapshot = None;
    while std::time::Instant::now() < deadline {
        if let Some(latest) = sampler.latest_after(0) {
            let ready = latest.cpu.clocks.is_some()
                && latest
                    .storage_sensors
                    .drives
                    .iter()
                    .any(|d| d.last_success.is_some());
            snapshot = Some(latest);
            if ready {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let snapshot = snapshot.expect("sampler produced a snapshot");
    drop(sampler);
    println!(
        "{}",
        crate::specs::text(
            &specs,
            Some((&snapshot, &specs.bridge, std::time::Instant::now())),
            false
        )
    );
    let mut renderer = super::offscreen::Renderer::new();
    for (dark, size, section, name) in [
        (
            true,
            Vec2::new(1280.0, 900.0),
            SectionId::Summary,
            "summary",
        ),
        (true, Vec2::new(1280.0, 900.0), SectionId::Cpu, "cpu"),
        (true, Vec2::new(1280.0, 900.0), SectionId::Memory, "ram"),
        (
            true,
            Vec2::new(1280.0, 900.0),
            SectionId::Graphics,
            "graphics",
        ),
        (
            true,
            Vec2::new(1280.0, 900.0),
            SectionId::Storage,
            "storage",
        ),
        (
            false,
            Vec2::new(1280.0, 900.0),
            SectionId::Motherboard,
            "board-light",
        ),
        (
            true,
            Vec2::new(1040.0, 640.0),
            SectionId::Summary,
            "summary-small",
        ),
        (
            true,
            Vec2::new(1040.0, 640.0),
            SectionId::Storage,
            "storage-small",
        ),
        (
            true,
            Vec2::new(1280.0, 900.0),
            SectionId::SensorBridge,
            "sources",
        ),
        (
            true,
            Vec2::new(1280.0, 900.0),
            SectionId::OperatingSystem,
            "os",
        ),
    ] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, false);
        app.page = Page::System;
        app.snapshot = snapshot.clone();
        app.specs_view = specs.clone();
        app.system_section = section;
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        assert!(
            app.specs.is_none(),
            "the page never starts workers in tests"
        );
        renderer.save(
            &ctx,
            output,
            size,
            &directory.join(format!("system-real-{name}.png")),
        );
    }
    println!("PNGs written to {}", directory.display());
}

#[test]
#[ignore = "offscreen GPU visual QA; writes test-only PNGs under target/ui-smoke, never opens a window"]
fn render_system_specs_fixture_visual_pass() {
    let mut renderer = super::offscreen::Renderer::new();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke");
    std::fs::create_dir_all(&directory).unwrap();
    for (dark, size, section) in [
        (true, Vec2::new(1280.0, 760.0), SectionId::Summary),
        (true, Vec2::new(1280.0, 760.0), SectionId::Cpu),
        (false, Vec2::new(1280.0, 760.0), SectionId::Motherboard),
        (true, Vec2::new(1040.0, 640.0), SectionId::Graphics),
    ] {
        let settings = ThemeSettings {
            dark,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = populated(settings);
        app.system_section = section;
        let mut output = egui::FullOutput::default();
        for _ in 0..3 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(
            &ctx,
            output,
            size,
            &directory.join(format!(
                "system-{}-{}.png",
                section.key(),
                if dark { "dark" } else { "light" }
            )),
        );
    }
}

#[test]
fn complete_sections_show_no_state_chip_and_a_short_read_line() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = populated(settings);
    let complete = app
        .specs_view
        .get(SectionId::Cpu)
        .expect("cpu fixture")
        .health
        .clone();
    for entry in &mut app.specs_view.entries {
        if entry.section.is_none() {
            entry.section = Some(std::sync::Arc::new(crate::specs::Section::not_implemented(
                entry.id,
            )));
        }
        entry.health = complete.clone();
        entry.health.state = crate::specs::SectionState::Complete;
    }
    let size = Vec2::new(1000.0, 580.0);
    for id in SectionId::ALL {
        app.system_section = id;
        let output = render(&ctx, &mut app, size);
        for (text, _) in text_shapes(&output) {
            let value = text.galley.job.text.to_uppercase();
            assert!(
                !value.contains("COMPLETE"),
                "Complete state drawn on {id:?}: {}",
                text.galley.job.text
            );
            assert!(
                !text.galley.job.text.contains(" ms"),
                "read duration belongs in the hover on {id:?}"
            );
        }
        if id != SectionId::Summary {
            let line = text_shapes(&output)
                .into_iter()
                .find(|(text, _)| text.galley.job.text.starts_with("Read "))
                .unwrap_or_else(|| panic!("missing read line on {id:?}"))
                .0;
            assert!(
                line.galley.job.text.ends_with("ago") || line.galley.job.text == "Read just now"
            );
            assert!(line.galley.job.sections[0].format.font_id.size >= 11.0);
        }
    }
}

#[test]
fn expand_and_collapse_all_reach_nested_core_groups_and_survive_navigation() {
    let ctx = egui::Context::default();
    let mut app = populated(ThemeSettings::default());
    theme::install(&ctx, app.theme);
    let size = Vec2::new(1600.0, 1200.0);
    app.system_section = SectionId::Cpu;
    click_local_text(&ctx, &mut app, size, "Expand all");
    for _ in 0..20 {
        frame(&ctx, &mut app, size, vec![]);
    }
    let output = render(&ctx, &mut app, size);
    assert_eq!(
        text_shapes(&output)
            .iter()
            .filter(|(text, _)| text.galley.job.text == "Core type")
            .count(),
        4
    );
    click_local_text(&ctx, &mut app, size, "Collapse all");
    for _ in 0..20 {
        frame(&ctx, &mut app, size, vec![]);
    }
    let output = render(&ctx, &mut app, size);
    assert!(!present(&output, "Core type"));
    assert!(!present(&output, "Average clock"));
    click_local_text(&ctx, &mut app, size, "Motherboard");
    click_local_text(&ctx, &mut app, size, "CPU");
    let output = render(&ctx, &mut app, size);
    assert!(!present(&output, "Average clock"));
    click_local_text(&ctx, &mut app, size, "Expand all");
    for _ in 0..20 {
        frame(&ctx, &mut app, size, vec![]);
    }
    let output = render(&ctx, &mut app, size);
    assert_eq!(
        text_shapes(&output)
            .iter()
            .filter(|(text, _)| text.galley.job.text == "Core type")
            .count(),
        4
    );
    assert!(app.specs.is_none());
}

fn pointer_label(
    ctx: &egui::Context,
    app: &mut TrontopApp,
    size: Vec2,
    label: &str,
    button: egui::PointerButton,
) -> egui::FullOutput {
    let output = render(ctx, app, size);
    let position = visible(&output, label)
        .unwrap_or_else(|| panic!("missing {label}"))
        .visual_bounding_rect()
        .center();
    let mut output = egui::FullOutput::default();
    for pressed in [true, false] {
        output.append(checked_frame(
            ctx,
            app,
            size,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            true,
        ));
    }
    output
}

#[test]
fn row_context_copy_refuses_hidden_values_and_preserves_unavailable_reason() {
    let ctx = egui::Context::default();
    let mut app = populated(ThemeSettings::default());
    theme::install(&ctx, app.theme);
    let size = Vec2::new(1280.0, 900.0);
    app.system_section = SectionId::Motherboard;
    pointer_label(
        &ctx,
        &mut app,
        size,
        "Serial number",
        egui::PointerButton::Secondary,
    );
    let output = render(&ctx, &mut app, size);
    assert!(visible(&output, "Private value hidden").is_some());
    assert!(!present(&output, "Copy value"));
    assert!(!present(&output, "Copy row"));
    // An explicit local Escape closes the context menu before opening another.
    frame(
        &ctx,
        &mut app,
        size,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    pointer_label(
        &ctx,
        &mut app,
        size,
        "FX-900",
        egui::PointerButton::Secondary,
    );
    let output = pointer_label(
        &ctx,
        &mut app,
        size,
        "Copy row",
        egui::PointerButton::Primary,
    );
    let [egui::OutputCommand::CopyText(text)] = output.platform_output.commands.as_slice() else {
        panic!("expected row copy");
    };
    assert_eq!(text, "Model: FX-900");
    app.system_section = SectionId::Cpu;
    pointer_label(
        &ctx,
        &mut app,
        size,
        "Package power",
        egui::PointerButton::Secondary,
    );
    let output = pointer_label(
        &ctx,
        &mut app,
        size,
        "Copy value",
        egui::PointerButton::Primary,
    );
    let [egui::OutputCommand::CopyText(text)] = output.platform_output.commands.as_slice() else {
        panic!("expected unavailable reason copy");
    };
    assert_eq!(text, "not exposed by Windows without a kernel driver");
    assert!(app.specs.is_none());
}

#[test]
fn section_copy_and_group_copy_follow_explicit_private_reveal() {
    for reveal in [false, true] {
        let ctx = egui::Context::default();
        let mut app = populated(ThemeSettings::default());
        theme::install(&ctx, app.theme);
        let size = Vec2::new(1280.0, 900.0);
        app.system_section = SectionId::Motherboard;
        if reveal {
            click_local_text(&ctx, &mut app, size, "Privacy");
            click_local_text(&ctx, &mut app, size, "Reveal private values");
            click_local_text(&ctx, &mut app, size, "Privacy •");
        }
        let output = pointer_label(
            &ctx,
            &mut app,
            size,
            "Copy section",
            egui::PointerButton::Primary,
        );
        let [egui::OutputCommand::CopyText(text)] = output.platform_output.commands.as_slice()
        else {
            panic!("expected section copy");
        };
        assert_eq!(text.contains(fixtures::PRIVATE_SERIAL), reveal);
        assert!(text.contains("Model: FX-900"));
        pointer_label(
            &ctx,
            &mut app,
            size,
            "Baseboard",
            egui::PointerButton::Secondary,
        );
        let output = pointer_label(
            &ctx,
            &mut app,
            size,
            "Copy group",
            egui::PointerButton::Primary,
        );
        let [egui::OutputCommand::CopyText(text)] = output.platform_output.commands.as_slice()
        else {
            panic!("expected group copy");
        };
        assert_eq!(text.contains(fixtures::PRIVATE_SERIAL), reveal);
        assert!(text.contains("FX-900"));
        assert!(
            !text.contains("1.23"),
            "group copy must exclude sibling BIOS"
        );
    }
}

#[test]
fn system_missing_reads_and_aged_success_remain_explicit_without_estimates() {
    use crate::specs::SectionState;
    let ctx = egui::Context::default();
    let mut app = populated(ThemeSettings::default());
    theme::install(&ctx, app.theme);
    let size = Vec2::new(1280.0, 900.0);
    app.system_section = SectionId::Motherboard;
    for (state, expected) in [
        (SectionState::Waiting, "Waiting for the first read."),
        (
            SectionState::Collecting,
            "Reading now. Nothing is shown until the first read completes.",
        ),
        (
            SectionState::Slow,
            "The first read is slow. Nothing is estimated meanwhile.",
        ),
        (
            SectionState::Stopped,
            "The collection worker stopped before its first read.",
        ),
        (SectionState::Unavailable, "No data."),
    ] {
        let entry = app
            .specs_view
            .entries
            .iter_mut()
            .find(|e| e.id == SectionId::Motherboard)
            .unwrap();
        entry.section = None;
        entry.health.collected_at = None;
        entry.health.state = state;
        let output = render(&ctx, &mut app, size);
        assert!(present(&output, expected));
        assert!(!present(&output, "FX-900"));
        assert!(output.platform_output.commands.is_empty());
    }
    let mut app = populated(ThemeSettings::default());
    app.system_section = SectionId::Motherboard;
    for (seconds, expected) in [(22, "Read 22 s ago"), (120, "Read 2 min ago")] {
        let entry = app
            .specs_view
            .entries
            .iter_mut()
            .find(|e| e.id == SectionId::Motherboard)
            .unwrap();
        entry.health.collected_at =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(seconds));
        let output = render(&ctx, &mut app, size);
        assert!(present(&output, expected));
    }
}

#[test]
fn private_summary_masks_both_bridge_values_and_cpu_headline_override_until_revealed() {
    use crate::specs::{BridgeReading, LiveKey, LiveUnit, Section, SummaryLine, Value};
    use std::sync::Arc;
    use std::time::Instant;
    for id in [SectionId::Motherboard, SectionId::Cpu] {
        let ctx = egui::Context::default();
        let mut app = populated(ThemeSettings::default());
        app.snapshot.cpu_percent = 37.2;
        theme::install(&ctx, app.theme);
        ctx.global_style_mut(|s| s.interaction.tooltip_delay = 0.0);
        let now = Instant::now();
        app.graphs.fixed_now = Some(now);
        let key = LiveKey::Sensor {
            id: "PRIVATE-UI-FIXTURE-ID".into(),
        };
        let entry = app
            .specs_view
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap();
        entry.section = Some(Arc::new(Section::new(id).summary_line(SummaryLine {
            text: Value::known("PRIVATE-UI-FIXTURE-HEADLINE"),
            live: Some(key.clone()),
            private: true,
        })));
        let bridge = Arc::make_mut(&mut app.specs_view.bridge);
        bridge.status = Value::known("Fixture bridge");
        bridge.collected_at = Some(now);
        bridge.readings.push(BridgeReading {
            key,
            label: "PRIVATE-UI-FIXTURE-LABEL".into(),
            source: "PRIVATE-UI-FIXTURE-SOURCE".into(),
            value: 987.25,
            unit: LiveUnit::Watts,
        });
        let size = Vec2::new(1600.0, 1200.0);
        for section in [SectionId::Summary, id] {
            app.system_section = section;
            app.reveal_private = false;
            let output = render(&ctx, &mut app, size);
            let hidden = visible(&output, "Hidden")
                .expect("masked summary stays visible")
                .visual_bounding_rect()
                .center();
            for (shape, _) in text_shapes(&output) {
                assert!(
                    !shape.galley.job.text.contains("PRIVATE-UI-FIXTURE")
                        && !shape.galley.job.text.contains("987.2"),
                    "private summary leaked: {}",
                    shape.galley.job.text
                );
            }
            if section == SectionId::Summary && id == SectionId::Cpu {
                assert!(
                    !text_shapes(&output).iter().any(|(s, _)| s
                        .galley
                        .job
                        .text
                        .starts_with("37.2%")
                        && s.visual_bounding_rect().center().x > hidden.x
                        && (s.visual_bounding_rect().center().y - hidden.y).abs() < 4.0),
                    "CPU load override bypassed the private summary gate"
                );
            }
            frame(
                &ctx,
                &mut app,
                size,
                vec![egui::Event::PointerMoved(hidden)],
            );
            let output = render(&ctx, &mut app, size);
            assert!(present(
                &output,
                "Private value hidden. Turn on \"Reveal private values\" at the top of this page to show it."
            ));
            for (shape, _) in text_shapes(&output) {
                assert!(
                    !shape.galley.job.text.contains("PRIVATE-UI-FIXTURE")
                        && !shape.galley.job.text.contains("987.2"),
                    "private hover leaked: {}",
                    shape.galley.job.text
                );
            }
            frame(&ctx, &mut app, size, vec![egui::Event::PointerGone]);
            app.reveal_private = true;
            let output = render(&ctx, &mut app, size);
            assert!(present(&output, "PRIVATE-UI-FIXTURE-HEADLINE"));
            if section != SectionId::Summary || id != SectionId::Cpu {
                assert!(present(&output, "987.2 W"));
            }
        }
    }
}

#[test]
fn system_live_cells_and_hover_distinguish_retained_clocks_commit_and_gpu_temperature() {
    use crate::diagnostics::{Provider, State};
    use crate::specs::{GpuMetric, GpuRef, LiveKey};
    use std::time::{Duration, Instant};
    for dark in [true, false] {
        let ctx = egui::Context::default();
        let mut app = populated(ThemeSettings {
            dark,
            ..Default::default()
        });
        app.snapshot.cpu_percent = 37.2;
        theme::install(&ctx, app.theme);
        let now = Instant::now();
        app.graphs.fixed_now = Some(now);
        let memory = app
            .specs_view
            .entries
            .iter_mut()
            .find(|e| e.id == SectionId::Memory)
            .unwrap();
        memory.section = Some(std::sync::Arc::new(
            crate::specs::Section::new(SectionId::Memory).group(
                crate::specs::Group::new("Windows memory counters")
                    .row(crate::specs::Row::live("Commit", LiveKey::MemoryCommit)),
            ),
        ));
        app.snapshot.gpu_sensors.last_success = Some(now - Duration::from_secs(4));
        app.snapshot.gpu_sensors.using_cached = false;
        for provider in [Provider::CpuClock, Provider::MemoryCounters] {
            app.snapshot.diagnostics.get_mut(provider).record(
                now,
                Duration::ZERO,
                State::Stale,
                None,
                None,
            );
        }
        let size = Vec2::new(1280.0, 900.0);
        for (id, key) in [
            (SectionId::Cpu, LiveKey::CpuClockAverage),
            (SectionId::Memory, LiveKey::MemoryCommit),
            (
                SectionId::Graphics,
                LiveKey::Gpu {
                    adapter: GpuRef {
                        name: fixtures::GPU_NAME.into(),
                        ordinal: 0,
                    },
                    metric: GpuMetric::Temperature,
                },
            ),
        ] {
            let (text, color, hover) = app.live_parts(&key, now, app.colors());
            assert!(text.ends_with("(cached)"), "{key:?}: {text}");
            assert_eq!(
                color,
                app.colors().text,
                "retained temperature has no fresh band color"
            );
            assert!(hover.starts_with("Cached:"), "{hover}");
            app.system_section = id;
            let output = render(&ctx, &mut app, size);
            assert!(present(&output, &text), "missing cached cell {text}");
        }
        app.system_section = SectionId::Summary;
        let output = render(&ctx, &mut app, size);
        assert!(
            text_shapes(&output)
                .iter()
                .any(|(s, _)| s.galley.job.text.starts_with("37.2%")
                    && s.galley.job.text.ends_with("(cached)"))
        );
        for provider in [Provider::CpuClock, Provider::MemoryCounters] {
            app.snapshot.diagnostics.get_mut(provider).record(
                now,
                Duration::ZERO,
                State::Live,
                None,
                None,
            );
        }
        app.snapshot.gpu_sensors.last_success = Some(now);
        let (_, color, hover) = app.live_parts(
            &LiveKey::Gpu {
                adapter: GpuRef {
                    name: fixtures::GPU_NAME.into(),
                    ordinal: 0,
                },
                metric: GpuMetric::Temperature,
            },
            now,
            app.colors(),
        );
        assert_ne!(color, app.colors().text);
        assert!(hover.starts_with("Live:") && !hover.contains("cached"));
    }
}

#[test]
#[ignore = "offscreen fixture-only System freshness review; no native window or OS input"]
fn render_system_live_freshness_review() {
    use crate::diagnostics::{Provider, State};
    use crate::specs::{Group, LiveKey, Row, Section};
    use std::time::{Duration, Instant};
    let mut renderer = super::offscreen::Renderer::new();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/ui-smoke/system-live-alpha48");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, dark, section) in [
        ("summary-cached-dark", true, SectionId::Summary),
        ("graphics-cached-light", false, SectionId::Graphics),
        ("memory-cached-dark", true, SectionId::Memory),
    ] {
        let ctx = egui::Context::default();
        let mut app = populated(ThemeSettings {
            dark,
            ..Default::default()
        });
        theme::install(&ctx, app.theme);
        let now = Instant::now();
        app.graphs.fixed_now = Some(now);
        app.system_section = section;
        app.snapshot.cpu_percent = 37.2;
        app.snapshot.gpu_sensors.last_success = Some(now - Duration::from_secs(4));
        app.snapshot.gpu_sensors.using_cached = false;
        for provider in [Provider::CpuClock, Provider::MemoryCounters] {
            app.snapshot.diagnostics.get_mut(provider).record(
                now,
                Duration::ZERO,
                State::Stale,
                None,
                None,
            );
        }
        let memory = app
            .specs_view
            .entries
            .iter_mut()
            .find(|e| e.id == SectionId::Memory)
            .unwrap();
        memory.section = Some(std::sync::Arc::new(
            Section::new(SectionId::Memory)
                .summary_line(
                    crate::specs::SummaryLine::known("Fixture RAM, 64 GB (test data)")
                        .live(LiveKey::MemoryUsed),
                )
                .group(
                    Group::new("Windows memory counters (test data)")
                        .row(Row::live("Physical memory", LiveKey::MemoryUsed))
                        .row(Row::live("Commit charge", LiveKey::MemoryCommit)),
                ),
        ));
        let size = Vec2::new(1280.0, 900.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..8 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        assert!(
            text_shapes(&output).iter().any(|(text, clip)| text
                .galley
                .job
                .text
                .ends_with("(cached)")
                && clip.contains_rect(text.visual_bounding_rect()))
        );
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    println!(
        "SYSTEM_LIVE: 3 offscreen fixture-only views in {}",
        directory.display()
    );
}
