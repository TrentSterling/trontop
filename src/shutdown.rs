//! Shutdown never waits indefinitely for an OS/driver call. Worker closures own
//! their resources; detaching a still-running handle does not free those resources.
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub fn finish(worker: JoinHandle<()>, budget: Duration) {
    let deadline = Instant::now() + budget;
    while !worker.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    if worker.is_finished() {
        let _ = worker.join();
    }
    // Otherwise detach. The process may exit normally even if a driver never
    // returns. Do not force-terminate a thread or destroy its in-flight buffers.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocked_worker_cannot_hold_up_shutdown() {
        let (release, blocked) = std::sync::mpsc::channel();
        let (done, completed) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            blocked.recv().unwrap();
            done.send(()).unwrap();
        });
        let start = Instant::now();
        finish(worker, Duration::from_millis(25));
        let elapsed = start.elapsed();
        release.send(()).unwrap();
        completed.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            elapsed < Duration::from_millis(500),
            "shutdown took {elapsed:?}"
        );
        println!("Blocked-worker shutdown: {elapsed:?}; worker released and completed");
    }
}
