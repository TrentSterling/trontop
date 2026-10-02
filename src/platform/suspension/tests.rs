use super::*;

#[cfg(windows)]
pub(crate) struct HeartbeatChild {
    child: std::process::Child,
    pub identity: ProcessIdentity,
    beats: std::sync::mpsc::Receiver<()>,
}

#[cfg(windows)]
impl HeartbeatChild {
    pub fn spawn() -> Self {
        use std::io::BufRead;
        use std::os::windows::process::CommandExt;
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "platform::suspension::tests::owned_hidden_heartbeat_child",
                "--ignored",
                "--nocapture",
            ])
            .env("TRONTOP_OWNED_SUSPEND_FIXTURE", "1")
            .creation_flags(0x0800_0000)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("owned hidden heartbeat fixture");
        let stdout = child.stdout.take().unwrap();
        let (sender, beats) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                match line {
                    Ok(line) if line == "TRONTOP_OWNED_HEARTBEAT" => {
                        if sender.send(()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        });
        let control = query_process_control(child.id()).unwrap();
        let owned = Self {
            identity: ProcessIdentity {
                pid: child.id(),
                created_at_100ns: control.created_at_100ns.unwrap(),
            },
            child,
            beats,
        };
        owned.assert_running();
        owned
    }

    pub fn assert_running(&self) {
        while self.beats.try_recv().is_ok() {}
        self.beats
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("owned child must continue producing real heartbeats");
    }

    pub fn assert_paused(&self) {
        // Drain writes that preceded the call, including pipe-reader jitter.
        std::thread::sleep(std::time::Duration::from_millis(100));
        while self.beats.try_recv().is_ok() {}
        assert!(
            matches!(
                self.beats
                    .recv_timeout(std::time::Duration::from_millis(250)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ),
            "suspended child produced a heartbeat or exited"
        );
    }

    fn terminate(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
}

#[cfg(windows)]
impl Drop for HeartbeatChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
#[test]
#[ignore = "Owned hidden child fixture only; requires parent-provided environment guard; no UI or OS input"]
fn owned_hidden_heartbeat_child() {
    use std::io::Write;
    assert_eq!(
        std::env::var("TRONTOP_OWNED_SUSPEND_FIXTURE").as_deref(),
        Ok("1")
    );
    for _ in 0..3000 {
        println!("TRONTOP_OWNED_HEARTBEAT");
        std::io::stdout().flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(windows)]
#[test]
fn owned_process_suspends_resumes_and_recovers_when_its_hold_is_dropped() {
    let child = HeartbeatChild::spawn();
    let mut holds = Suspensions::default();
    holds.suspend(child.identity).unwrap();
    child.assert_paused();
    assert_eq!(holds.held(), vec![child.identity]);
    assert!(
        holds
            .suspend(child.identity)
            .unwrap_err()
            .contains("already holds")
    );
    holds.resume(child.identity).unwrap();
    child.assert_running();
    assert!(holds.held().is_empty());
    holds.suspend(child.identity).unwrap();
    child.assert_paused();
    drop(holds);
    child.assert_running();
    println!(
        "OWNED_SUSPENSION: exact identity, heartbeat stop/resume, duplicate guard, state-handle release PASS"
    );
}

#[cfg(windows)]
#[test]
fn resume_preserves_independent_external_suspensions_and_cleans_exited_holds() {
    use windows::Win32::System::Threading::PROCESS_SUSPEND_RESUME;
    type Suspend = unsafe extern "system" fn(windows::Win32::Foundation::HANDLE) -> i32;
    let mut child = HeartbeatChild::spawn();
    let process = ProcessHandle::verified(child.identity, PROCESS_SUSPEND_RESUME).unwrap();
    let suspend: Suspend = unsafe { std::mem::transmute(export(c"NtSuspendProcess").unwrap()) };
    let mut holds = Suspensions::default();
    holds.suspend(child.identity).unwrap();
    assert!(unsafe { suspend(process.0) } >= 0);
    holds.resume(child.identity).unwrap();
    assert!(holds.held().is_empty());
    child.assert_paused();
    holds.resume(child.identity).unwrap();
    child.assert_running();
    holds.suspend(child.identity).unwrap();
    child.terminate();
    holds.prune_exited();
    assert!(holds.held().is_empty());
    assert!(holds.resume(child.identity).is_err());
    println!(
        "OWNED_SUSPENSION: independent hold retained, external Resume, exited process cleanup PASS"
    );
}

#[cfg(windows)]
#[test]
fn stale_missing_kernel_and_self_identities_cannot_change_suspension() {
    let child = HeartbeatChild::spawn();
    let mut holds = Suspensions::default();
    let wrong = ProcessIdentity {
        created_at_100ns: child.identity.created_at_100ns + 1,
        ..child.identity
    };
    assert!(
        holds
            .suspend(wrong)
            .unwrap_err()
            .contains("different process")
    );
    assert!(
        holds
            .resume(wrong)
            .unwrap_err()
            .contains("different process")
    );
    child.assert_running();
    holds.suspend(child.identity).unwrap();
    assert!(
        holds
            .resume(wrong)
            .unwrap_err()
            .contains("different process")
    );
    child.assert_paused();
    for identity in [
        ProcessIdentity {
            created_at_100ns: 0,
            ..child.identity
        },
        ProcessIdentity {
            pid: 0,
            created_at_100ns: 1,
        },
        ProcessIdentity {
            pid: 4,
            created_at_100ns: 1,
        },
        ProcessIdentity {
            pid: std::process::id(),
            created_at_100ns: 1,
        },
    ] {
        assert!(holds.suspend(identity).is_err());
        assert!(holds.resume(identity).is_err());
    }
    child.assert_paused();
    holds.resume(child.identity).unwrap();
    child.assert_running();
}

#[cfg(windows)]
#[test]
fn missing_exports_and_native_errors_are_explicit() {
    assert!(
        export(c"TrontopMissingSuspensionExportForTest")
            .unwrap_err()
            .contains("unavailable")
    );
    for (status, reason) in [
        (0xc000_0022u32, "Access denied"),
        (0xc000_010a, "terminating"),
        (0xc000_00bb, "unsupported"),
        (0xc000_0001, "refused"),
    ] {
        let error = nt_result(status as i32, "suspend", 4242).unwrap_err();
        assert!(error.contains(reason) && error.contains("PID 4242") && error.contains("NTSTATUS"));
    }
    assert!(nt_result(0, "suspend", 4242).is_ok());
}
