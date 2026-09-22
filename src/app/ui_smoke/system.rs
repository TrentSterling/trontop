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
            let t = app.colors();
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
                        let hot = visible(&output, "88 °C").expect("bridge CPU temperature");
                        assert_eq!(hot.galley.job.sections[0].format.color, t.ink(t.danger));
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
