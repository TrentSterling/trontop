use super::*;
use windows::Win32::System::Services::*;
use windows::core::PCWSTR;

struct Handle(SC_HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        let _ = unsafe { CloseServiceHandle(self.0) };
    }
}

pub(super) struct Native;
impl Backend for Native {
    fn open(&mut self, request: &Request) -> Result<Box<dyn Service>, String> {
        let manager = Handle(
            unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) }
                .map_err(|e| describe("Open local service manager", e))?,
        );
        let mut access = SERVICE_QUERY_STATUS;
        if request.action != Action::Start {
            access |= SERVICE_STOP;
        }
        if request.action != Action::Stop {
            access |= SERVICE_START;
        }
        let name: Vec<u16> = request.name.encode_utf16().chain(Some(0)).collect();
        let handle = Handle(
            unsafe { OpenServiceW(manager.0, PCWSTR(name.as_ptr()), access) }
                .map_err(|e| describe("Open selected service", e))?,
        );
        Ok(Box::new(NativeService(handle)))
    }
}
struct NativeService(Handle);
impl Service for NativeService {
    fn query(&mut self) -> Result<Status, String> {
        let mut status = SERVICE_STATUS_PROCESS::default();
        let mut needed = 0;
        let buffer = unsafe {
            std::slice::from_raw_parts_mut(
                (&mut status as *mut SERVICE_STATUS_PROCESS).cast::<u8>(),
                std::mem::size_of_val(&status),
            )
        };
        unsafe {
            QueryServiceStatusEx(self.0.0, SC_STATUS_PROCESS_INFO, Some(buffer), &mut needed)
        }
        .map_err(|e| describe("Read service status", e))?;
        Ok(status_from_native(status))
    }
    fn start(&mut self) -> Result<(), String> {
        unsafe { StartServiceW(self.0.0, None) }.map_err(|e| describe("Start service", e))
    }
    fn stop(&mut self) -> Result<(), String> {
        let mut status = SERVICE_STATUS::default();
        unsafe { ControlService(self.0.0, SERVICE_CONTROL_STOP, &mut status) }
            .map_err(|e| describe("Stop service", e))
    }
}

pub(crate) fn status_from_native(s: SERVICE_STATUS_PROCESS) -> Status {
    Status {
        state: match s.dwCurrentState {
            SERVICE_STOPPED => State::Stopped,
            SERVICE_START_PENDING => State::Starting,
            SERVICE_STOP_PENDING => State::Stopping,
            SERVICE_RUNNING => State::Running,
            SERVICE_CONTINUE_PENDING => State::Resuming,
            SERVICE_PAUSE_PENDING => State::Pausing,
            SERVICE_PAUSED => State::Paused,
            _ => State::Unknown,
        },
        // Windows documents the PID as invalid for Stopped and possibly invalid
        // during start/stop pending. It must not become an action identity there.
        pid: if matches!(
            s.dwCurrentState,
            SERVICE_RUNNING | SERVICE_PAUSED | SERVICE_PAUSE_PENDING | SERVICE_CONTINUE_PENDING
        ) {
            s.dwProcessId
        } else {
            0
        },
        accepts_stop: s.dwControlsAccepted & SERVICE_ACCEPT_STOP != 0,
        win32_service: s.dwServiceType.contains(SERVICE_WIN32_OWN_PROCESS)
            || s.dwServiceType.contains(SERVICE_WIN32_SHARE_PROCESS),
        system_process: s.dwServiceFlags.0 & SERVICE_RUNS_IN_SYSTEM_PROCESS.0 != 0,
        exit_code: s.dwWin32ExitCode,
        service_exit_code: s.dwServiceSpecificExitCode,
    }
}

fn describe(operation: &str, error: windows::core::Error) -> String {
    let code = error.code().0 as u32;
    let reason = match code {
        0x80070005 => {
            "Access denied by Windows. Trontop does not auto-elevate or change service permissions."
        }
        0x8007041B => {
            "Running dependent services prevent this Stop. Trontop will not stop them recursively."
        }
        0x80070422 => "The service is disabled. Its startup configuration was not changed.",
        0x80070424 => "The selected service no longer exists.",
        0x80070425 => "The service cannot accept this command in its current state.",
        0x8007041D => {
            "Windows timed out handling the command. Its final outcome may still be unknown."
        }
        _ => "Windows rejected or could not complete the request.",
    };
    format!("{operation}: {reason} (0x{code:08X}: {error})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_status_mapping_preserves_capabilities_and_rejects_invalid_pids() {
        let mut raw = SERVICE_STATUS_PROCESS {
            dwCurrentState: SERVICE_RUNNING,
            dwProcessId: 1200,
            dwServiceType: SERVICE_WIN32_OWN_PROCESS,
            dwControlsAccepted: SERVICE_ACCEPT_STOP,
            ..Default::default()
        };
        let live = status_from_native(raw);
        assert_eq!(live.state, State::Running);
        assert!(Action::Stop.refusal(live).is_none());
        raw.dwServiceFlags = SERVICE_RUNS_IN_SYSTEM_PROCESS;
        assert!(Action::Stop.refusal(status_from_native(raw)).is_some());
        raw.dwServiceFlags = Default::default();
        raw.dwServiceType = SERVICE_KERNEL_DRIVER;
        assert!(Action::Stop.refusal(status_from_native(raw)).is_some());
        for state in [SERVICE_STOPPED, SERVICE_START_PENDING, SERVICE_STOP_PENDING] {
            raw.dwCurrentState = state;
            assert_eq!(status_from_native(raw).pid, 0);
        }
    }

    #[test]
    fn native_errors_explain_permissions_dependencies_disabled_and_timeout() {
        for (code, expected) in [
            (0x80070005u32, "Access denied"),
            (0x8007041B, "dependent services"),
            (0x80070422, "disabled"),
            (0x80070424, "no longer exists"),
            (0x8007041D, "unknown"),
        ] {
            let message = describe(
                "Fixture operation",
                windows::core::Error::from_hresult(windows::core::HRESULT(code as i32)),
            );
            assert!(message.contains(expected), "{message}");
            assert!(message.contains(&format!("0x{code:08X}")));
        }
    }

    #[test]
    fn native_query_uses_only_read_permissions_and_sends_no_controls() {
        let manager = Handle(
            unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) }.unwrap(),
        );
        let rows = crate::windows_metrics::enumerate_services().unwrap();
        let started = Instant::now();
        for row in rows.iter().take(32) {
            let name: Vec<u16> = row.name.encode_utf16().chain(Some(0)).collect();
            // Deliberately does not use Native::open, which requests control rights.
            if let Ok(handle) =
                unsafe { OpenServiceW(manager.0, PCWSTR(name.as_ptr()), SERVICE_QUERY_STATUS) }
            {
                let mut service = NativeService(Handle(handle));
                if let Ok(status) = service.query() {
                    assert!(status.win32_service);
                    eprintln!(
                        "Read-only SCM status query: {:?} in {:?}; no Start/Stop or desktop input",
                        status.state,
                        started.elapsed()
                    );
                    return;
                }
            }
        }
        panic!("No readable service status among the bounded probe candidates");
    }
}
