//! A view must reach the screen already laid out: the first painted frame
//! after a page opens, or after its content arrives, matches the settled
//! layout. Frames run on a 60 Hz clock like the native window, so egui's
//! scroll-bar slide-in is visible to these tests. Synthetic TEST DATA and
//! egui RawInput only: no workers, windows or OS input.
use super::*;
use crate::specs::{SectionId, fixtures};

const SIZE: Vec2 = Vec2::new(1000.0, 580.0);
/// Text right of this belongs to the page, not the navigation rail.
const CONTENT_LEFT: f32 = 205.0;

struct Clock {
    ctx: egui::Context,
    time: f64,
}

impl Clock {
    fn new(scale: f32) -> Self {
        let ctx = egui::Context::default();
        theme::install(&ctx, ThemeSettings::default());
        ctx.set_pixels_per_point(scale);
        Self { ctx, time: 0.0 }
    }

    /// One 60 Hz frame of `ui` only, as `frame` but with a clock.
    fn frame(&mut self, app: &mut TrontopApp, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
                time: Some(self.time),
                predicted_dt: 1.0 / 60.0,
                events,
                ..Default::default()
            },
            |ui| app.ui(ui, &mut eframe::Frame::_new_kittest()),
        );
        assert!(output.platform_output.commands.is_empty());
        output
    }

    /// Enough frames for any egui animation (1/12 s) to finish.
    fn settle(&mut self, app: &mut TrontopApp) -> egui::FullOutput {
        let mut output = self.frame(app, vec![]);
        for _ in 0..30 {
            output = self.frame(app, vec![]);
        }
        output
    }
}

/// Content text with its rounded position, right of the navigation rail.
fn layout(output: &egui::FullOutput) -> Vec<(String, i32, i32)> {
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

fn differences(first: &egui::FullOutput, settled: &egui::FullOutput) -> Option<String> {
    let (a, b) = (layout(first), layout(settled));
    (a != b).then(|| {
        let only_first: Vec<_> = a.iter().filter(|x| !b.contains(x)).take(3).collect();
        let only_settled: Vec<_> = b.iter().filter(|x| !a.contains(x)).take(3).collect();
        format!(
            "first {} vs settled {} texts; first only {only_first:?}; settled only {only_settled:?}",
            a.len(),
            b.len()
        )
    })
}

/// A settled page is idle: one pass per frame and no repaint loop, so the
/// settling work never costs CPU while nothing changes.
fn busy(settled: &egui::FullOutput) -> Option<String> {
    let passes = settled.platform_output.num_completed_passes;
    let delay = settled.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    (passes != 1 || delay.is_zero()).then(|| {
        format!(
            "settled frame not idle: {passes} passes, repaint in {delay:?}, discards {:?}",
            settled.platform_output.request_discard_reasons
        )
    })
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

fn click(at: egui::Pos2) -> Vec<egui::Event> {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    vec![egui::Event::PointerMoved(at), button(true), button(false)]
}

/// Opens `label` from `home` and returns the first painted frame and the
/// settled one.
fn open(
    clock: &mut Clock,
    app: &mut TrontopApp,
    home: Page,
    label: &str,
) -> (egui::FullOutput, egui::FullOutput) {
    app.page = home;
    let before = clock.settle(app);
    let at = nav_center(&before, label);
    let first = clock.frame(app, click(at));
    assert_eq!(app.page.title(), label, "the click did not open {label}");
    let settled = clock.settle(app);
    (first, settled)
}

/// Overflowing pages open with their solid scroll bar already reserved:
/// egui otherwise lays a new scroll area out full width, then narrows the
/// content as the bar slides in over the next frames.
#[test]
fn opened_pages_paint_their_settled_layout_on_the_first_frame() {
    let mut failures = Vec::new();
    for scale in [1.0_f32, 1.5] {
        for (home, label) in [
            (Page::Processes, "System"),
            (Page::Processes, "Overview"),
            (Page::Overview, "Details"),
            (Page::Overview, "History"),
            (Page::Overview, "Hardware sensors"),
        ] {
            let mut clock = Clock::new(scale);
            let mut app = app(ThemeSettings::default(), true);
            app.specs_view = fixtures::snapshot();
            let (first, settled) = open(&mut clock, &mut app, home, label);
            if let Some(difference) = differences(&first, &settled) {
                failures.push(format!("{label} @{scale}: {difference}"));
            }
            if let Some(busy) = busy(&settled) {
                failures.push(format!("{label} @{scale}: {busy}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Content that arrives while a page is open (a worker publishing) is laid
/// out with the scroll bar it needs in the same frame.
#[test]
fn arriving_content_paints_settled_in_its_first_frame() {
    let mut clock = Clock::new(1.0);
    let mut app = app(ThemeSettings::default(), true);
    app.page = Page::System;
    // Nothing published yet: the page fits without a scroll bar.
    app.specs_view = crate::specs::Snapshot {
        entries: Vec::new(),
        ..Default::default()
    };
    clock.settle(&mut app);
    app.specs_view = fixtures::snapshot();
    let first = clock.frame(&mut app, vec![]);
    let settled = clock.settle(&mut app);
    assert!(
        layout(&settled).iter().any(|(text, _, _)| text == "CPU"),
        "the fixture sections never drew"
    );
    if let Some(difference) = differences(&first, &settled) {
        panic!("{difference}");
    }
    assert_eq!(busy(&settled), None);
}

fn fixture_cpu(_: &crate::specs::Context) -> crate::specs::Section {
    use crate::specs::{Group, SummaryLine, Value};
    crate::specs::Section::new(SectionId::Cpu)
        .summary_line(SummaryLine::known("Fixture CPU (test data)"))
        .group(Group::new("Fixture CPU (test data)").kv("Cores", Value::known("24")))
}

/// Opening System paints what the specs workers have already published,
/// never the stale or empty view left from before the page was visible.
#[test]
fn opening_system_paints_published_specs_in_the_click_frame() {
    let mut monitor = crate::specs::Monitor::fixture(
        |id| (id == SectionId::Cpu).then_some(fixture_cpu as crate::specs::Provider),
        std::sync::Arc::new(|| {}),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while monitor.snapshot(std::time::Instant::now()).entries[..]
        .iter()
        .all(|entry| entry.section.is_none())
    {
        assert!(
            std::time::Instant::now() < deadline,
            "fixture worker never published"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let mut clock = Clock::new(1.0);
    let mut app = app(ThemeSettings::default(), true);
    app.specs = Some(monitor);
    // Never visible yet, so the view is still the empty default.
    app.specs_view = crate::specs::Snapshot::default();
    let (first, _) = open(&mut clock, &mut app, Page::Processes, "System");
    assert!(
        layout(&first)
            .iter()
            .any(|(text, _, _)| text.contains("Fixture CPU (test data)")),
        "the click frame painted a stale specs view: {:?}",
        layout(&first)
    );
}
