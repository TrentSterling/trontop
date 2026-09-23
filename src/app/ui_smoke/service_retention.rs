use super::*;
use crate::diagnostics::{Issue, Provider, State as ProviderState};
use crate::service_control::{Action, Event, State, Status};
use std::time::{Duration, Instant};

pub(super) fn retained_fixture(app: &mut TrontopApp) {
    let at = Instant::now();
    for index in 0..3 {
        let event = Event {
            name: app.snapshot.services[index].name.clone(),
            action: Action::Stop,
            phase: if index == 1 {
                "Not completed"
            } else {
                "Completed"
            },
            command_at: Some(at),
            observed: (index != 1).then_some((
                at,
                Status {
                    state: State::Stopped,
                    pid: 0,
                    ..app.snapshot.services[index].status
                },
            )),
            done: true,
            error: (index == 1).then(|| "Fixture unknown outcome".into()),
        };
        app.service_observations.record(&event);
        app.service_event = Some(event);
    }
    app.selected_service = Some(app.snapshot.services[1].name.clone());
}

#[test]
fn retained_service_results_reach_rows_after_another_service_command_and_failed_refresh() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    fixture_service_controls(&mut app, &ctx);
    retained_fixture(&mut app);
    let at = Instant::now();
    app.snapshot.diagnostics.get_mut(Provider::Services).record(
        at,
        Duration::ZERO,
        ProviderState::Unavailable,
        None,
        Some(Issue::ServiceQuery),
    );
    let output = frame(&ctx, &mut app, Vec2::new(1280.0, 760.0), vec![]);
    let text = text_shapes(&output)
        .into_iter()
        .map(|(text, _)| text.galley.job.text.clone())
        .collect::<Vec<_>>();
    // STATE and PID are separate columns now; a service with no PID leaves
    // that column blank instead of a trailing "| -".
    assert!(
        text.iter()
            .filter(|value| value.as_str() == "Stopped")
            .count()
            >= 2
    );
    assert!(!text.iter().any(|value| value.as_str() == "Stopped | -"));
    assert!(
        text.iter()
            .filter(|value| value.as_str() == "Command read")
            .count()
            >= 2
    );
    assert!(!app.service_is_fresh(&app.snapshot.services[1]));
    click_local_text(&ctx, &mut app, Vec2::new(1280.0, 760.0), "Restart");
    assert!(app.pending_service.is_none());
    assert!(output.platform_output.commands.is_empty());
    app.snapshot.diagnostics.get_mut(Provider::Services).record(
        Instant::now(),
        Duration::ZERO,
        ProviderState::Live,
        None,
        None,
    );
    frame(&ctx, &mut app, Vec2::new(1280.0, 760.0), vec![]);
    assert!(app.service_is_fresh(&app.snapshot.services[1]));
    assert!(!app.service_status(&app.snapshot.services[0]).1);
}

#[test]
fn service_history_capacity_is_explained_and_does_not_dispatch() {
    let ctx = egui::Context::default();
    let mut app = app(ThemeSettings::default(), true);
    fixture_service_controls(&mut app, &ctx);
    let at = Instant::now();
    for index in 0..256 {
        app.service_observations.record(&Event {
            name: format!("OtherFixture{index}"),
            action: Action::Stop,
            phase: "Not completed",
            command_at: Some(at),
            observed: None,
            done: true,
            error: None,
        });
    }
    let output = frame(&ctx, &mut app, Vec2::new(1040.0, 640.0), vec![]);
    assert!(
        text_shapes(&output).iter().any(|(text, _)| text
            .galley
            .job
            .text
            .contains("Command history is full"))
    );
    click_local_text(&ctx, &mut app, Vec2::new(1040.0, 640.0), "Restart");
    assert!(app.pending_service.is_none());
    assert!(!app.service_controller.busy());
}

#[test]
fn inventory_timeouts_keep_cached_rows_and_table_header_geometry() {
    for page in [Page::Startup, Page::Services] {
        for dark in [true, false] {
            let settings = ThemeSettings {
                dark,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut app = app(settings, true);
            app.page = page;
            let size = Vec2::new(1040.0, 640.0);
            let label = if page == Page::Startup {
                "NAME"
            } else {
                "DISPLAY NAME"
            };
            let provider = if page == Page::Startup {
                Provider::Startup
            } else {
                Provider::Services
            };
            let anchor = |output: &egui::FullOutput| {
                text_shapes(output)
                    .iter()
                    .find(|(text, _)| text.galley.job.text == label)
                    .unwrap()
                    .0
                    .visual_bounding_rect()
            };
            let before = anchor(&frame(&ctx, &mut app, size, vec![]));
            for issue in [Issue::InventoryTimeout, Issue::InventoryWorker] {
                inventory_state_fixture(&mut app, page, 2);
                let health = app.snapshot.diagnostics.get_mut(provider);
                health.issue = Some(issue);
                health.query_millis = None;
                let output = frame(&ctx, &mut app, size, vec![]);
                assert_eq!(anchor(&output), before);
                assert!(app.snapshot.startup.rows().count() > 0);
                assert!(!app.snapshot.services.is_empty());
                assert!(
                    text_shapes(&output).iter().any(|(text, _)| text
                        .galley
                        .job
                        .text
                        .contains("Cached"))
                );
            }
        }
    }
}
