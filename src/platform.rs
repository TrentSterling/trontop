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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_kernel_pids() {
        assert!(can_terminate(0).is_err());
        assert!(can_terminate(4).is_err());
    }

    #[test]
    fn refuses_own_pid() {
        assert!(can_terminate(std::process::id()).is_err());
    }
}
