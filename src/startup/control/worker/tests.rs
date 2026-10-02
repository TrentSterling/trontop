use super::*;
use crate::startup::control::{Action, Approval, Control, RawValue, Registration};
use std::time::{Duration, Instant};
fn request() -> Request {
    Request::new(
        crate::model::StartupRow {
            key: "Owned".into(),
            name: "Fixture".into(),
            command: "fixture.exe".into(),
            source: crate::startup::Source::UserRun,
            control: Some(Control {
                registration: Some(Registration::Run(RawValue {
                    kind: 1,
                    bytes: vec![],
                })),
                approval: Approval::Missing,
            }),
        },
        Action::Disable,
    )
}
fn wait(controller: &mut Controller) -> Outcome {
    let start = Instant::now();
    loop {
        if let Some(outcome) = controller.poll() {
            return outcome;
        }
        assert!(start.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn single_flight_poll_and_drop_never_wait_for_the_backend() {
    let (entered, started) = mpsc::channel();
    let (release, blocked) = mpsc::channel();
    let mut controller = Controller::with_backend(Default::default(), move |_| {
        entered.send(()).unwrap();
        blocked.recv().unwrap();
        Err(Failure::unchanged("fixture denied"))
    });
    assert!(controller.ready());
    controller.submit(request()).unwrap();
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(controller.busy());
    assert!(!controller.ready());
    assert!(controller.submit(request()).is_err());
    let at = Instant::now();
    assert!(controller.poll().is_none());
    drop(controller);
    assert!(at.elapsed() < Duration::from_millis(100));
    release.send(()).unwrap();
}
#[test]
fn completed_failure_is_reported_once_and_a_panicked_worker_never_retries() {
    let mut controller = Controller::with_backend(Default::default(), |_| {
        Err(Failure::unchanged("permission denied"))
    });
    controller.submit(request()).unwrap();
    assert!(!wait(&mut controller).result.unwrap_err().uncertain);
    assert!(!controller.busy());
    assert!(controller.poll().is_none());
    let mut controller =
        Controller::with_backend(Default::default(), |_| panic!("owned fixture panic"));
    controller.submit(request()).unwrap();
    assert!(wait(&mut controller).result.unwrap_err().uncertain);
    assert!(controller.poll().is_none());
    assert!(controller.submit(request()).is_err());
    assert!(Controller::default().submit(request()).is_err());
}
#[test]
fn stale_request_and_publication_contention_do_not_call_or_block_the_ui() {
    let mut controller = Controller::with_backend(Default::default(), |_| {
        panic!("expired request must not reach backend")
    });
    let mut stale = request();
    stale.confirmed_at -= Duration::from_secs(31);
    assert!(controller.submit(stale).unwrap_err().contains("expired"));
    let shared = Arc::clone(&controller.latest);
    let _lock = shared.lock().unwrap();
    let at = Instant::now();
    assert!(controller.poll().is_none());
    assert!(at.elapsed() < Duration::from_millis(100));
}
