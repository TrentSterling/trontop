//! Real-render first paint: the production sampler, specs workers and icon
//! cache run headlessly, frames follow eframe's native order (`raw_input_hook`,
//! then one `run_ui` calling `logic` and `ui` per pass), and every frame is
//! drawn by the real wgpu renderer with its texture deltas. A synthetic egui
//! click (RawInput only, never OS input) opens each page; the frames after it
//! must already match the settled picture. Every page is opened twice (first
//! visit, revisit). Run with `cargo test first_paint -- --ignored`; optional
//! TRONTOP_FIRST_PAINT_OUT (PNG folder), _SCALE (1.5) and _SIZE (1000x580).
use super::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Content right of the navigation rail, in points.
const CONTENT_LEFT: f32 = 205.0;
/// Frames captured after the click; the last one is the settled picture.
const FRAMES: usize = 12;

struct Harness {
    ctx: egui::Context,
    app: TrontopApp,
    renderer: super::offscreen::Renderer,
    /// Texture deltas since the last capture; the renderer must see them all.
    pending: egui::FullOutput,
    /// A 60 Hz frame clock, as the native window's vsync: egui animations
    /// (a scroll bar sliding in) take their real number of frames.
    time: f64,
    /// Physical pixels.
    size: Vec2,
}

struct Frame {
    image: image::RgbaImage,
    texts: Vec<(String, i32, i32)>,
    /// Physical-pixel cells of textured images (process icons). Their art
    /// loads off-thread into a fixed cell, so only the cell may change.
    images: Vec<egui::Rect>,
    repaint_now: bool,
}

impl Harness {
    /// One eframe-native frame: hook, then `logic` + `ui` in every pass.
    fn step(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                self.size / self.ctx.pixels_per_point(),
            )),
            time: Some(self.time),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let mut frame = eframe::Frame::_new_kittest();
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        let app = &mut self.app;
        let output = self.ctx.run_ui(raw, |ui| {
            eframe::App::logic(app, ui.ctx(), &mut eframe::Frame::_new_kittest());
            eframe::App::ui(app, ui, &mut frame);
        });
        assert!(
            output.platform_output.commands.is_empty(),
            "unexpected platform command"
        );
        let shapes = output.shapes.clone();
        let viewport_output = output.viewport_output.clone();
        let pixels_per_point = output.pixels_per_point;
        self.pending.append(output);
        egui::FullOutput {
            shapes,
            viewport_output,
            pixels_per_point,
            ..Default::default()
        }
    }

    fn capture(&mut self, events: Vec<egui::Event>) -> Frame {
        let output = self.step(events);
        let repaint_now = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|viewport| viewport.repaint_delay == Duration::ZERO);
        let texts = content_texts(&output);
        let mut images = Vec::new();
        for clipped in &output.shapes {
            image_cells(&clipped.shape, output.pixels_per_point, &mut images);
        }
        let pending = std::mem::take(&mut self.pending);
        let image = self.renderer.capture(&self.ctx, pending, self.size);
        Frame {
            image,
            texts,
            images,
            repaint_now,
        }
    }

    /// Keeps the renderer's textures current without reading pixels back.
    fn idle(&mut self, frames: usize) {
        for _ in 0..frames {
            let _ = self.capture(vec![]);
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}

fn image_cells(shape: &egui::Shape, scale: f32, out: &mut Vec<egui::Rect>) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                image_cells(shape, scale, out);
            }
        }
        egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
            let rect = shape.visual_bounding_rect();
            out.push(egui::Rect::from_min_max(
                (rect.min.to_vec2() * scale).to_pos2(),
                (rect.max.to_vec2() * scale).to_pos2(),
            ));
        }
        _ => {}
    }
}

fn content_texts(output: &egui::FullOutput) -> Vec<(String, i32, i32)> {
    let mut out: Vec<_> = text_shapes(output)
        .iter()
        .filter(|(text, clip)| {
            let rect = text.visual_bounding_rect().intersect(*clip);
            rect.is_positive() && rect.left() > CONTENT_LEFT
        })
        .map(|(text, _)| {
            (
                text.galley.text().to_string(),
                text.pos.x.round() as i32,
                text.pos.y.round() as i32,
            )
        })
        .collect();
    out.sort();
    out
}

fn nav_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    // The lowest match: "Processes" and "System" are also rail section headers.
    text_shapes(output)
        .iter()
        .filter(|(text, _)| {
            text.galley.text() == label && text.visual_bounding_rect().left() < CONTENT_LEFT
        })
        .map(|(text, _)| text.visual_bounding_rect().center())
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap_or_else(|| panic!("no navigation entry {label}"))
}

/// Differing pixels right of the rail (physical pixels).
fn diff(
    a: &image::RgbaImage,
    b: &image::RgbaImage,
    left: u32,
    icons: &[egui::Rect],
) -> (u64, Option<[u32; 4]>) {
    let mut count = 0;
    let mut bounds: Option<[u32; 4]> = None;
    for (x, y, pa) in a.enumerate_pixels() {
        let at = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
        if x < left || icons.iter().any(|cell| cell.expand(1.0).contains(at)) {
            continue;
        }
        let pb = b.get_pixel(x, y);
        let delta =
            pa.0.iter()
                .zip(pb.0)
                .map(|(p, q)| p.abs_diff(q))
                .max()
                .unwrap();
        if delta > 24 {
            count += 1;
            let r = bounds.get_or_insert([x, y, x, y]);
            r[0] = r[0].min(x);
            r[1] = r[1].min(y);
            r[2] = r[2].max(x);
            r[3] = r[3].max(y);
        }
    }
    (count, bounds)
}

fn click(at: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
#[ignore = "REAL read-only sampler, specs workers and icon cache; offscreen wgpu; synthetic egui input only, no window or OS input"]
fn real_render_opened_pages_paint_settled_on_their_first_frame() {
    let out = std::env::var_os("TRONTOP_FIRST_PAINT_OUT").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let scale: f32 = std::env::var("TRONTOP_FIRST_PAINT_SCALE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1.0);
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    ctx.set_pixels_per_point(scale);
    let sampler = crate::sampler::Sampler::spawn(ctx.clone(), None);
    let mut app = TrontopApp::with_services(settings, Some(sampler), None);
    app.specs_enabled = true;
    app.process_icons = crate::process_icons::Cache::spawn(ctx.clone());
    app.page = Page::Overview;
    let points = std::env::var("TRONTOP_FIRST_PAINT_SIZE")
        .ok()
        .and_then(|value| {
            let (w, h) = value.split_once('x')?;
            Some(Vec2::new(w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or(Vec2::new(1280.0, 800.0));
    let mut h = Harness {
        ctx,
        app,
        renderer: super::offscreen::Renderer::new(),
        pending: egui::FullOutput::default(),
        time: 0.0,
        size: points * scale,
    };
    // Warm up on Overview until real samples flow; specs stay unstarted, as
    // in a fresh app, so first visits are exercised too.
    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        h.idle(1);
    }
    assert!(
        h.app.seen_generation > 0,
        "the sampler produced no snapshot"
    );
    let left = (CONTENT_LEFT * scale) as u32;
    let mut failures = Vec::new();
    let mut targets: Vec<Page> = Page::NAV_GROUPS
        .iter()
        .flat_map(|(_, pages)| pages.iter().copied())
        .collect();
    // First visits run bottom-up so System opens before Hardware sensors
    // (either one starts the specs workers); then every page is revisited.
    targets.reverse();
    targets.extend(targets.clone().into_iter().rev());
    for (round, target) in targets.into_iter().enumerate() {
        let label = target.title();
        let home = if target == Page::Processes {
            Page::Overview
        } else {
            Page::Processes
        };
        h.app.page = home;
        h.idle(4);
        let before = h.step(vec![]);
        let at = nav_center(&before, label);
        // A person points at the entry for a moment before clicking.
        let _ = h.capture(vec![egui::Event::PointerMoved(at)]);
        h.idle(6);
        // Freeze the data for the window: the graph clock stops and new
        // samples wait, so only the view change may move pixels. `logic`
        // still runs everything else (specs polling, preferences).
        h.app.graphs.fixed_now = Some(Instant::now());
        let sampler = h.app.sampler.take();
        let _ = h.capture(click(at, true));
        let frames: Vec<Frame> = (0..FRAMES)
            .map(|index| {
                let frame = h.capture(if index == 0 { click(at, false) } else { vec![] });
                // Real time for workers between vsync-paced frames.
                std::thread::sleep(Duration::from_millis(16));
                frame
            })
            .collect();
        h.app.graphs.fixed_now = None;
        h.app.sampler = sampler;
        assert_eq!(h.app.page, target, "{label}: the click did not open it");
        let settled = frames.last().unwrap();
        let mut report = Vec::new();
        for (index, frame) in frames.iter().enumerate().take(frames.len() - 1) {
            let icons: Vec<_> = frame
                .images
                .iter()
                .chain(&settled.images)
                .copied()
                .collect();
            let (pixels, bounds) = diff(&frame.image, &settled.image, left, &icons);
            let only: Vec<_> = frame
                .texts
                .iter()
                .filter(|text| !settled.texts.contains(text))
                .take(3)
                .collect();
            let missing: Vec<_> = settled
                .texts
                .iter()
                .filter(|text| !frame.texts.contains(text))
                .take(3)
                .collect();
            println!(
                "{round:02} {label}: frame {} repaint_now {} texts {} vs {}: {pixels} px differ {bounds:?} only {only:?} missing {missing:?}",
                index + 1,
                frame.repaint_now,
                frame.texts.len(),
                settled.texts.len(),
            );
            if pixels > 0 || frame.texts != settled.texts {
                report.push(format!("frame {} {pixels} px {bounds:?}", index + 1));
            }
        }
        if let Some(out) = &out {
            for (index, frame) in frames.iter().enumerate() {
                let slug: String = label
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                    .collect();
                frame
                    .image
                    .save(out.join(format!("{round:02}-{slug}-f{}.png", index + 1)))
                    .unwrap();
            }
        }
        if !report.is_empty() {
            failures.push(format!("{label} (round {round}): {}", report.join("; ")));
        }
    }
    assert!(
        failures.is_empty(),
        "unsettled first frames:\n{}",
        failures.join("\n")
    );
}
