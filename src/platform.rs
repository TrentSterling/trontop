use crate::model::{PriorityClass, ProcessControlInfo};
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
pub fn query_process_control(pid: u32) -> Result<ProcessControlInfo, String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, GetPriorityClass,
        GetProcessAffinityMask, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, REALTIME_PRIORITY_CLASS,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|error| format!("Could not inspect PID {pid}: {error}"))?;
        let raw_priority = GetPriorityClass(handle);
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
            GetProcessAffinityMask(handle, &mut affinity_mask, &mut system_affinity_mask);
        let _ = CloseHandle(handle);
        affinity_result
            .map_err(|error| format!("Could not inspect PID {pid} affinity: {error}"))?;
        Ok(ProcessControlInfo {
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
pub fn set_process_priority(pid: u32, priority: PriorityClass) -> Result<(), String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS,
        IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, OpenProcess, PROCESS_SET_INFORMATION,
        SetPriorityClass,
    };

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
        let handle = OpenProcess(PROCESS_SET_INFORMATION, false, pid)
            .map_err(|error| format!("Could not open PID {pid} for priority control: {error}"))?;
        let result = SetPriorityClass(handle, native)
            .map_err(|error| format!("Could not set PID {pid} priority: {error}"));
        let _ = CloseHandle(handle);
        result
    }
}

#[cfg(not(windows))]
pub fn set_process_priority(_pid: u32, _priority: PriorityClass) -> Result<(), String> {
    Err("Process controls are only supported on Windows.".into())
}

#[cfg(windows)]
pub fn set_process_affinity(pid: u32, affinity_mask: usize) -> Result<(), String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
        SetProcessAffinityMask,
    };

    can_control(pid)?;
    if affinity_mask == 0 {
        return Err("At least one logical processor must remain selected.".into());
    }
    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_INFORMATION,
            false,
            pid,
        )
        .map_err(|error| format!("Could not open PID {pid} for affinity control: {error}"))?;
        let result = SetProcessAffinityMask(handle, affinity_mask)
            .map_err(|error| format!("Could not set PID {pid} affinity: {error}"));
        let _ = CloseHandle(handle);
        result
    }
}

#[cfg(not(windows))]
pub fn set_process_affinity(_pid: u32, _affinity_mask: usize) -> Result<(), String> {
    Err("Process controls are only supported on Windows.".into())
}

#[cfg(windows)]
pub fn terminate_process(pid: u32) -> Result<(), String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

    can_terminate(pid)?;
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, false, pid)
            .map_err(|error| format!("Could not open PID {pid}: {error}"))?;
        let result = TerminateProcess(handle, 1)
            .map_err(|error| format!("Could not terminate PID {pid}: {error}"));
        let _ = CloseHandle(handle);
        result
    }
}

#[cfg(not(windows))]
pub fn terminate_process(_pid: u32) -> Result<(), String> {
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

    #[test]
    fn refuses_empty_affinity_without_touching_a_process() {
        assert!(set_process_affinity(u32::MAX, 0).is_err());
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
        assert!(original.accessible);
        assert_ne!(original.affinity_mask, 0);

        set_process_priority(pid, PriorityClass::BelowNormal)
            .expect("failed to set disposable child priority");
        let changed = query_process_control(pid).expect("failed to re-read child priority");
        assert_eq!(changed.priority, PriorityClass::BelowNormal);

        let one_processor = original.affinity_mask & original.affinity_mask.wrapping_neg();
        set_process_affinity(pid, one_processor).expect("failed to set disposable child affinity");
        let changed = query_process_control(pid).expect("failed to re-read child affinity");
        assert_eq!(changed.affinity_mask, one_processor);

        if !matches!(
            original.priority,
            PriorityClass::Realtime | PriorityClass::Unknown
        ) {
            set_process_priority(pid, original.priority)
                .expect("failed to restore disposable child priority");
        }
        set_process_affinity(pid, original.affinity_mask)
            .expect("failed to restore disposable child affinity");
    }
}
