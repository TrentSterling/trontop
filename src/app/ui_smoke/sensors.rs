//! Hardware sensor freshness through production UI, no native workers or input.
use super::*;
use crate::specs::{BridgeReading, BridgeReadings, LiveKey, LiveUnit, Value, fixtures};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn setup() -> (egui::Context, TrontopApp) {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    theme::install(&ctx, app.theme);
    app.page = Page::Sensors;
    app.specs_view = fixtures::snapshot();
    app.snapshot.storage_sensors = Arc::new(Default::default());
    app.specs_view.bridge = Arc::new(BridgeReadings {
        readings: vec![
            BridgeReading {
                key: LiveKey::CpuCoreTemperature { index: 3 },
                label: "Fixture core 3".into(),
                value: 83.0,
                unit: LiveUnit::Celsius,
                source: "Fixture provider".into(),
            },
            BridgeReading {
                key: LiveKey::CpuCoreTemperature { index: 1 },
                label: "Fixture core 1".into(),
                value: 51.0,
                unit: LiveUnit::Celsius,
                source: "Fixture provider".into(),
            },
        ],
        status: Value::known("Fixture provider"),
        collected_at: Some(Instant::now() + Duration::from_secs(3600)),
        retry_after: None,
    });
    (ctx, app)
}
fn render(ctx: &egui::Context, app: &mut TrontopApp) -> egui::FullOutput {
    let mut output = egui::FullOutput::default();
    for _ in 0..5 {
        output = frame(ctx, app, Vec2::new(1600.0, 1300.0), vec![]);
    }
    output
}
fn has(output: &egui::FullOutput, label: &str) -> bool {
    text_shapes(output)
        .iter()
        .any(|(text, _)| text.galley.job.text == label)
}

#[test]
fn cpu_core_chips_expire_with_provider_and_recover_without_reusing_stale_values() {
    let (ctx, mut app) = setup();
    let fresh = render(&ctx, &mut app);
    assert!(has(&fresh, "C1 51 °C") && has(&fresh, "C3 83 °C"));
    let shapes = text_shapes(&fresh);
    let first = shapes
        .iter()
        .find(|(text, _)| text.galley.job.text == "C1 51 °C")
        .unwrap()
        .0
        .visual_bounding_rect();
    let second = shapes
        .iter()
        .find(|(text, _)| text.galley.job.text == "C3 83 °C")
        .unwrap()
        .0
        .visual_bounding_rect();
    assert!(
        first.left() < second.left(),
        "core chips must be ordered by core index"
    );
    Arc::make_mut(&mut app.specs_view.bridge).collected_at =
        Some(Instant::now() - crate::specs::BRIDGE_STALE_AFTER - Duration::from_secs(1));
    let stale = render(&ctx, &mut app);
    assert!(has(&stale, "CPU: provider stopped"));
    assert!(
        has(&stale, "C1 --") && has(&stale, "C3 --"),
        "stopped provider must not leave fresh-looking core readings"
    );
    assert!(!has(&stale, "C1 51 °C") && !has(&stale, "C3 83 °C"));
    let bridge = Arc::make_mut(&mut app.specs_view.bridge);
    bridge.collected_at = Some(Instant::now() + Duration::from_secs(3600));
    bridge.readings[1].value = 57.0;
    let recovered = render(&ctx, &mut app);
    assert!(has(&recovered, "C1 57 °C"));
    assert!(!has(&recovered, "C1 51 °C"));
    assert!(app.specs.is_none());
}

#[test]
fn invalid_core_readings_are_gaps_and_unavailable_provider_never_exposes_retained_chips() {
    let (ctx, mut app) = setup();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
        Arc::make_mut(&mut app.specs_view.bridge).readings[1].value = value;
        let output = render(&ctx, &mut app);
        assert!(has(&output, "C1 --"));
        assert!(
            !text_shapes(&output)
                .iter()
                .any(|(text, _)| text.galley.job.text.contains("NaN")
                    || text.galley.job.text.contains(" inf"))
        );
    }
    Arc::make_mut(&mut app.specs_view.bridge).status =
        Value::unavailable("Fixture provider missing");
    let output = render(&ctx, &mut app);
    assert!(!has(&output, "C3 83 °C"));
    assert!(has(&output, "CPU & motherboard"));
    assert!(app.specs.is_none());
}

#[test]
fn fresh_provider_with_overflowing_package_temperature_retains_gap_and_can_recover() {
    let (ctx, mut app) = setup();
    let reading = &mut Arc::make_mut(&mut app.specs_view.bridge).readings[1];
    reading.key = LiveKey::CpuPackageTemperature;
    reading.value = f64::MAX;
    assert!(app.cpu_temperature_gap(Instant::now()).is_some());
    let output = render(&ctx, &mut app);
    assert!(
        !text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.job.text.contains(" inf"))
    );
    Arc::make_mut(&mut app.specs_view.bridge).readings[1].value = 61.0;
    assert!(app.cpu_temperature_gap(Instant::now()).is_none());
    let output = render(&ctx, &mut app);
    assert!(has(&output, "61 °C"));
}

#[test]
fn cached_multi_gpu_sensors_keep_explicit_state_and_missing_adapter_fields() {
    let (ctx, mut app) = setup();
    let mut second = app.snapshot.gpu_sensors.adapters[0].clone();
    second.name = "Fixture second GPU".into();
    second.uuid = None;
    second.memory = None;
    second.power_w = None;
    second.temperature_c = None;
    second.error = Some("Fixture adapter reading failed".into());
    app.snapshot.gpu_sensors.adapters[0].memory_includes_reserved = true;
    app.snapshot.gpu_sensors.adapters.push(second);
    app.snapshot.gpu_sensors.using_cached = true;
    app.snapshot.gpu_sensors.error = Some("Fixture NVML disconnected".into());
    app.snapshot.gpu_sensors.last_success = Some(Instant::now() - Duration::from_secs(20));
    let mut history = SensorHistory::default();
    let now = Instant::now();
    for (age, values) in [
        (10, Some((49, 40.0))),
        (9, Some((71, 91.0))),
        (8, None),
        (3, Some((56, 62.0))),
        (0, Some((50, 30.0))),
    ] {
        let point = values.map(|(temperature, power)| {
            let mut adapter = app.snapshot.gpu_sensors.adapters[0].clone();
            adapter.temperature_c = Some(temperature);
            adapter.power_w = Some(power);
            adapter
        });
        history.push(now - Duration::from_secs(age), point.as_ref());
    }
    app.sensor_history.insert("FIXTURE-GPU-0".into(), history);
    for page in [Page::Sensors, Page::Performance] {
        app.page = page;
        app.performance_device = PerformanceDevice::GpuSensors;
        let output = render(&ctx, &mut app);
        assert!(has(
            &output,
            if page == Page::Sensors {
                "GPU: Cached"
            } else {
                "Cached"
            }
        ));
        assert!(has(&output, "Fixture second GPU"));
        assert!(has(&output, "Fixture adapter reading failed"));
        assert!(has(&output, "--"));
        assert!(has(&output, "peak 71 °C"));
        assert!(has(&output, "peak 91.0 W"));
        assert!(output.platform_output.commands.is_empty());
    }
}

#[test]
#[ignore = "Offscreen sensor freshness review; fixtures only, no native windows or provider queries"]
fn render_sensor_freshness_review() {
    let mut renderer = offscreen::Renderer::new();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/ui-smoke/sensor-state-alpha46");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, dark, stale) in [
        ("fresh-dark", true, false),
        ("stopped-dark", true, true),
        ("stopped-light", false, true),
    ] {
        let (ctx, mut app) = setup();
        app.theme.dark = dark;
        theme::install(&ctx, app.theme);
        if stale {
            Arc::make_mut(&mut app.specs_view.bridge).collected_at =
                Some(Instant::now() - crate::specs::BRIDGE_STALE_AFTER - Duration::from_secs(1));
        }
        let size = Vec2::new(1200.0, 900.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..20 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        assert!(has(&output, if stale { "C1 --" } else { "C1 51 °C" }));
        renderer.save(&ctx, output, size, &directory.join(format!("{name}.png")));
    }
    println!(
        "SENSOR_STATE: 3 fixture-only offscreen images in {}",
        directory.display()
    );
}
