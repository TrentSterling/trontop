use super::*;

fn identity() -> ProcessIdentity {
    ProcessIdentity {
        pid: 900_001,
        created_at_100ns: 123_456_789,
    }
}

fn wait(controller: &mut Controller) -> Outcome {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(outcome) = controller.poll() {
            return outcome;
        }
        assert!(Instant::now() < deadline, "action did not complete");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn actions_dispatch_exact_arguments_off_the_calling_thread() {
    let (sent, observed) = mpsc::channel();
    let ui_thread = thread::current().id();
    let mut controller = Controller::with_backend(Context::default(), move |action| {
        sent.send((action.clone(), thread::current().id())).unwrap();
        Ok(())
    });
    for action in [
        Action::End(identity()),
        Action::Priority(identity(), PriorityClass::BelowNormal),
        Action::Affinity(identity(), 3),
        Action::Reveal(PathBuf::from("C:/Fixture/应用.exe")),
        Action::Launch("fixture program.exe --not-actually-run".into()),
    ] {
        assert!(controller.ready());
        controller
            .submit(Request::new(action.clone(), "Frozen target".into()))
            .unwrap();
        let (received, worker_thread) = observed.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(received, action);
        assert_ne!(worker_thread, ui_thread);
        let outcome = wait(&mut controller);
        assert_eq!(outcome.request.action, action);
        assert_eq!(outcome.request.target, "Frozen target");
        let (message, error) = outcome.message();
        assert!(!error);
        assert!(message.contains("Request accepted"));
        assert!(!controller.busy());
        assert!(controller.poll().is_none());
    }
}

#[test]
fn stalled_action_has_no_duplicates_no_blocking_poll_and_no_shutdown_join() {
    let (release, blocked) = mpsc::channel();
    let (entered, started) = mpsc::channel();
    let (finished, done) = mpsc::channel();
    let mut controller = Controller::with_backend(Context::default(), move |_| {
        entered.send(()).unwrap();
        blocked.recv().unwrap();
        finished.send(()).unwrap();
        Ok(())
    });
    let request = Request::new(Action::End(identity()), "Fixture.exe".into());
    controller.submit(request.clone()).unwrap();
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    let started_poll = Instant::now();
    for _ in 0..1_000 {
        assert!(controller.poll().is_none());
        assert!(controller.busy());
        assert!(!controller.ready());
    }
    let poll_elapsed = started_poll.elapsed();
    assert!(controller.submit(request).is_err());
    // A slow-warning threshold is not permission to clear active or resubmit.
    controller.active.as_mut().unwrap().confirmed_at =
        Instant::now() - SLOW_AFTER - Duration::from_secs(1);
    assert!(controller.busy());
    let drop_started = Instant::now();
    drop(controller);
    let drop_elapsed = drop_started.elapsed();
    release.send(()).unwrap();
    done.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(poll_elapsed < Duration::from_millis(100));
    assert!(drop_elapsed < Duration::from_millis(100));
    println!(
        "ACTION_STALL 1000_polls_us={:.1} drop_us={:.1}; one in-flight call, no native API",
        poll_elapsed.as_secs_f64() * 1e6,
        drop_elapsed.as_secs_f64() * 1e6
    );
}

#[test]
fn disconnected_action_reports_unknown_once_and_cannot_restart_itself() {
    let mut controller = Controller::with_backend(Context::default(), |_| {
        panic!("injected action worker failure");
    });
    controller
        .submit(Request::new(Action::End(identity()), "Fixture.exe".into()))
        .unwrap();
    let outcome = wait(&mut controller);
    assert!(outcome.result.unwrap_err().contains("Outcome is unknown"));
    assert!(!controller.ready());
    assert!(!controller.busy());
    assert!(controller.poll().is_none());
    assert!(
        controller
            .submit(Request::new(Action::End(identity()), "Fixture.exe".into()))
            .is_err()
    );
}

#[test]
fn default_invalid_and_expired_requests_cannot_execute() {
    let mut disabled = Controller::default();
    assert!(!disabled.ready());
    assert!(
        disabled
            .submit(Request::new(Action::End(identity()), "Fixture".into()))
            .is_err()
    );
    let mut controller =
        Controller::with_backend(Context::default(), |_| panic!("must not execute"));
    for action in [
        Action::End(ProcessIdentity {
            pid: 4,
            ..identity()
        }),
        Action::Priority(identity(), PriorityClass::Realtime),
        Action::Affinity(identity(), 0),
        Action::Launch("  ".into()),
        Action::Launch("nul\0command".into()),
        Action::Reveal(PathBuf::new()),
    ] {
        assert!(
            controller
                .submit(Request::new(action, "Invalid".into()))
                .is_err()
        );
    }
    let expired = Request {
        confirmed_at: Instant::now() - START_DEADLINE - Duration::from_secs(1),
        ..Request::new(Action::End(identity()), "Expired".into())
    };
    assert!(
        controller
            .submit(expired)
            .unwrap_err()
            .contains("30 seconds")
    );
    assert!(!controller.busy());
}

#[test]
fn failed_action_can_be_followed_by_a_new_explicit_request() {
    let mut attempts = 0;
    let mut controller = Controller::with_backend(Context::default(), move |_| {
        attempts += 1;
        if attempts == 1 {
            Err("injected access denied".into())
        } else {
            Ok(())
        }
    });
    for expected_ok in [false, true] {
        controller
            .submit(Request::new(
                Action::Priority(identity(), PriorityClass::Normal),
                "Captured".into(),
            ))
            .unwrap();
        let outcome = wait(&mut controller);
        assert_eq!(outcome.result.is_ok(), expected_ok);
        assert!(controller.ready());
    }
}

#[cfg(windows)]
#[test]
fn native_action_worker_rechecks_identity_on_its_owned_hidden_child() {
    use std::os::windows::process::CommandExt;
    struct Owned(std::process::Child);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Owned(
        std::process::Command::new("ping.exe")
            .args(["-t", "127.0.0.1"])
            .creation_flags(0x0800_0000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let before = platform::query_process_control(child.0.id()).unwrap();
    let identity = ProcessIdentity {
        pid: child.0.id(),
        created_at_100ns: before.created_at_100ns.unwrap(),
    };
    let wrong = ProcessIdentity {
        created_at_100ns: identity.created_at_100ns + 1,
        ..identity
    };
    let mut controller = Controller::spawn(Context::default());
    for action in [
        Action::End(wrong),
        Action::Priority(wrong, PriorityClass::BelowNormal),
        Action::Affinity(wrong, 1),
    ] {
        controller
            .submit(Request::new(action, "Owned hidden test child".into()))
            .unwrap();
        assert!(
            wait(&mut controller)
                .result
                .unwrap_err()
                .contains("different process")
        );
        assert!(child.0.try_wait().unwrap().is_none());
    }
    let after = platform::query_process_control(child.0.id()).unwrap();
    assert_eq!(after.priority, before.priority);
    assert_eq!(after.affinity_mask, before.affinity_mask);
    controller
        .submit(Request::new(
            Action::End(identity),
            "Owned hidden test child".into(),
        ))
        .unwrap();
    assert!(wait(&mut controller).result.is_ok());
    let deadline = Instant::now() + Duration::from_secs(3);
    while child.0.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "owned child did not terminate");
        thread::sleep(Duration::from_millis(1));
    }
}
