use super::*;
use std::hint::black_box;
use std::time::Instant;

fn workload(count: usize) -> TrontopApp {
    let mut app = app(ThemeSettings::default(), false);
    let mut sample = fixture();
    let template = sample.processes[0].clone();
    sample.processes = (0..count)
        .map(|index| {
            let mut row = template.clone();
            row.pid = 900_000 + index as u32;
            row.parent_pid = None;
            row.name = format!("Fixture.Process.{index:05}.exe");
            row.command = "--fixture-argument=synthetic-data ".repeat(64);
            row.cpu_percent = (index % 100) as f32 / 100.0;
            row.accumulated_cpu_millis = (index * 100) as u64;
            row
        })
        .collect();
    sample.process_count = count;
    app.accept_sample(sample);
    app
}

#[test]
fn indexed_views_follow_snapshot_reorder_replacement_filter_and_empty_state() {
    let mut app = workload(64);
    let original = app.snapshot.processes.clone();
    app.selected_pid = Some(original[7].pid);
    let mut snapshot = app.snapshot.clone();
    snapshot.processes.reverse();
    snapshot.sequence += 1;
    app.accept_sample(snapshot);
    assert_eq!(app.selected_process().unwrap().pid, original[7].pid);
    for &index in &app.visible_processes {
        assert!(index < app.snapshot.processes.len());
    }
    for row in &app.visible_process_tree {
        let source = &app.snapshot.processes[row.process_index];
        assert_eq!(source.cpu_percent, row.totals.cpu_percent); // All roots in this fixture.
    }
    assert_eq!(app.history_processes.len(), 12);
    assert_eq!(
        app.snapshot.processes[app.history_processes[0]].pid,
        original[63].pid
    );

    app.query = "Process.00007".into();
    app.rebuild_visible_processes();
    assert_eq!(app.visible_processes.len(), 1);
    assert_eq!(
        app.snapshot.processes[app.visible_processes[0]].pid,
        original[7].pid
    );
    assert_eq!(app.history_processes, app.visible_processes);

    // Selection must not follow a reused PID merely because it occupies the same slot.
    let mut replacement = app.snapshot.clone();
    let row = replacement
        .processes
        .iter_mut()
        .find(|r| r.pid == original[7].pid)
        .unwrap();
    row.started_at_unix += 1;
    row.control.created_at_100ns = row.control.created_at_100ns.map(|t| t + 10_000_000);
    app.accept_sample(replacement);
    assert!(app.selected_pid.is_none());
    app.accept_sample(SystemSnapshot::default());
    assert!(app.visible_processes.is_empty());
    assert!(app.visible_process_tree.is_empty());
    assert!(app.history_processes.is_empty());
    let ctx = egui::Context::default();
    for page in [Page::Processes, Page::Details, Page::History] {
        app.page = page;
        frame(&ctx, &mut app, Vec2::new(1040.0, 640.0), vec![]);
    }
}

#[test]
fn painting_reuses_index_storage_and_history_order_without_rebuilding_it() {
    let mut app = workload(5000);
    let storage = (
        app.visible_processes.as_ptr(),
        app.visible_process_tree.as_ptr(),
        app.history_processes.as_ptr(),
    );
    let history = app.history_processes.clone();
    let ctx = egui::Context::default();
    for page in [Page::Processes, Page::Details, Page::History] {
        app.page = page;
        for _ in 0..3 {
            frame(&ctx, &mut app, Vec2::new(1040.0, 640.0), vec![]);
        }
        assert_eq!(
            storage,
            (
                app.visible_processes.as_ptr(),
                app.visible_process_tree.as_ptr(),
                app.history_processes.as_ptr()
            )
        );
        assert_eq!(history, app.history_processes);
    }
    // Tree metadata is Copy and owns no strings or paths, unlike a process record.
    fn require_copy<T: Copy>() {}
    require_copy::<ProcessTreeRow>();
}

#[test]
#[ignore = "headless synthetic process-view timing; no native window, providers or input"]
fn process_view_timing_probe() {
    for count in [500, 5000] {
        let mut app = workload(count);
        let ctx = egui::Context::default();
        theme::install(&ctx, app.theme);
        let size = Vec2::new(1280.0, 760.0);
        let at = Instant::now();
        for _ in 0..20 {
            app.rebuild_visible_processes();
            black_box(&app.visible_process_tree);
        }
        println!(
            "PROCESS_VIEW count={count} rebuild_us={:.1}",
            at.elapsed().as_secs_f64() * 1e6 / 20.0
        );
        for (label, page, tree) in [
            ("tree", Page::Processes, true),
            ("flat", Page::Processes, false),
            ("details", Page::Details, false),
            ("history", Page::History, false),
        ] {
            app.page = page;
            app.tree_mode = tree;
            for _ in 0..5 {
                black_box(frame(&ctx, &mut app, size, vec![]));
            }
            let mut times = Vec::new();
            for _ in 0..60 {
                let at = Instant::now();
                black_box(frame(&ctx, &mut app, size, vec![]));
                times.push(at.elapsed().as_secs_f64() * 1e6);
            }
            times.sort_by(f64::total_cmp);
            println!(
                "PROCESS_VIEW count={count} page={label} median_us={:.1} p95_us={:.1}",
                times[30], times[57]
            );
        }
    }
}
