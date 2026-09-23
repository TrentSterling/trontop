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

/// How a gauntlet state is captured.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// One window-sized frame.
    Screen,
    /// The whole scrolled page, also sliced.
    Full,
    /// A dialog over the page; the number identifies the window so the
    /// harness lets a different one fade out first.
    Dialog(u8),
}

/// One named UI state. `set` configures the app completely from the baseline,
/// so a state never depends on the one before it.
struct Step {
    name: String,
    kind: Kind,
    set: Box<dyn Fn(&mut TrontopApp)>,
}

impl Step {
    fn new(name: impl Into<String>, kind: Kind, set: impl Fn(&mut TrontopApp) + 'static) -> Self {
        Self {
            name: name.into(),
            kind,
            set: Box::new(move |app| {
                baseline(app);
                set(app);
            }),
        }
    }
}

/// No dialog, no selection, default sub-views.
fn baseline(app: &mut TrontopApp) {
    app.show_theme_editor = false;
    app.show_diagnostics = false;
    app.show_export = false;
    app.selected_pid = None;
    app.tree_mode = true;
    app.performance_device = PerformanceDevice::Cpu;
    app.graphs.cpu_all_cores = false;
    app.graphs.set_gauntlet_view(None, false);
    app.system_section = SectionId::Summary;
    app.theme_studio.tab = 0;
}

/// Every state the polish gauntlet photographs, in capture order. Device
/// counts come from the current snapshot.
fn steps(app: &TrontopApp) -> Vec<Step> {
    let mut steps = vec![
        Step::new("overview", Kind::Screen, |app| app.page = Page::Overview),
        Step::new("overview-full", Kind::Full, |app| app.page = Page::Overview),
    ];
    for (tab, label) in crate::app::graphs::Dashboard::gauntlet_tabs() {
        steps.push(Step::new(
            format!("graphs-{}", slug(label)),
            Kind::Screen,
            move |app| {
                app.page = Page::Graphs;
                app.graphs.set_gauntlet_view(tab, false);
            },
        ));
    }
    steps.push(Step::new("graphs-everything-bars", Kind::Screen, |app| {
        app.page = Page::Graphs;
        app.graphs.set_gauntlet_view(None, true);
    }));
    steps.push(Step::new("graphs-everything-full", Kind::Full, |app| {
        app.page = Page::Graphs;
    }));
    steps.push(Step::new("processes-tree", Kind::Screen, |app| {
        app.page = Page::Processes;
    }));
    steps.push(Step::new("processes-flat", Kind::Screen, |app| {
        app.page = Page::Processes;
        app.tree_mode = false;
    }));
    let mut devices = vec![
        (PerformanceDevice::Cpu, "cpu".to_string()),
        (PerformanceDevice::Memory, "memory".into()),
        (PerformanceDevice::Gpu, "gpu".into()),
        (PerformanceDevice::GpuSensors, "gpu-sensors".into()),
        (PerformanceDevice::PhysicalDisks, "physical-disks".into()),
    ];
    for index in 0..app.snapshot.disks.len() {
        devices.push((PerformanceDevice::Disk(index), format!("volume-{index}")));
    }
    for index in 0..app.snapshot.networks.len() {
        devices.push((
            PerformanceDevice::Network(index),
            format!("network-{index}"),
        ));
    }
    for (device, name) in devices {
        steps.push(Step::new(
            format!("performance-{name}"),
            Kind::Screen,
            move |app| {
                app.page = Page::Performance;
                app.performance_device = device;
            },
        ));
    }
    steps.push(Step::new("performance-cpu-cores", Kind::Screen, |app| {
        app.page = Page::Performance;
        app.graphs.cpu_all_cores = true;
    }));
    for (page, name) in [
        (Page::History, "history"),
        (Page::Startup, "startup"),
        (Page::Users, "users"),
        (Page::Details, "details"),
        (Page::Services, "services"),
    ] {
        steps.push(Step::new(name, Kind::Screen, move |app| app.page = page));
    }
    steps.push(Step::new("sensors", Kind::Screen, |app| {
        app.page = Page::Sensors;
    }));
    steps.push(Step::new("sensors-full", Kind::Full, |app| {
        app.page = Page::Sensors;
    }));
    for id in SectionId::ALL {
        steps.push(Step::new(
            format!("system-{}", id.key()),
            Kind::Screen,
            move |app| {
                app.page = Page::System;
                app.system_section = id;
            },
        ));
    }
    steps.push(Step::new("system-summary-full", Kind::Full, |app| {
        app.page = Page::System;
    }));
    // Dialogs over Overview; Export over Processes. Display only: nothing is
    // clicked, and the gauntlet exporter has no backend.
    for (tab, name) in ["studio", "appearance", "presets", "library"]
        .into_iter()
        .enumerate()
    {
        steps.push(Step::new(
            format!("theme-{name}"),
            Kind::Dialog(0),
            move |app| {
                app.page = Page::Overview;
                app.show_theme_editor = true;
                app.theme_studio.tab = tab;
            },
        ));
    }
    steps.push(Step::new("about", Kind::Dialog(1), |app| {
        app.page = Page::Overview;
        app.show_diagnostics = true;
    }));
    steps.push(Step::new("export", Kind::Dialog(2), |app| {
        app.page = Page::Processes;
        app.show_export = true;
    }));
    steps
}

fn pages_at(g: &mut Gauntlet, size: Vec2) {
    const PAGE: usize = 4;
    const DIALOG: usize = 16;
    let mut previous = Kind::Screen;
    for step in steps(&g.app) {
        // A dialog needs a few frames to fade in, and a different one must
        // fade out first.
        if matches!(previous, Kind::Dialog(_)) && previous != step.kind {
            baseline(&mut g.app);
            for _ in 0..DIALOG {
                g.frame(size);
            }
        }
        (step.set)(&mut g.app);
        match step.kind {
            Kind::Screen => g.shoot(size, &step.name, PAGE),
            Kind::Full => g.shoot_full(size, &step.name, PAGE),
            Kind::Dialog(_) => g.shoot(size, &step.name, DIALOG),
        }
        previous = step.kind;
    }
    baseline(&mut g.app);
    for _ in 0..DIALOG {
        g.frame(size);
    }
}

/// egui debug builds paint a 2 px red outline (clip `EVERYTHING`) when a
/// widget rect keeps its place but changes id between frames, and a red
/// outline plus "Double use of ... ID" text on a same-frame id clash.
fn id_warnings(output: &egui::FullOutput) -> Vec<String> {
    let mut found = Vec::new();
    for clipped in &output.shapes {
        if let egui::Shape::Rect(rect) = &clipped.shape
            && clipped.clip_rect == egui::Rect::EVERYTHING
            && rect.stroke.color == egui::Color32::RED
            && rect.stroke.width >= 1.0
        {
            found.push(format!("id warning outline at {:?}", rect.rect));
        }
    }
    for (text, _) in text_shapes(output) {
        let text = &text.galley.job.text;
        if text.contains("Double use of") || text.contains("changed id") {
            found.push(text.clone());
        }
    }
    found
}

/// Flips the title-bar System state between Live and Stale, as a slow sample
/// does in the running app. The status pill appears and disappears with it.
fn set_system_stale(app: &mut TrontopApp, stale: bool) {
    let at = Instant::now() + Duration::from_secs(3600);
    let health = app
        .snapshot
        .diagnostics
        .get_mut(crate::diagnostics::Provider::System);
    health.record(
        at,
        Duration::from_micros(420),
        crate::diagnostics::State::Live,
        None,
        None,
    );
    if stale {
        health.record(
            at,
            Duration::from_micros(420),
            crate::diagnostics::State::Unavailable,
            None,
            None,
        );
    }
}

#[test]
fn gauntlet_states_paint_no_egui_id_warnings_in_debug_builds() {
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    // The debug-build defaults, made explicit so the test is never vacuous.
    ctx.all_styles_mut(|style| style.debug.warn_if_rect_changes_id = true);
    ctx.options_mut(|options| options.warn_on_id_clash = true);
    let mut app = app(settings, true);
    let mut failures = Vec::new();
    let mut checked = 0;
    for (w, h) in SIZES {
        let size = Vec2::new(w, h);
        for step in steps(&app) {
            (step.set)(&mut app);
            set_system_stale(&mut app, false);
            // Let the state settle; a page switch may legitimately move ids.
            for _ in 0..3 {
                frame(&ctx, &mut app, size, vec![]);
            }
            for stale in [false, true, false] {
                set_system_stale(&mut app, stale);
                let output = frame(&ctx, &mut app, size, vec![]);
                for warning in id_warnings(&output) {
                    failures.push(format!("{w}x{h} {} (stale {stale}): {warning}", step.name));
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 150, "only {checked} frames checked");
    assert!(
        failures.is_empty(),
        "egui id warnings:\n{}",
        failures.join("\n")
    );
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
