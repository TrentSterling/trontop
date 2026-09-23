//! Polish gauntlet: every page at three window sizes from REAL read-only
//! telemetry. The production sampler and specs workers run headlessly for a
//! warmup so graphs carry real history; the app has no tray, no native window
//! and receives no OS input. No process, service or export action is invoked.
use super::*;
use crate::specs::SectionId;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SIZES: [(f32, f32); 3] = [(1000.0, 580.0), (1280.0, 800.0), (1600.0, 1000.0)];
/// Tallest whole-page capture, further capped by the adapter's texture limit.
const TALLEST: u32 = 16_000;
/// Height of each readable slice of a whole-page capture.
const SLICE: u32 = 1_600;
/// Text left of this belongs to the navigation sidebar.
const CONTENT_LEFT: f32 = 205.0;

struct Gauntlet {
    ctx: egui::Context,
    app: TrontopApp,
    renderer: super::offscreen::Renderer,
    /// Texture deltas from every frame since the last save. The renderer must
    /// replay all of them or glyphs added mid-sequence go missing.
    pending: egui::FullOutput,
    directory: PathBuf,
    written: Vec<String>,
    encoders: Vec<std::thread::JoinHandle<()>>,
    /// Graph lines break after 3 s without an accepted sample, so the
    /// harness reports its own worst polling gap.
    last_accept: Option<Instant>,
    worst_accept_gap: Duration,
}

impl Gauntlet {
    /// Production `logic` (sampler and specs publication reads only) then `ui`.
    /// No input events at all.
    fn frame(&mut self, size: Vec2) -> egui::FullOutput {
        let generation = self.app.seen_generation;
        eframe::App::logic(&mut self.app, &self.ctx, &mut eframe::Frame::_new_kittest());
        let output = frame(&self.ctx, &mut self.app, size, vec![]);
        if self.app.seen_generation != generation {
            let now = Instant::now();
            if let Some(last) = self.last_accept.replace(now) {
                self.worst_accept_gap = self.worst_accept_gap.max(now - last);
            }
        }
        let shapes = output.shapes.clone();
        self.pending.append(output);
        egui::FullOutput {
            shapes,
            ..Default::default()
        }
    }

    fn shoot(&mut self, size: Vec2, name: &str, frames: usize) {
        self.shoot_named(size, size, name, frames, None);
    }

    /// `label` is the window size in the file name; `size` is what is drawn.
    fn shoot_named(
        &mut self,
        label: Vec2,
        size: Vec2,
        name: &str,
        frames: usize,
        slices: Option<u32>,
    ) {
        for _ in 0..frames.max(1) {
            self.frame(size);
        }
        let output = std::mem::take(&mut self.pending);
        let image = self.renderer.capture(&self.ctx, output, size);
        let stem = format!("{}x{}-{name}", label.x as u32, label.y as u32);
        self.encode(stem, image, slices);
    }

    /// PNG encoding runs off this thread: a slow debug encode here would
    /// starve sample polling and draw false gaps into the graphs.
    fn encode(&mut self, stem: String, image: image::RgbaImage, slices: Option<u32>) {
        let mut files = vec![format!("{stem}.png")];
        let slice = slices.filter(|slice| image.height() > *slice);
        if let Some(slice) = slice {
            let count = image.height().div_ceil(slice);
            files.extend((1..=count).map(|index| format!("{stem}-part{index}.png")));
        }
        self.written.extend(files.iter().cloned());
        self.encoders.retain(|encoder| !encoder.is_finished());
        let directory = self.directory.clone();
        self.encoders.push(std::thread::spawn(move || {
            image.save(directory.join(&files[0])).unwrap();
            if let Some(slice) = slice {
                for (file, top) in files[1..]
                    .iter()
                    .zip((0..image.height()).step_by(slice as usize))
                {
                    let height = slice.min(image.height() - top);
                    image::imageops::crop_imm(&image, 0, top, image.width(), height)
                        .to_image()
                        .save(directory.join(file))
                        .unwrap();
                }
            }
        }));
    }

    /// Whole scrolled page: lay out very tall, measure the lowest content
    /// text, then render at that height. Also writes readable `-part<N>`
    /// slices of the same image. Named after the window size `base`, so
    /// rounds compare file for file.
    fn shoot_full(&mut self, base: Vec2, name: &str, frames: usize) {
        let tallest = self.renderer.max_dimension().min(TALLEST) as f32;
        let probe = Vec2::new(base.x, tallest);
        let mut shapes = egui::FullOutput::default();
        for _ in 0..3 {
            shapes = self.frame(probe);
        }
        let bottom = text_shapes(&shapes)
            .iter()
            .map(|(text, clip)| text.visual_bounding_rect().intersect(*clip))
            .filter(|rect| rect.left() > CONTENT_LEFT && rect.is_positive())
            .map(|rect| rect.bottom())
            .fold(0.0_f32, f32::max);
        if bottom >= tallest - 60.0 {
            println!("GAUNTLET WARNING: {name} at {base:?} is taller than {tallest} px and is cut");
        }
        let height = (bottom + 40.0).clamp(base.y, tallest).ceil();
        let slice = SLICE.max(base.y as u32);
        self.shoot_named(base, Vec2::new(base.x, height), name, frames, Some(slice));
    }
}

fn slug(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

fn warm_up(g: &mut Gauntlet) {
    let seconds = std::env::var("TRONTOP_GAUNTLET_WARMUP_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(20);
    let size = Vec2::new(SIZES[0].0, SIZES[0].1);
    // System starts the lazily spawned specs workers, exactly as a first
    // visit would in the running app.
    g.app.page = Page::System;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut accepted = 0u64;
    let mut last = 0;
    while Instant::now() < deadline {
        g.frame(size);
        if g.app.seen_generation != last {
            last = g.app.seen_generation;
            accepted += 1;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let ready = g
        .app
        .specs_view
        .entries
        .iter()
        .filter(|entry| entry.section.is_some())
        .count();
    println!(
        "GAUNTLET warmup {seconds}s: {accepted} real samples accepted (sequence {last}), \
         {} graph signals, specs sections ready {ready}/{}",
        g.app.graphs.gauntlet_signal_count(),
        g.app.specs_view.entries.len()
    );
    assert!(accepted > 0, "the sampler produced no snapshot");
}

fn pages_at(g: &mut Gauntlet, size: Vec2) {
    const PAGE: usize = 4;
    const DIALOG: usize = 16;
    g.app.show_theme_editor = false;
    g.app.show_diagnostics = false;
    g.app.show_export = false;
    g.app.selected_pid = None;

    g.app.page = Page::Overview;
    g.shoot(size, "overview", PAGE);
    g.shoot_full(size, "overview-full", PAGE);

    g.app.page = Page::Graphs;
    for (tab, label) in crate::app::graphs::Dashboard::gauntlet_tabs() {
        g.app.graphs.set_gauntlet_view(tab, false);
        g.shoot(size, &format!("graphs-{}", slug(label)), PAGE);
    }
    g.app.graphs.set_gauntlet_view(None, true);
    g.shoot(size, "graphs-everything-bars", PAGE);
    g.app.graphs.set_gauntlet_view(None, false);
    g.shoot_full(size, "graphs-everything-full", PAGE);

    g.app.page = Page::Processes;
    g.app.tree_mode = true;
    g.shoot(size, "processes-tree", PAGE);
    g.app.tree_mode = false;
    g.shoot(size, "processes-flat", PAGE);
    g.app.tree_mode = true;

    g.app.page = Page::Performance;
    let mut devices = vec![
        (PerformanceDevice::Cpu, "cpu".to_string()),
        (PerformanceDevice::Memory, "memory".into()),
        (PerformanceDevice::Gpu, "gpu".into()),
        (PerformanceDevice::GpuSensors, "gpu-sensors".into()),
        (PerformanceDevice::PhysicalDisks, "physical-disks".into()),
    ];
    for index in 0..g.app.snapshot.disks.len() {
        devices.push((PerformanceDevice::Disk(index), format!("volume-{index}")));
    }
    for index in 0..g.app.snapshot.networks.len() {
        devices.push((
            PerformanceDevice::Network(index),
            format!("network-{index}"),
        ));
    }
    for (device, name) in devices {
        g.app.performance_device = device;
        g.shoot(size, &format!("performance-{name}"), PAGE);
    }
    g.app.performance_device = PerformanceDevice::Cpu;

    for (page, name) in [
        (Page::History, "history"),
        (Page::Startup, "startup"),
        (Page::Users, "users"),
        (Page::Details, "details"),
        (Page::Services, "services"),
    ] {
        g.app.page = page;
        g.shoot(size, name, PAGE);
    }

    g.app.page = Page::Sensors;
    g.shoot(size, "sensors", PAGE);
    g.shoot_full(size, "sensors-full", PAGE);

    g.app.page = Page::System;
    for id in SectionId::ALL {
        g.app.system_section = id;
        g.shoot(size, &format!("system-{}", id.key()), PAGE);
    }
    g.app.system_section = SectionId::Summary;
    g.shoot_full(size, "system-summary-full", PAGE);

    // Dialogs over Overview; they need a few frames to fade in.
    g.app.page = Page::Overview;
    g.app.show_theme_editor = true;
    for (tab, name) in ["studio", "appearance", "presets", "library"]
        .into_iter()
        .enumerate()
    {
        g.app.theme_studio.tab = tab;
        g.shoot(size, &format!("theme-{name}"), DIALOG);
    }
    g.app.theme_studio.tab = 0;
    g.app.show_theme_editor = false;
    // Let the Studio window fade out before the next dialog.
    for _ in 0..DIALOG {
        g.frame(size);
    }
    g.app.show_diagnostics = true;
    g.shoot(size, "about", DIALOG);
    g.app.show_diagnostics = false;
    for _ in 0..DIALOG {
        g.frame(size);
    }
    // Display only. Nothing is clicked, and this exporter has no backend.
    g.app.page = Page::Processes;
    g.app.show_export = true;
    g.shoot(size, "export", DIALOG);
    g.app.show_export = false;
    for _ in 0..DIALOG {
        g.frame(size);
    }
}

fn theme_spot_checks(g: &mut Gauntlet) {
    let size = Vec2::new(1280.0, 800.0);
    let original = g.app.theme;
    g.app.page = Page::Overview;
    for (settings, name) in [
        (
            ThemeSettings {
                dark: false,
                ..ThemeSettings::default()
            },
            "overview-light",
        ),
        (ThemeSettings::copper_legacy(), "overview-copper"),
        (
            ThemeSettings {
                dark: false,
                ..ThemeSettings::copper_legacy()
            },
            "overview-copper-light",
        ),
    ] {
        g.app.theme = settings;
        theme::install(&g.ctx, settings);
        g.shoot(size, name, 4);
        g.shoot_full(size, &format!("{name}-full"), 4);
    }
    g.app.theme = original;
    theme::install(&g.ctx, original);
}

#[test]
#[ignore = "polish gauntlet: REAL read-only sampler and specs workers, offscreen PNGs to TRONTOP_GAUNTLET_OUT; no window, tray or OS input"]
fn render_gauntlet_all_pages() {
    let directory = PathBuf::from(
        std::env::var_os("TRONTOP_GAUNTLET_OUT")
            .expect("set TRONTOP_GAUNTLET_OUT to the PNG output directory"),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let started = Instant::now();
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let sampler = crate::sampler::Sampler::spawn(ctx.clone(), None);
    let mut app = TrontopApp::with_services(settings, Some(sampler), None);
    // Specs workers are read-only and start lazily on the System page.
    app.specs_enabled = true;
    assert!(app.tray.is_none());
    let mut g = Gauntlet {
        ctx,
        app,
        renderer: super::offscreen::Renderer::new(),
        pending: egui::FullOutput::default(),
        directory,
        written: Vec::new(),
        encoders: Vec::new(),
        last_accept: None,
        worst_accept_gap: Duration::ZERO,
    };
    warm_up(&mut g);
    for (w, h) in SIZES {
        pages_at(&mut g, Vec2::new(w, h));
    }
    theme_spot_checks(&mut g);
    for encoder in std::mem::take(&mut g.encoders) {
        encoder.join().expect("PNG encoder");
    }
    for file in &g.written {
        let path: &Path = &g.directory.join(file);
        assert!(
            std::fs::metadata(path).is_ok_and(|m| m.len() > 0),
            "{file} missing"
        );
    }
    if g.worst_accept_gap >= Duration::from_secs(3) {
        println!("GAUNTLET WARNING: harness polling was slow; some graph gaps are artifacts");
    }
    println!(
        "GAUNTLET worst gap between accepted samples: {:.2}s (graph lines break above 3 s)",
        g.worst_accept_gap.as_secs_f32()
    );
    println!(
        "GAUNTLET: {} PNGs in {} after {:.1}s; real read-only telemetry, no window or OS input",
        g.written.len(),
        g.directory.display(),
        started.elapsed().as_secs_f32()
    );
    for file in &g.written {
        println!("  {file}");
    }
}
