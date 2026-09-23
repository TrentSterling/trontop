use super::*;
use std::collections::VecDeque;

fn status(state: State) -> Status {
    Status {
        state,
        pid: if state == State::Stopped { 0 } else { 12345 },
        accepts_stop: true,
        win32_service: true,
        ..Default::default()
    }
}
fn request(action: Action) -> Request {
    Request {
        name: "FixtureService".into(),
        display_name: "Fixture service".into(),
        action,
        expected: status(if action == Action::Start {
            State::Stopped
        } else {
            State::Running
        }),
        staged_at: Instant::now(),
    }
}
struct Fake {
    states: VecDeque<Result<Status, String>>,
    last: Status,
    calls: Arc<Mutex<Vec<&'static str>>>,
    fail_action: Option<Action>,
    close_on_stop: Option<Arc<AtomicBool>>,
}
impl Service for Fake {
    fn query(&mut self) -> Result<Status, String> {
        self.calls.lock().unwrap().push("query");
        if let Some(next) = self.states.pop_front() {
            self.last = next?;
        }
        Ok(self.last)
    }
    fn start(&mut self) -> Result<(), String> {
        self.calls.lock().unwrap().push("start");
        if self.fail_action == Some(Action::Start) {
            return Err("Fixture start denied".into());
        }
        self.last = status(State::Running);
        Ok(())
    }
    fn stop(&mut self) -> Result<(), String> {
        self.calls.lock().unwrap().push("stop");
        if self.fail_action == Some(Action::Stop) {
            return Err("Fixture dependent services running".into());
        }
        if let Some(stop) = &self.close_on_stop {
            stop.store(true, Ordering::Release);
        }
        self.last = status(State::Stopped);
        Ok(())
    }
}
struct FakeBackend {
    service: Option<Fake>,
    opens: usize,
}
impl Backend for FakeBackend {
    fn open(&mut self, _: &Request) -> Result<Box<dyn Service>, String> {
        self.opens += 1;
        self.service
            .take()
            .map(|s| Box::new(s) as Box<dyn Service>)
            .ok_or("Fixture open denied".into())
    }
}
fn backend(initial: Status) -> (FakeBackend, Arc<Mutex<Vec<&'static str>>>) {
    let calls = Arc::new(Mutex::new(vec![]));
    (
        FakeBackend {
            service: Some(Fake {
                states: VecDeque::new(),
                last: initial,
                calls: Arc::clone(&calls),
                fail_action: None,
                close_on_stop: None,
            }),
            opens: 0,
        },
        calls,
    )
}
fn run(backend: &mut FakeBackend, request: &Request, stop: &AtomicBool) -> Event {
    let mut events = vec![];
    execute(backend, request, stop, Duration::ZERO, &mut |event| {
        events.push(event)
    });
    let final_event = events.pop().unwrap();
    assert!(final_event.done);
    final_event
}

#[test]
fn commands_observe_target_state_and_restart_uses_one_handle() {
    for action in Action::ALL {
        let request = request(action);
        let (mut backend, calls) = backend(request.expected);
        let event = run(&mut backend, &request, &AtomicBool::new(false));
        assert!(event.error.is_none(), "{:?}", event.error);
        assert_eq!(backend.opens, 1);
        assert_eq!(
            event.observed.unwrap().1.state,
            if action == Action::Stop {
                State::Stopped
            } else {
                State::Running
            }
        );
        let calls = calls.lock().unwrap();
        let mutations: Vec<_> = calls.iter().copied().filter(|s| *s != "query").collect();
        assert_eq!(
            mutations,
            match action {
                Action::Start => vec!["start"],
                Action::Stop => vec!["stop"],
                Action::Restart => vec!["stop", "start"],
            }
        );
    }
}

#[test]
fn changed_state_pid_controls_and_protected_hosts_refuse_before_mutation() {
    for current in [
        Status {
            pid: 999,
            ..status(State::Running)
        },
        status(State::Stopped),
        Status {
            accepts_stop: false,
            ..status(State::Running)
        },
        Status {
            win32_service: false,
            ..status(State::Running)
        },
        Status {
            system_process: true,
            ..status(State::Running)
        },
    ] {
        let (mut backend, calls) = backend(current);
        let event = run(
            &mut backend,
            &request(Action::Restart),
            &AtomicBool::new(false),
        );
        assert!(event.error.is_some());
        assert!(calls.lock().unwrap().iter().all(|call| *call == "query"));
    }
}

#[test]
fn invalid_expired_and_cancelled_requests_never_open_services() {
    for name in ["", "Bad\0Suffix", "path/name", "path\\name"] {
        let mut request = request(Action::Start);
        request.name = name.into();
        let (mut backend, _) = backend(request.expected);
        assert!(
            run(&mut backend, &request, &AtomicBool::new(false))
                .error
                .is_some()
        );
        assert_eq!(backend.opens, 0);
    }
    let mut expired = request(Action::Start);
    expired.staged_at = Instant::now() - Duration::from_secs(31);
    let (mut backend, _) = backend(expired.expected);
    assert!(
        run(&mut backend, &expired, &AtomicBool::new(false))
            .error
            .is_some()
    );
    assert_eq!(backend.opens, 0);
    assert!(
        run(
            &mut backend,
            &request(Action::Start),
            &AtomicBool::new(true)
        )
        .error
        .is_some()
    );
    assert_eq!(backend.opens, 0);
}

#[test]
fn restart_does_not_start_after_denied_stop_timeout_or_app_shutdown() {
    for scenario in 0..3 {
        let stop = Arc::new(AtomicBool::new(false));
        let request = request(Action::Restart);
        let (mut backend, calls) = backend(request.expected);
        let fake = backend.service.as_mut().unwrap();
        match scenario {
            0 => fake.fail_action = Some(Action::Stop),
            1 => fake.states = [Ok(status(State::Running)), Ok(status(State::Stopping))].into(),
            _ => fake.close_on_stop = Some(Arc::clone(&stop)),
        }
        let event = run(&mut backend, &request, &stop);
        let error = event.error.unwrap();
        assert!(error.contains("may remain stopped"));
        if scenario == 1 {
            assert!(error.contains("Timed out"));
        }
        assert!(!calls.lock().unwrap().contains(&"start"));
    }
}

#[test]
fn restart_reports_start_failure_and_failed_observation_does_not_claim_success() {
    let request = request(Action::Restart);
    let (mut backend, calls) = backend(request.expected);
    backend.service.as_mut().unwrap().fail_action = Some(Action::Start);
    let event = run(&mut backend, &request, &AtomicBool::new(false));
    assert!(event.error.unwrap().contains("may remain stopped"));
    assert_eq!(event.observed.unwrap().1.state, State::Stopped);
    assert!(calls.lock().unwrap().contains(&"start"));
    let (mut backend, _) = self::backend(status(State::Stopped));
    backend.service.as_mut().unwrap().states = [
        Ok(status(State::Stopped)),
        Ok(status(State::Stopped)),
        Err("Fixture query failed".into()),
    ]
    .into();
    assert!(
        run(
            &mut backend,
            &self::request(Action::Start),
            &AtomicBool::new(false)
        )
        .error
        .unwrap()
        .contains("query failed")
    );
}

#[test]
fn competing_start_between_restart_phases_is_not_started_twice() {
    let (mut backend, calls) = backend(status(State::Running));
    backend.service.as_mut().unwrap().states = [
        Ok(status(State::Running)),
        Ok(status(State::Stopped)),
        Ok(status(State::Running)),
    ]
    .into();
    assert!(
        run(
            &mut backend,
            &request(Action::Restart),
            &AtomicBool::new(false)
        )
        .error
        .is_some()
    );
    assert!(!calls.lock().unwrap().contains(&"start"));
}

#[test]
fn worker_is_single_flight_and_completion_releases_it() {
    let (backend, calls) = backend(status(State::Running));
    let mut controller = Controller::with_backend(eframe::egui::Context::default(), backend);
    assert!(controller.available());
    controller.submit(request(Action::Stop)).unwrap();
    assert!(controller.submit(request(Action::Stop)).is_err());
    let started = Instant::now();
    while controller.busy() {
        controller.poll();
        assert!(started.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| **call == "stop")
            .count(),
        1
    );
    assert!(Controller::default().submit(request(Action::Stop)).is_err());
}

#[test]
fn blocked_native_call_does_not_block_controller_drop() {
    struct BlockedBackend {
        entered: SyncSender<()>,
        unblock: mpsc::Receiver<()>,
    }
    impl Backend for BlockedBackend {
        fn open(&mut self, _: &Request) -> Result<Box<dyn Service>, String> {
            self.entered.send(()).unwrap();
            self.unblock.recv().unwrap();
            Err("Fixture released blocked open".into())
        }
    }
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (unblock_tx, unblock_rx) = mpsc::sync_channel(1);
    let mut controller = Controller::with_backend(
        eframe::egui::Context::default(),
        BlockedBackend {
            entered: entered_tx,
            unblock: unblock_rx,
        },
    );
    controller.submit(request(Action::Restart)).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let started = Instant::now();
    drop(controller);
    let elapsed = started.elapsed();
    unblock_tx.send(()).unwrap();
    // The backend stays blocked until after this measurement, so any bound
    // proves drop did not join it; 2 s only absorbs a loaded test run.
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

#[test]
fn busy_result_mailbox_cannot_block_ui_poll_or_lose_completion() {
    let mut controller = Controller {
        active: Some(request(Action::Stop)),
        requests: None,
        latest: Arc::new(std::sync::Mutex::new(None)),
        stop: Arc::new(AtomicBool::new(false)),
        worker: None,
    };
    let mailbox = Arc::clone(&controller.latest);
    let mut held = mailbox.lock().unwrap();
    *held = Some(Event {
        name: "FixtureService".into(),
        action: Action::Stop,
        phase: "Completed",
        observed: None,
        command_at: Some(Instant::now()),
        done: true,
        error: None,
    });
    let (sent, received) = mpsc::channel();
    let poller = thread::spawn(move || {
        let event = controller.poll();
        sent.send((controller, event)).unwrap();
    });
    // Release even when regressing to lock(), so a failing check cannot hang CI.
    let early = received.recv_timeout(Duration::from_secs(2));
    let was_nonblocking = early.is_ok();
    drop(held);
    let (mut controller, event) = early
        .or_else(|_| received.recv_timeout(Duration::from_secs(3)))
        .unwrap();
    poller.join().unwrap();
    assert!(
        was_nonblocking,
        "UI poll waited for the worker publication lock"
    );
    assert!(event.is_none());
    assert!(controller.busy());
    let event = controller.poll().expect("held completion was lost");
    assert!(event.done);
    assert_eq!(event.phase, "Completed");
    assert!(!controller.busy());
    assert!(controller.poll().is_none());
}

// Used only by the local egui fixture harness. No native service handles exist.
pub(crate) fn fixture_controller(ctx: eframe::egui::Context, initial: Status) -> Controller {
    Controller::with_backend(ctx, backend(initial).0)
}

#[test]
fn close_at_dispatch_boundary_prevents_the_remaining_mutation() {
    for phase in ["Sending Stop", "Sending Start"] {
        let (mut backend, calls) = backend(status(State::Running));
        let stop = AtomicBool::new(false);
        let mut final_event = None;
        execute(
            &mut backend,
            &request(Action::Restart),
            &stop,
            Duration::ZERO,
            &mut |event| {
                if event.phase == phase {
                    stop.store(true, Ordering::Release);
                }
                if event.done {
                    final_event = Some(event);
                }
            },
        );
        assert!(final_event.unwrap().error.is_some());
        let calls = calls.lock().unwrap();
        assert!(!calls.contains(&"start"));
        if phase == "Sending Stop" {
            assert!(!calls.contains(&"stop"));
        }
    }
}

#[test]
fn stopped_during_start_reports_service_exit_codes_not_completion() {
    let (mut backend, _) = backend(status(State::Stopped));
    backend.service.as_mut().unwrap().states = [
        Ok(status(State::Stopped)),
        Ok(status(State::Stopped)),
        Ok(Status {
            exit_code: 1066,
            service_exit_code: 42,
            ..status(State::Stopped)
        }),
    ]
    .into();
    let event = run(
        &mut backend,
        &request(Action::Start),
        &AtomicBool::new(false),
    );
    let error = event.error.unwrap();
    assert!(error.contains("1066") && error.contains("42"));
}
