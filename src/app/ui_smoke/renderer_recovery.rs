//! Opt-in GPU fault injection. Only the test's own offscreen device is destroyed;
//! this never resets an adapter/driver or creates a native window, tray or input.
use super::*;
use eframe::{egui_wgpu, wgpu};
use egui_wgpu::recovery::{DeviceRecovery, TextureReplay};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[test]
fn renderer_recovery_retains_view_but_disables_stale_process_actions() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), false);
    let size = Vec2::new(1040.0, 640.0);
    app.selected_pid = Some(900_000);
    app.show_run_task = true;
    let saved_theme = app.theme.encode();
    app.graphics_recovering
        .store(true, std::sync::atomic::Ordering::Release);
    let output = frame(&ctx, &mut app, size, vec![]);
    let texts = text_shapes(&output);
    assert!(
        texts
            .iter()
            .any(|(text, _)| text.galley.job.text == "Reconnecting graphics")
    );
    assert!(
        !texts
            .iter()
            .any(|(text, _)| text.galley.job.text == "Run new task")
    );
    assert_eq!(app.selected_pid, Some(900_000));
    assert_eq!(app.theme.encode(), saved_theme);
    assert!(app.show_run_task);
    app.graphics_recovering
        .store(false, std::sync::atomic::Ordering::Release);
    for _ in 0..3 {
        frame(&ctx, &mut app, size, vec![]);
    }
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        !text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.job.text == "Reconnecting graphics")
    );
    assert_eq!(app.selected_pid, Some(900_000));
    assert_eq!(app.theme.encode(), saved_theme);
}

#[test]
fn renderer_replay_retains_only_live_texture_allocations() {
    let mut replay = TextureReplay::default();
    let id = egui::TextureId::Managed(15);
    let full = egui::epaint::ImageDelta::full(
        egui::ColorImage::filled([8, 8], Color32::RED),
        egui::TextureOptions::LINEAR,
    );
    replay.apply(&egui::TexturesDelta {
        set: vec![(id, full)],
        free: vec![],
    });
    assert_eq!(replay.bytes(), 256);
    for _ in 0..1000 {
        replay.apply(&egui::TexturesDelta {
            set: vec![(
                id,
                egui::epaint::ImageDelta::partial(
                    [2, 3],
                    egui::ColorImage::filled([2, 2], Color32::GREEN),
                    egui::TextureOptions::NEAREST,
                ),
            )],
            free: vec![],
        });
        assert_eq!(replay.bytes(), 256, "partial uploads must not accumulate");
    }
    let merged = replay.image(id).unwrap();
    assert!(merged.pos.is_none());
    assert_eq!(merged.options, egui::TextureOptions::NEAREST);
    let egui::ImageData::Color(image) = &merged.image;
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(
                image[(x, y)],
                if (2..4).contains(&x) && (3..5).contains(&y) {
                    Color32::GREEN
                } else {
                    Color32::RED
                }
            );
        }
    }
    replay.apply(&egui::TexturesDelta {
        set: vec![],
        free: vec![id],
    });
    assert_eq!(
        replay.bytes(),
        256,
        "free follows the last frame using the texture"
    );
    replay.apply(&Default::default());
    assert_eq!(replay.bytes(), 0);
}

/// Draw real production UI meshes into a 2D texture and read it back. No surface.
fn pixels(
    state: &egui_wgpu::RenderState,
    jobs: &[egui::ClippedPrimitive],
    screen: &egui_wgpu::ScreenDescriptor,
) -> Vec<u8> {
    let [width, height] = screen.size_in_pixels;
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = state.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Trontop recovery fixture"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: state.target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let stride = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = state.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Trontop recovery readback"),
        size: u64::from(stride * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let view = target.create_view(&Default::default());
    let mut encoder = state.device.create_command_encoder(&Default::default());
    let mut renderer = state.renderer.write();
    let mut commands =
        renderer.update_buffers(&state.device, &state.queue, &mut encoder, jobs, screen);
    assert!(!renderer.upload_failed());
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            })
            .forget_lifetime();
        renderer.render(&mut pass, jobs, screen);
    }
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: None,
            },
        },
        extent,
    );
    commands.push(encoder.finish());
    let submission = state.queue.submit(commands);
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
    state
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(10)),
        })
        .unwrap();
    receive
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    let data = {
        let mapped = readback.slice(..).get_mapped_range();
        mapped
            .chunks(stride as usize)
            .flat_map(|row| row[..width as usize * 4].iter().copied())
            .collect()
    };
    readback.unmap();
    data
}

#[test]
#[ignore = "Opt-in offscreen fault test; destroys only its own wgpu device"]
fn offscreen_device_loss_recovers_production_ui_pixels() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let reported = Arc::clone(&events);
    let config = egui_wgpu::WgpuConfiguration {
        recover_device: true,
        on_renderer_event: Arc::new(move |event| reported.lock().unwrap().push(event)),
        ..Default::default()
    };
    let instance = pollster::block_on(config.wgpu_setup.new_instance());
    let options = egui_wgpu::RendererOptions::PREDICTABLE;
    let mut state = pollster::block_on(egui_wgpu::RenderState::create(
        &config, &instance, None, options,
    ))
    .unwrap();
    println!("RECOVERY adapter={:?}", state.adapter.get_info());
    let mut recovery = DeviceRecovery::default();
    recovery.arm(&state, Arc::new(|| {}), &config);
    let mut replay = TextureReplay::default();
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), false);
    theme::install(&ctx, app.theme);
    app.selected_pid = Some(900_000);
    let size = Vec2::new(1280.0, 760.0);
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [1280, 760],
        pixels_per_point: 1.0,
    };
    let mut jobs = Vec::new();
    // Warm layout and perform incremental atlas uploads, then compare exact pixels
    // after rebuilding all GPU resources without rebuilding the application/context.
    for _ in 0..4 {
        let output = frame(&ctx, &mut app, size, vec![]);
        replay.apply(&output.textures_delta);
        replay.upload(&state);
        jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
        let _ = pixels(&state, &jobs, &screen);
    }
    let before = pixels(&state, &jobs, &screen);
    assert!(
        before
            .chunks_exact(4)
            .any(|p| p[0] > 100 && p[1] > 100 && p[2] > 100),
        "fixture must render real text"
    );
    let bytes = replay.bytes();
    for cycle in 0..3 {
        state.device.destroy();
        let mut encoder = state.device.create_command_encoder(&Default::default());
        {
            let mut renderer = state.renderer.write();
            // Alpha.19 panicked here. A destroyed device deterministically reaches
            // the same None-staging-buffer branch without crashing the GPU/desktop.
            renderer.update_buffers(&state.device, &state.queue, &mut encoder, &jobs, &screen);
            assert!(renderer.upload_failed());
        }
        recovery.fail_upload(&config);
        assert!(recovery.needed());
        let started = Instant::now();
        let mut longest_poll = Duration::ZERO;
        let replacement = loop {
            let poll = Instant::now();
            let result = recovery.poll_or_start(&config, &instance, None, options, Arc::new(|| {}));
            longest_poll = longest_poll.max(poll.elapsed());
            if let Some(state) = result {
                break state;
            }
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "replacement device timed out"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        state = replacement;
        replay.invalidate();
        // A new device can arrive before the native surface is usable. Simulate
        // skipped frames, including a texture born and retired during the wait.
        // The full replay must remain pending and include only current textures.
        let transient_id = egui::TextureId::Managed(987_654);
        replay.apply(&egui::TexturesDelta {
            set: vec![(
                transient_id,
                egui::epaint::ImageDelta::full(
                    egui::ColorImage::filled([2, 2], Color32::GREEN),
                    egui::TextureOptions::LINEAR,
                ),
            )],
            free: vec![],
        });
        replay.apply(&egui::TexturesDelta {
            set: vec![],
            free: vec![transient_id],
        });
        replay.apply(&Default::default());
        assert!(replay.image(transient_id).is_none());
        assert!(replay.upload_if_pending(&state));
        assert!(!replay.upload_if_pending(&state), "replay only once");
        let after = pixels(&state, &jobs, &screen);
        assert_eq!(
            before, after,
            "fonts/icons/gradients changed after recovery {cycle}"
        );
        assert_eq!(app.selected_pid, Some(900_000));
        assert_eq!(replay.bytes(), bytes);
        assert!(!recovery.needed());
        println!(
            "RECOVERY cycle={cycle} elapsed_ms={:.2} longest_poll_ms={:.3} replay_bytes={bytes} pixels_identical=true",
            started.elapsed().as_secs_f64() * 1000.0,
            longest_poll.as_secs_f64() * 1000.0
        );
    }
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, egui_wgpu::RendererEvent::UploadFailed))
            .count(),
        3
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, egui_wgpu::RendererEvent::RecoveryStarted))
            .count(),
        3
    );
}

#[test]
#[ignore = "Opt-in offscreen adapter setup with injected failure/stall; no windows"]
fn offscreen_recovery_failure_and_stall_keep_ui_nonblocking() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let attempts = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(AtomicUsize::new(0));
    let invoked = Arc::clone(&attempts);
    let failed = Arc::clone(&failures);
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.native_adapter_selector = Some(Arc::new(move |_, _| {
        invoked.fetch_add(1, Ordering::Relaxed);
        Err("Injected test-only adapter refusal".into())
    }));
    let config = egui_wgpu::WgpuConfiguration {
        recover_device: true,
        wgpu_setup: setup.into(),
        on_renderer_event: Arc::new(move |event| {
            if matches!(event, egui_wgpu::RendererEvent::RecoveryFailed) {
                failed.fetch_add(1, Ordering::Relaxed);
            }
        }),
        ..Default::default()
    };
    let instance = pollster::block_on(config.wgpu_setup.new_instance());
    let options = egui_wgpu::RendererOptions::PREDICTABLE;
    let mut recovery = DeviceRecovery::default();
    // Healthy calls cannot accidentally start another renderer.
    assert!(
        recovery
            .poll_or_start(&config, &instance, None, options, Arc::new(|| {}))
            .is_none()
    );
    assert_eq!(attempts.load(Ordering::Relaxed), 0);
    recovery.fail_upload(&config);
    let deadline = Instant::now() + Duration::from_secs(10);
    while failures.load(Ordering::Relaxed) == 0 {
        assert!(
            recovery
                .poll_or_start(&config, &instance, None, options, Arc::new(|| {}))
                .is_none()
        );
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    for _ in 0..1000 {
        assert!(
            recovery
                .poll_or_start(&config, &instance, None, options, Arc::new(|| {}))
                .is_none()
        );
    }
    assert_eq!(
        attempts.load(Ordering::Relaxed),
        1,
        "failed setup must back off"
    );
    assert_eq!(failures.load(Ordering::Relaxed), 1);

    let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    struct Release(Arc<(Mutex<bool>, std::sync::Condvar)>);
    impl Drop for Release {
        fn drop(&mut self) {
            *self.0.0.lock().unwrap() = true;
            self.0.1.notify_all();
        }
    }
    let release = Release(Arc::clone(&gate));
    let (started_send, started_receive) = std::sync::mpsc::sync_channel(1);
    let (done_send, done_receive) = std::sync::mpsc::sync_channel(1);
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.native_adapter_selector = Some(Arc::new(move |_, _| {
        started_send.try_send(()).unwrap();
        let _guard = gate
            .1
            .wait_while(gate.0.lock().unwrap(), |released| !*released)
            .unwrap();
        Err("Injected stalled setup released".into())
    }));
    let config = egui_wgpu::WgpuConfiguration {
        recover_device: true,
        wgpu_setup: setup.into(),
        ..Default::default()
    };
    let mut recovery = DeviceRecovery::default();
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let _ = done_send.try_send(());
    });
    recovery.fail_upload(&config);
    assert!(
        recovery
            .poll_or_start(&config, &instance, None, options, Arc::clone(&wake))
            .is_none()
    );
    started_receive
        .recv_timeout(Duration::from_secs(10))
        .unwrap();
    let at = Instant::now();
    for _ in 0..1000 {
        assert!(
            recovery
                .poll_or_start(&config, &instance, None, options, Arc::clone(&wake))
                .is_none()
        );
    }
    let polling = at.elapsed();
    let at = Instant::now();
    drop(recovery); // The initializer is still blocked. This must not join it.
    let dropping = at.elapsed();
    drop(release);
    done_receive.recv_timeout(Duration::from_secs(10)).unwrap();
    println!(
        "RECOVERY_STALL polls=1000 total_us={:.1} drop_us={:.1} retry_attempts=1",
        polling.as_secs_f64() * 1e6,
        dropping.as_secs_f64() * 1e6
    );
}
