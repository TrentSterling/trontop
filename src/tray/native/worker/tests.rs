//! Uses real private Win32 wake events and this worker's message queue, but never
//! creates a tray icon/window, registers handlers, injects input or touches files.
use super::*;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

struct Release(Option<Sender<()>>);
impl Release {
    fn now(&mut self) {
        self.0.take();
    }
}
impl Drop for Release {
    fn drop(&mut self) {
        self.now();
    }
}

struct Fake {
    owner: thread::ThreadId,
    // Deliberately !Send. Like a TrayIcon, this object must stay on its owner.
    _local: std::rc::Rc<()>,
    applied: Sender<f32>,
    done: Sender<thread::ThreadId>,
    blocked: Option<Receiver<()>>,
    fail_once: bool,
}
impl Backend for Fake {
    fn update(&mut self, sample: TraySample) -> Result<(), ()> {
        assert_eq!(thread::current().id(), self.owner);
        self.applied.send(sample.cpu_percent).unwrap();
        if let Some(blocked) = self.blocked.take() {
            let _ = blocked.recv_timeout(Duration::from_secs(3));
        }
        if std::mem::take(&mut self.fail_once) {
            Err(())
        } else {
            Ok(())
        }
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.done.send(thread::current().id()).unwrap();
    }
}

fn sample(cpu: f32) -> TraySample {
    TraySample {
        cpu_percent: cpu,
        memory_percent: 40.0,
        gpu_percent: Some(10.0),
        process_count: 123,
    }
}

fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !condition() {
        assert!(Instant::now() < deadline, "owned tray worker timed out");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn tray_startup_returns_before_constructor_and_coalesces_latest_sample() {
    let (release, blocked) = mpsc::channel();
    let mut release = Release(Some(release));
    let (started, entered) = mpsc::channel();
    let (applied, samples) = mpsc::channel();
    let (done, dropped) = mpsc::channel();
    let start = Instant::now();
    let tray = TrayController::spawn(egui::Context::default(), move |_, _| {
        let owner = thread::current().id();
        started.send(owner).unwrap();
        let _ = blocked.recv_timeout(Duration::from_secs(3));
        Some(Fake {
            owner,
            _local: Default::default(),
            applied,
            done,
            blocked: None,
            fail_once: false,
        })
    })
    .unwrap();
    let elapsed = start.elapsed();
    let owner = entered.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(tray.state(), TrayState::Starting);
    assert_ne!(owner, thread::current().id());
    for n in 0..10_000 {
        tray.sink.publish(sample(n as f32));
    }
    assert_eq!(
        tray.sink.0.latest.lock().unwrap().unwrap().cpu_percent,
        9999.0
    );
    assert!(samples.try_recv().is_err());
    release.now();
    assert_eq!(
        samples.recv_timeout(Duration::from_secs(3)).unwrap(),
        9999.0
    );
    until(|| tray.applied() == 1);
    assert_eq!(tray.state(), TrayState::Ready);
    let shared = Arc::downgrade(&tray.sink.0);
    drop(tray);
    assert_eq!(dropped.recv_timeout(Duration::from_secs(3)).unwrap(), owner);
    until(|| shared.upgrade().is_none());
    assert!(samples.try_recv().is_err()); // Not a queue of 10,000 old updates.
    assert!(
        elapsed < Duration::from_secs(1),
        "constructor waited: {elapsed:?}"
    );
    println!(
        "Async tray controller returned in {elapsed:?}; 10,000 startup samples coalesced to one"
    );
}

#[test]
fn tray_close_during_startup_drops_late_backend_on_its_owner_without_updates() {
    let (release, blocked) = mpsc::channel();
    let mut release = Release(Some(release));
    let (started, entered) = mpsc::channel();
    let (applied, samples) = mpsc::channel();
    let (done, dropped) = mpsc::channel();
    let tray = TrayController::spawn(egui::Context::default(), move |_, _| {
        let owner = thread::current().id();
        started.send(owner).unwrap();
        let _ = blocked.recv_timeout(Duration::from_secs(3));
        Some(Fake {
            owner,
            _local: Default::default(),
            applied,
            done,
            blocked: None,
            fail_once: false,
        })
    })
    .unwrap();
    let owner = entered.recv_timeout(Duration::from_secs(3)).unwrap();
    let sink = tray.sink();
    sink.publish(sample(35.0));
    let start = Instant::now();
    drop(tray);
    let elapsed = start.elapsed();
    release.now();
    assert_eq!(dropped.recv_timeout(Duration::from_secs(3)).unwrap(), owner);
    until(|| sink.0.state.load(Ordering::Acquire) == TrayState::Stopped as u8);
    sink.publish(sample(90.0));
    assert_eq!(sink.0.latest.lock().unwrap().unwrap().cpu_percent, 35.0);
    assert!(samples.try_recv().is_err());
    assert!(
        elapsed < Duration::from_secs(2),
        "blocked startup joined: {elapsed:?}"
    );
}

#[test]
fn tray_failed_startup_is_unavailable_without_retrying_or_accepting_samples() {
    let (release, blocked) = mpsc::channel();
    let mut release = Release(Some(release));
    let (started, entered) = mpsc::channel();
    let tray = TrayController::spawn(egui::Context::default(), move |_, _| {
        started.send(()).unwrap();
        let _ = blocked.recv_timeout(Duration::from_secs(3));
        None::<Fake>
    })
    .unwrap();
    entered.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(tray.state(), TrayState::Starting);
    release.now();
    until(|| tray.state() == TrayState::Unavailable);
    for _ in 0..1000 {
        tray.sink.publish(sample(50.0));
        assert!(tray.poll().is_none());
    }
    assert!(tray.sink.0.latest.lock().unwrap().is_none());
    assert!(entered.try_recv().is_err());
    until(|| tray.worker.as_ref().unwrap().is_finished());
}

#[test]
fn tray_update_failure_recovers_without_ui_frames_and_actions_remain_single_shot() {
    let (release, blocked) = mpsc::channel();
    let mut release = Release(Some(release));
    let (applied, samples) = mpsc::channel();
    let (done, dropped) = mpsc::channel();
    let tray = TrayController::spawn(egui::Context::default(), move |_, _| {
        Some(Fake {
            owner: thread::current().id(),
            _local: Default::default(),
            applied,
            done,
            blocked: Some(blocked),
            fail_once: true,
        })
    })
    .unwrap();
    tray.sink.publish(sample(1.0));
    assert_eq!(samples.recv_timeout(Duration::from_secs(3)).unwrap(), 1.0);
    release.now();
    until(|| tray.state() == TrayState::UpdateFailed);
    assert_eq!(tray.applied(), 0);
    tray.sink.publish(sample(80.0));
    assert_eq!(samples.recv_timeout(Duration::from_secs(3)).unwrap(), 80.0);
    until(|| tray.applied() == 1 && tray.state() == TrayState::Ready);
    tray.sink.0.pending.store(1, Ordering::Release);
    assert_eq!(tray.poll(), Some(TrayAction::Show));
    assert_eq!(tray.poll(), None);
    tray.sink.0.pending.store(2, Ordering::Release);
    assert_eq!(tray.poll(), Some(TrayAction::Quit));
    drop(tray);
    dropped.recv_timeout(Duration::from_secs(3)).unwrap();
}

#[test]
fn tray_successful_samples_do_not_request_ui_repaints() {
    let ctx = egui::Context::default();
    let repaints = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counter = Arc::clone(&repaints);
    ctx.set_request_repaint_callback(move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    let (applied, samples) = mpsc::channel();
    let (done, dropped) = mpsc::channel();
    let tray = TrayController::spawn(ctx, move |_, _| {
        Some(Fake {
            owner: thread::current().id(),
            _local: Default::default(),
            applied,
            done,
            blocked: None,
            fail_once: false,
        })
    })
    .unwrap();
    until(|| tray.state() == TrayState::Ready);
    // Ensure the transition callback completed before taking the baseline.
    until(|| repaints.load(Ordering::Relaxed) > 0);
    let baseline = repaints.load(Ordering::Relaxed);
    for (index, cpu) in [10.0, 20.0, 20.0].into_iter().enumerate() {
        tray.sink.publish(sample(cpu));
        assert_eq!(samples.recv_timeout(Duration::from_secs(3)).unwrap(), cpu);
        until(|| tray.applied() == index as u64 + 1);
    }
    assert_eq!(repaints.load(Ordering::Relaxed), baseline);
    drop(tray);
    dropped.recv_timeout(Duration::from_secs(3)).unwrap();
}

#[test]
fn tray_message_wait_handles_already_observed_private_queue_input() {
    let (applied, samples) = mpsc::channel();
    let (done, dropped) = mpsc::channel();
    let tray = TrayController::spawn(egui::Context::default(), move |_, _| {
        let owner = thread::current().id();
        // This writes only to this disposable worker's own queue. No HWND,
        // target thread ID, desktop input or other application is involved.
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
            let mut message = MSG::default();
            assert!(PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE).as_bool());
            assert_eq!(message.message, WM_QUIT);
        }
        Some(Fake {
            owner,
            _local: Default::default(),
            applied,
            done,
            blocked: None,
            fail_once: false,
        })
    })
    .unwrap();
    // An old/seen queue message must wake the production message-aware wait,
    // even with no samples, stop signal, UI frame or native window.
    let exited = dropped.recv_timeout(Duration::from_secs(1));
    drop(tray); // Always wake/clean up before asserting a wait regression.
    assert!(exited.is_ok(), "already observed message was stranded");
    assert!(samples.try_recv().is_err());
}

#[test]
fn tray_blocked_update_keeps_one_pending_sample_and_bounded_drop() {
    let (release, blocked) = mpsc::channel();
    let mut release = Release(Some(release));
    let (applied, samples) = mpsc::channel();
    let (done, dropped) = mpsc::channel();
    let tray = TrayController::spawn(egui::Context::default(), move |_, _| {
        Some(Fake {
            owner: thread::current().id(),
            _local: Default::default(),
            applied,
            done,
            blocked: Some(blocked),
            fail_once: false,
        })
    })
    .unwrap();
    tray.sink.publish(sample(1.0));
    samples.recv_timeout(Duration::from_secs(3)).unwrap();
    for n in 0..10_000 {
        tray.sink.publish(sample(n as f32));
        assert!(tray.poll().is_none());
    }
    assert_eq!(
        tray.sink.0.latest.lock().unwrap().unwrap().cpu_percent,
        9999.0
    );
    let sink = tray.sink();
    let start = Instant::now();
    drop(tray);
    let elapsed = start.elapsed();
    release.now();
    dropped.recv_timeout(Duration::from_secs(3)).unwrap();
    until(|| sink.0.state.load(Ordering::Acquire) == TrayState::Stopped as u8);
    assert!(samples.try_recv().is_err());
    assert!(
        elapsed < Duration::from_secs(2),
        "blocked update joined: {elapsed:?}"
    );
}
