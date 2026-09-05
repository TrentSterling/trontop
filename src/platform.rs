use crate::model::{PriorityClass, ProcessControlInfo, ProcessIdentity};
use std::path::Path;

pub fn can_terminate(pid: u32) -> Result<(), String> {
    if pid <= 4 {
        return Err("Trontop will not terminate Windows kernel processes.".into());
    }
    if pid == std::process::id() {
        return Err("Use the window close button to exit Trontop.".into());
    }
    Ok(())
}

pub fn can_control(pid: u32) -> Result<(), String> {
    if pid <= 4 {
        return Err("Trontop will not modify Windows kernel processes.".into());
    }
    if pid == std::process::id() {
        return Err("Trontop will not modify its own scheduling controls.".into());
    }
    Ok(())
}

#[cfg(windows)]
struct ProcessHandle(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: exactly one owned OpenProcess reference, never a pseudo handle.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
impl ProcessHandle {
    fn open(
        pid: u32,
        access: windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS,
    ) -> Result<Self, String> {
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        // SAFETY: no pointers; failures remain explicit, never trigger elevation.
        unsafe { OpenProcess(access | PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map(Self)
            .map_err(|error| format!("Could not open PID {pid}: {error}"))
    }

    fn created_at(&self) -> Result<u64, String> {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::GetProcessTimes;
        let (mut creation, mut exit, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        // SAFETY: live handle with query access and distinct writable FILETIMEs.
        unsafe { GetProcessTimes(self.0, &mut creation, &mut exit, &mut kernel, &mut user) }
            .map_err(|error| format!("Could not verify process creation time: {error}"))?;
        Ok((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
    }

    fn verified(
        identity: ProcessIdentity,
        access: windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS,
    ) -> Result<Self, String> {
        if identity.created_at_100ns == 0 {
            return Err("Process identity is unavailable. No action was taken.".into());
        }
        let handle = Self::open(identity.pid, access)?;
        if handle.created_at()? != identity.created_at_100ns {
            return Err(
                "The selected PID now belongs to a different process. No action was taken.".into(),
            );
        }
        let mut critical = windows::core::BOOL::default();
        // SAFETY: same query-capable native handle and writable BOOL. Fail closed
        // if protection status cannot be established, including when elevated.
        unsafe { windows::Win32::System::Threading::IsProcessCritical(handle.0, &mut critical) }
            .map_err(|error| {
                format!("Could not verify process protection status. No action was taken: {error}")
            })?;
        reject_critical(critical.as_bool())?;
        // The caller acts on THIS handle. Never reopen by PID after verification.
        Ok(handle)
    }
}

#[cfg(windows)]
fn reject_critical(critical: bool) -> Result<(), String> {
    if critical {
        Err(
            "Windows marks this process as critical. Trontop will not modify or terminate it."
                .into(),
        )
    } else {
        Ok(())
    }
}

#[cfg(windows)]
pub fn query_process_control(pid: u32) -> Result<ProcessControlInfo, String> {
    use windows::Win32::System::Threading::{
        ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, GetPriorityClass,
        GetProcessAffinityMask, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
        PROCESS_QUERY_LIMITED_INFORMATION, REALTIME_PRIORITY_CLASS,
    };

    unsafe {
        let handle = ProcessHandle::open(pid, PROCESS_QUERY_LIMITED_INFORMATION)?;
        let created_at_100ns = Some(handle.created_at()?);
        let raw_priority = GetPriorityClass(handle.0);
        let priority = match raw_priority {
            value if value == IDLE_PRIORITY_CLASS.0 => PriorityClass::Idle,
            value if value == BELOW_NORMAL_PRIORITY_CLASS.0 => PriorityClass::BelowNormal,
            value if value == NORMAL_PRIORITY_CLASS.0 => PriorityClass::Normal,
            value if value == ABOVE_NORMAL_PRIORITY_CLASS.0 => PriorityClass::AboveNormal,
            value if value == HIGH_PRIORITY_CLASS.0 => PriorityClass::High,
            value if value == REALTIME_PRIORITY_CLASS.0 => PriorityClass::Realtime,
            _ => PriorityClass::Unknown,
        };
        let mut affinity_mask = 0_usize;
        let mut system_affinity_mask = 0_usize;
        let affinity_result =
            GetProcessAffinityMask(handle.0, &mut affinity_mask, &mut system_affinity_mask);
        if affinity_result.is_err() {
            affinity_mask = 0;
            system_affinity_mask = 0;
        }
        Ok(ProcessControlInfo {
            created_at_100ns,
            priority,
            affinity_mask,
            system_affinity_mask,
            accessible: raw_priority != 0,
        })
    }
}

#[cfg(not(windows))]
pub fn query_process_control(_pid: u32) -> Result<ProcessControlInfo, String> {
    Err("Process controls are only supported on Windows.".into())
}

#[cfg(windows)]
pub fn set_process_priority(
    identity: ProcessIdentity,
    priority: PriorityClass,
) -> Result<(), String> {
    use windows::Win32::System::Threading::{
        ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS,
        IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_SET_INFORMATION, SetPriorityClass,
    };

    let pid = identity.pid;
    can_control(pid)?;
    let native = match priority {
        PriorityClass::Idle => IDLE_PRIORITY_CLASS,
        PriorityClass::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
        PriorityClass::Normal => NORMAL_PRIORITY_CLASS,
        PriorityClass::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
        PriorityClass::High => HIGH_PRIORITY_CLASS,
        PriorityClass::Realtime | PriorityClass::Unknown => {
            return Err("That priority class cannot be applied.".into());
        }
    };
    unsafe {
        let handle = ProcessHandle::verified(identity, PROCESS_SET_INFORMATION)?;
        SetPriorityClass(handle.0, native)
            .map_err(|error| format!("Could not set PID {pid} priority: {error}"))
    }
}

#[cfg(not(windows))]
pub fn set_process_priority(
    _identity: ProcessIdentity,
    _priority: PriorityClass,
) -> Result<(), String> {
    Err("Process controls are only supported on Windows.".into())
}

#[cfg(windows)]
pub fn set_process_affinity(identity: ProcessIdentity, affinity_mask: usize) -> Result<(), String> {
    use windows::Win32::System::Threading::{PROCESS_SET_INFORMATION, SetProcessAffinityMask};

    let pid = identity.pid;
    can_control(pid)?;
    if affinity_mask == 0 {
        return Err("At least one logical processor must remain selected.".into());
    }
    unsafe {
        let handle = ProcessHandle::verified(identity, PROCESS_SET_INFORMATION)?;
        SetProcessAffinityMask(handle.0, affinity_mask)
            .map_err(|error| format!("Could not set PID {pid} affinity: {error}"))
    }
}

#[cfg(not(windows))]
pub fn set_process_affinity(
    _identity: ProcessIdentity,
    _affinity_mask: usize,
) -> Result<(), String> {
    Err("Process controls are only supported on Windows.".into())
}

#[cfg(windows)]
pub fn terminate_process(identity: ProcessIdentity) -> Result<(), String> {
    use windows::Win32::System::Threading::{PROCESS_TERMINATE, TerminateProcess};

    let pid = identity.pid;
    can_terminate(pid)?;
    unsafe {
        let handle = ProcessHandle::verified(identity, PROCESS_TERMINATE)?;
        TerminateProcess(handle.0, 1)
            .map_err(|error| format!("Could not terminate PID {pid}: {error}"))
    }
}

#[cfg(not(windows))]
pub fn terminate_process(_identity: ProcessIdentity) -> Result<(), String> {
    Err("Ending processes is only supported on Windows.".into())
}

#[cfg(windows)]
pub fn reveal_in_explorer(path: &Path) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open Explorer: {error}"))
}

#[cfg(not(windows))]
pub fn reveal_in_explorer(_path: &Path) -> Result<(), String> {
    Err("Explorer is only available on Windows.".into())
}

#[cfg(windows)]
pub fn launch_command(command: &str) -> Result<(), String> {
    std::process::Command::new("cmd.exe")
        .args(["/D", "/S", "/C", "start", "", command])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not launch command: {error}"))
}

#[cfg(not(windows))]
pub fn launch_command(command: &str) -> Result<(), String> {
    std::process::Command::new(command)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not launch command: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    struct TestChild(std::process::Child);

    #[cfg(windows)]
    impl Drop for TestChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn refuses_kernel_pids() {
        assert!(can_terminate(0).is_err());
        assert!(can_terminate(4).is_err());
    }

    #[test]
    fn refuses_own_pid() {
        assert!(can_terminate(std::process::id()).is_err());
        assert!(can_control(std::process::id()).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn critical_process_policy_fails_closed_without_touching_system_processes() {
        assert!(reject_critical(true).is_err());
        assert!(reject_critical(false).is_ok());
    }

    #[test]
    fn refuses_empty_affinity_without_touching_a_process() {
        assert!(
            set_process_affinity(
                ProcessIdentity {
                    pid: u32::MAX,
                    created_at_100ns: 1
                },
                0
            )
            .is_err()
        );
    }

    #[cfg(windows)]
    #[test]
    fn reads_and_changes_controls_on_a_disposable_child() {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let child = std::process::Command::new("ping.exe")
            .args(["-t", "127.0.0.1"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .expect("failed to launch disposable test process");
        let child = TestChild(child);
        let pid = child.0.id();
        let original = query_process_control(pid).expect("failed to inspect disposable child");
        let identity = ProcessIdentity {
            pid,
            created_at_100ns: original.created_at_100ns.expect("missing creation time"),
        };
        assert!(original.accessible);
        assert_ne!(original.affinity_mask, 0);
        let mut system = sysinfo::System::new();
        let native_pid = sysinfo::Pid::from_u32(pid);
        system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[native_pid]), true);
        let displayed_start = system.process(native_pid).unwrap().start_time();
        assert_eq!(
            original
                .for_observed_start(displayed_start)
                .created_at_100ns,
            original.created_at_100ns
        );

        set_process_priority(identity, PriorityClass::BelowNormal)
            .expect("failed to set disposable child priority");
        let changed = query_process_control(pid).expect("failed to re-read child priority");
        assert_eq!(changed.priority, PriorityClass::BelowNormal);

        let one_processor = original.affinity_mask & original.affinity_mask.wrapping_neg();
        set_process_affinity(identity, one_processor)
            .expect("failed to set disposable child affinity");
        let changed = query_process_control(pid).expect("failed to re-read child affinity");
        assert_eq!(changed.affinity_mask, one_processor);

        if !matches!(
            original.priority,
            PriorityClass::Realtime | PriorityClass::Unknown
        ) {
            set_process_priority(identity, original.priority)
                .expect("failed to restore disposable child priority");
        }
        set_process_affinity(identity, original.affinity_mask)
            .expect("failed to restore disposable child affinity");
    }

    #[cfg(windows)]
    #[test]
    fn mismatched_creation_time_cannot_kill_or_modify_a_disposable_child() {
        use std::os::windows::process::CommandExt;
        let child = std::process::Command::new("ping.exe")
            .args(["-t", "127.0.0.1"])
            .creation_flags(0x0800_0000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to launch disposable child");
        let mut child = TestChild(child);
        let pid = child.0.id();
        let before = query_process_control(pid).unwrap();
        let identity = ProcessIdentity {
            pid,
            created_at_100ns: before.created_at_100ns.unwrap(),
        };
        // Same PID, creation time differs by only 100 ns. Rounded seconds would
        // incorrectly accept this. No actual PID recycling or external process used.
        let wrong = ProcessIdentity {
            created_at_100ns: identity.created_at_100ns + 1,
            ..identity
        };
        for error in [
            terminate_process(wrong).unwrap_err(),
            set_process_priority(wrong, PriorityClass::BelowNormal).unwrap_err(),
            set_process_affinity(
                wrong,
                before.affinity_mask & before.affinity_mask.wrapping_neg(),
            )
            .unwrap_err(),
        ] {
            assert!(error.contains("different process"), "{error}");
        }
        assert!(child.0.try_wait().unwrap().is_none());
        let after = query_process_control(pid).unwrap();
        assert_eq!(after.priority, before.priority);
        assert_eq!(after.affinity_mask, before.affinity_mask);
        assert!(
            terminate_process(ProcessIdentity {
                created_at_100ns: 0,
                ..identity
            })
            .is_err()
        );
        assert!(child.0.try_wait().unwrap().is_none());
        terminate_process(identity).expect("verified termination of disposable child");
        child.0.wait().expect("disposable child exits");
    }
}
