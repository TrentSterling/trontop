//! Injected adapter metadata and local egui events only; no native actions.
use super::*;
use crate::specs::{Group, LiveKey, Row, Section, SectionId, SectionState};
use std::sync::Arc;

const INTERFACE: &str = "Fixture Ethernet adapter";

fn populated(settings: ThemeSettings) -> TrontopApp {
    let mut app = app(settings, true);
    app.page = Page::Performance;
    app.performance_device = PerformanceDevice::Network(0);
    let group = |interface: &str, speed: &str| {
        Group::new(interface)
            .live(LiveKey::NetworkThroughput {
                interface: interface.into(),
            })
            .row(Row::known("Adapter", "Fixture network hardware"))
            .row(Row::known("Type", "Ethernet"))
            .row(Row::known("Status", "Connected"))
            .row(Row::known("Link speed", speed))
            .row(Row::known("MTU", "1500 bytes"))
            .row(Row::known("IPv4", "PRIVATE-NETWORK-ADDRESS").private())
    };
    let entry = app
        .specs_view
        .entries
        .iter_mut()
        .find(|entry| entry.id == SectionId::Network)
        .unwrap();
    // Enumeration order is intentionally different from the live interfaces.
    entry.section = Some(Arc::new(
        Section::new(SectionId::Network)
            .group(group("Other fixture adapter", "100 Mbps"))
            .group(group(INTERFACE, "2.5 Gbps")),
    ));
    entry.health.state = SectionState::Complete;
    entry.health.collected_at = Some(std::time::Instant::now());
    app
}

fn settled(ctx: &egui::Context, app: &mut TrontopApp, size: Vec2) -> egui::FullOutput {
    let mut output = egui::FullOutput::default();
    for _ in 0..5 {
        output = frame(ctx, app, size, vec![]);
    }
    output
}

#[test]
fn network_performance_matches_inventory_keeps_cached_geometry_and_private_values_hidden() {
    for dark in [true, false] {
        for size in [Vec2::new(1000.0, 580.0), Vec2::new(1280.0, 800.0)] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut app = populated(settings);
            let mut baseline = Vec::new();
            for state in [
                SectionState::Complete,
                SectionState::Collecting,
                SectionState::Slow,
                SectionState::Unavailable,
            ] {
                app.specs_view
                    .entries
                    .iter_mut()
                    .find(|entry| entry.id == SectionId::Network)
                    .unwrap()
                    .health
                    .state = state;
                let output = settled(&ctx, &mut app, size);
                let texts = text_shapes(&output);
                let positions: Vec<_> = [
                    "Type",
                    "Status",
                    "Link speed",
                    "MTU",
                    "2.5 Gbps",
                    "1500 bytes",
                    "System details",
                ]
                .into_iter()
                .map(|label| {
                    let (text, clip) = texts
                        .iter()
                        .find(|(text, _)| text.galley.text() == label)
                        .unwrap_or_else(|| panic!("missing {label} at {size:?}/{state:?}"));
                    assert!(
                        clip.contains_rect(text.visual_bounding_rect()),
                        "clipped {label} at {size:?}/{state:?}"
                    );
                    text.pos
                })
                .collect();
                if state == SectionState::Complete {
                    baseline = positions;
                } else {
                    assert_eq!(positions, baseline, "state shifted adapter metadata");
                }
                assert!(!texts.iter().any(|(text, _)| {
                    ["100 Mbps", "PRIVATE-NETWORK-ADDRESS"].contains(&text.galley.text())
                }));
                if matches!(state, SectionState::Slow | SectionState::Unavailable) {
                    assert!(
                        texts
                            .iter()
                            .any(|(text, _)| text.galley.text() == "Adapter details / Cached")
                    );
                }
                assert!(app.specs.is_none() && app.sampler.is_none() && app.tray.is_none());
            }
        }
    }
}

#[test]
fn network_details_open_the_correct_system_section_and_never_borrow_another_adapter() {
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = populated(settings);
    let size = Vec2::new(1040.0, 640.0);
    click_local_text(&ctx, &mut app, size, "System details");
    assert_eq!(app.page, Page::System);
    assert_eq!(app.system_section, SectionId::Network);
    assert!(!app.reveal_private);
    app.page = Page::Performance;
    let entry = app
        .specs_view
        .entries
        .iter_mut()
        .find(|entry| entry.id == SectionId::Network)
        .unwrap();
    Arc::make_mut(entry.section.as_mut().unwrap()).groups.pop();
    let output = settled(&ctx, &mut app, size);
    assert!(
        text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.text() == "Not reported")
    );
    assert!(
        !text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.text() == "100 Mbps")
    );
    app.specs_view
        .entries
        .iter_mut()
        .find(|entry| entry.id == SectionId::Network)
        .unwrap()
        .section = None;
    let output = settled(&ctx, &mut app, size);
    assert!(
        text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.text() == "Reading")
    );
}

#[test]
fn performance_device_selection_follows_interface_and_volume_identity_through_refresh() {
    for network in [true, false] {
        let mut app = app(ThemeSettings::default(), true);
        if network {
            app.snapshot.networks.push(NetworkRow {
                name: "Second fixture interface".into(),
                ..Default::default()
            });
            app.performance_device = PerformanceDevice::Network(0);
        } else {
            app.performance_device = PerformanceDevice::Disk(0);
        }
        let mut changed = app.snapshot.clone();
        if network {
            changed.networks.swap(0, 1);
        } else {
            changed.disks.swap(0, 1);
        }
        changed.sequence += 1;
        app.accept_sample(changed);
        assert!(if network {
            app.performance_device == PerformanceDevice::Network(1)
        } else {
            app.performance_device == PerformanceDevice::Disk(1)
        });
        let mut removed = app.snapshot.clone();
        if network {
            removed.networks.remove(1);
        } else {
            removed.disks.remove(1);
        }
        removed.sequence += 1;
        app.accept_sample(removed);
        assert!(if network {
            app.performance_device == PerformanceDevice::Network(usize::MAX)
        } else {
            app.performance_device == PerformanceDevice::Disk(usize::MAX)
        });
        // A later refresh cannot silently select the replacement at that index.
        let mut next = app.snapshot.clone();
        next.sequence += 1;
        app.accept_sample(next);
        assert!(if network {
            app.performance_device == PerformanceDevice::Network(usize::MAX)
        } else {
            app.performance_device == PerformanceDevice::Disk(usize::MAX)
        });
    }
}

#[test]
#[ignore = "Offscreen network Performance fixtures; no native window, sampler or input"]
fn render_network_performance_visual_pass() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/ui-smoke/iteration-20260929/network-fixtures");
    std::fs::create_dir_all(&directory).unwrap();
    let mut renderer = offscreen::Renderer::new();
    for dark in [true, false] {
        for state in ["complete", "cached", "loading", "unmatched"] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut app = populated(settings);
            let entry = app
                .specs_view
                .entries
                .iter_mut()
                .find(|entry| entry.id == SectionId::Network)
                .unwrap();
            match state {
                "cached" => entry.health.state = SectionState::Slow,
                "loading" => {
                    entry.section = None;
                    entry.health.state = SectionState::Collecting;
                }
                "unmatched" => {
                    Arc::make_mut(entry.section.as_mut().unwrap()).groups.pop();
                }
                _ => {}
            }
            let size = Vec2::new(1000.0, 580.0);
            let mut output = egui::FullOutput::default();
            for _ in 0..6 {
                output.append(frame(&ctx, &mut app, size, vec![]));
            }
            renderer.save(
                &ctx,
                output,
                size,
                &directory.join(format!(
                    "network-{state}-{}.png",
                    if dark { "dark" } else { "light" }
                )),
            );
        }
    }
    println!("NETWORK_VISUAL: 8 synthetic fixture PNGs; no native desktop interaction");
}
