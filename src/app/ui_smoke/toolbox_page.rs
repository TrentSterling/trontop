use super::*;
use std::path::Path;
use std::time::{Duration, Instant};

fn open_map(ctx: &egui::Context, app: &mut TrontopApp, size: Vec2) -> egui::FullOutput {
    app.page = Page::Toolbox;
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    app.toolbox.open_folder(ctx, src);
    let mut output = egui::FullOutput::default();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        output.append(frame(ctx, app, size, vec![]));
        if text_shapes(&output)
            .iter()
            .any(|(text, _)| text.galley.job.text.contains("links skipped"))
        {
            return output;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out scanning the Trontop source folder"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn toolbox_tabs_and_rescan_work_through_the_native_app_view() {
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, false);
    let size = Vec2::new(1000.0, 580.0);
    open_map(&ctx, &mut app, size);
    for (tab, expected) in [
        ("File types", ".rs"),
        ("Duplicates", "Check for duplicates"),
        (
            "Build caches",
            "No matching project and generated folder pairs.",
        ),
        (
            "Changes",
            "Scan this folder again to compare it with this result.",
        ),
        ("Cleaner", "Preview cleanup"),
        #[cfg(windows)]
        ("Installed Apps", "Refresh app list"),
        #[cfg(windows)]
        ("Maintenance", "Run health check"),
        ("Drive watch", "Mount history"),
        ("File users", "Find processes"),
    ] {
        click_local_text(&ctx, &mut app, size, tab);
        let output = frame(&ctx, &mut app, size, vec![]);
        assert!(
            text_shapes(&output)
                .iter()
                .any(|(t, clip)| t.galley.job.text == expected
                    && clip.contains(t.visual_bounding_rect().center())),
            "{tab}: {expected}"
        );
    }
    click_local_text(&ctx, &mut app, size, "SpaceView");
    click_local_text(&ctx, &mut app, size, "Scan folder");
    let deadline = Instant::now() + Duration::from_secs(15);
    while !text_shapes(&frame(&ctx, &mut app, size, vec![]))
        .iter()
        .any(|(t, _)| t.galley.job.text == "No size changes.")
    {
        assert!(
            Instant::now() < deadline,
            "Rescan did not publish its comparison"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(windows)]
#[test]
fn maintenance_navigation_and_schedule_confirmation_are_reachable_without_desktop_input() {
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, false);
    app.page = Page::Toolbox;
    let size = Vec2::new(1120.0, 680.0);
    click_local_text(&ctx, &mut app, size, "Maintenance");
    for (tab, expected) in [
        ("Updates", "Check WinGet updates"),
        ("Browsers", "Scan installed extensions"),
        ("Registry", "Scan app paths"),
        ("Restore points", "List restore points"),
        ("Drive tools", "Wipe free space"),
        ("Drivers", "List installed drivers"),
        ("Cloud storage", "OneDrive storage"),
        ("Performance", "Rank by memory"),
        ("Schedule", "Review schedule"),
    ] {
        click_local_text(&ctx, &mut app, size, tab);
        let output = frame(&ctx, &mut app, size, vec![]);
        assert!(
            text_shapes(&output)
                .iter()
                .any(|(s, clip)| s.galley.job.text == expected
                    && clip.contains(s.visual_bounding_rect().center())),
            "{tab}: {expected}"
        );
    }
    click_local_text(&ctx, &mut app, size, "Review schedule");
    click_local_text(&ctx, &mut app, size, "Cancel");
    assert!(!app.toolbox.busy());
}

#[cfg(windows)]
#[test]
fn minimized_sleep_review_can_be_cancelled_and_manual_actions_respect_its_owner() {
    use crate::platform::minimized::{Entry, Status};
    use crate::process_actions::{Action, Controller};
    let ctx = egui::Context::default();
    let settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    app.page = Page::Toolbox;
    app.process_actions =
        Controller::with_backend(ctx.clone(), |_| panic!("no native process action expected"));
    let size = Vec2::new(1280.0, 800.0);
    click_local_text(&ctx, &mut app, size, "Maintenance");
    click_local_text(&ctx, &mut app, size, "Performance");
    click_local_text(&ctx, &mut app, size, "Sleep when minimized");
    let render = frame(&ctx, &mut app, size, vec![]);
    assert!(
        text_shapes(&render)
            .iter()
            .any(|(s, _)| s.galley.job.text == "Enable sleep rule")
    );
    click_local_text(&ctx, &mut app, size, "Cancel");
    assert!(app.toolbox.sleep_rules.entries.is_empty());
    let id = app
        .snapshot
        .processes
        .iter()
        .find_map(|p| p.identity())
        .unwrap();
    app.toolbox.sleep_rules.entries.push(Entry {
        id,
        name: "fixture".into(),
        status: Status::Sleeping,
    });
    let row = app
        .snapshot
        .processes
        .iter()
        .find(|p| p.identity() == Some(id))
        .unwrap();
    assert_eq!(app.observed_process_state(row), "Suspended");
    assert!(!app.submit_process_action(&ctx, Action::Resume(id), "fixture".into()));
    assert!(app.message.as_ref().unwrap().0.contains("sleep rule"));
}

#[test]
#[ignore = "offscreen GPU render of a real folder scan"]
fn render_toolbox_spaceview() {
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, false);
    let size = Vec2::new(1280.0, 800.0);
    let mut output = open_map(&ctx, &mut app, size);
    for _ in 0..5 {
        output.append(frame(&ctx, &mut app, size, vec![]));
    }
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke/toolbox");
    std::fs::create_dir_all(&directory).unwrap();
    offscreen::Renderer::new().save(&ctx, output, size, &directory.join("spaceview.png"));
}
