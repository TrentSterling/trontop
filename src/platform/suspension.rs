//! Worker-owned suspension references. No privilege changes or renderer calls.
use super::*;

#[cfg(windows)]
const MAX_HOLDS: usize = 256;

#[cfg(windows)]
#[derive(Default)]
pub struct Suspensions {
    held: Vec<Hold>,
}

#[cfg(windows)]
impl Suspensions {
    pub fn held(&self) -> Vec<ProcessIdentity> {
        self.held.iter().map(|hold| hold.identity).collect()
    }

    pub fn prune_exited(&mut self) {
        self.held.retain(|hold| !hold.process.has_exited());
    }

    pub fn suspend(&mut self, identity: ProcessIdentity) -> Result<(), String> {
        can_control(identity.pid)?;
        if self.held.iter().any(|hold| hold.identity == identity) {
            return Err(
                "Trontop already holds a suspension for this process. Resume it first.".into(),
            );
        }
        self.prune_exited();
        if self.held.len() >= MAX_HOLDS {
            return Err(
                "Trontop holds 256 process suspensions. Resume a process before adding another."
                    .into(),
            );
        }
        self.held.push(Hold::create(identity)?);
        Ok(())
    }

    pub fn resume(&mut self, identity: ProcessIdentity) -> Result<(), String> {
        can_control(identity.pid)?;
        if let Some(index) = self.held.iter().position(|hold| hold.identity == identity) {
            // Retain the reference on failure for a later explicit Resume.
            self.held[index].resume()?;
            self.held.swap_remove(index);
            Ok(())
        } else {
            resume_external(identity)
        }
    }
}

#[cfg(windows)]
struct Hold {
    identity: ProcessIdentity,
    state: StateHandle,
    process: ProcessHandle,
    change: Change,
}

#[cfg(windows)]
impl Hold {
    fn create(identity: ProcessIdentity) -> Result<Self, String> {
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::Threading::{PROCESS_SET_INFORMATION, PROCESS_SUSPEND_RESUME};
        let process =
            ProcessHandle::verified(identity, PROCESS_SUSPEND_RESUME | PROCESS_SET_INFORMATION)?;
        let create: Create = unsafe { std::mem::transmute(export(c"NtCreateProcessStateChange")?) };
        let change: Change = unsafe { std::mem::transmute(export(c"NtChangeProcessState")?) };
        let mut state = HANDLE::default();
        // SAFETY: matching ntdll ABI, writable output, same verified process;
        // PROCESS_STATE_CHANGE_STATE (1), no optional attributes, reserved zero.
        let status = unsafe { create(&mut state, 1, std::ptr::null(), process.0, 0) };
        nt_result(status, "create a suspension reference for", identity.pid)?;
        if state.is_invalid() {
            return Err(
                "Windows returned no suspension reference. No suspend request was sent.".into(),
            );
        }
        let hold = Self {
            identity,
            state: StateHandle(state),
            process,
            change,
        };
        hold.apply(0, "suspend")?;
        Ok(hold)
    }

    fn resume(&self) -> Result<(), String> {
        // Recheck protection and exact lifetime on the original held handle.
        self.process.validate_identity(self.identity)?;
        self.apply(1, "resume")
    }

    fn apply(&self, operation: u32, verb: &str) -> Result<(), String> {
        // SAFETY: paired owned handles; 0/1 mean Suspend/Resume. No optional data.
        let status = unsafe {
            (self.change)(
                self.state.0,
                self.process.0,
                operation,
                std::ptr::null(),
                0,
                0,
            )
        };
        nt_result(status, verb, self.identity.pid)
    }
}

#[cfg(windows)]
struct StateHandle(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Drop for StateHandle {
    fn drop(&mut self) {
        // The owned-child gate verifies release of only this state's suspension.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
type Create = unsafe extern "system" fn(
    *mut windows::Win32::Foundation::HANDLE,
    u32,
    *const std::ffi::c_void,
    windows::Win32::Foundation::HANDLE,
    u32,
) -> i32;
#[cfg(windows)]
type Change = unsafe extern "system" fn(
    windows::Win32::Foundation::HANDLE,
    windows::Win32::Foundation::HANDLE,
    u32,
    *const std::ffi::c_void,
    usize,
    u32,
) -> i32;

#[cfg(windows)]
fn export(name: &std::ffi::CStr) -> Result<unsafe extern "system" fn() -> isize, String> {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::core::{PCSTR, w};
    // ntdll is loaded for the process lifetime; this module handle is borrowed.
    let module = unsafe { GetModuleHandleW(w!("ntdll.dll")) }
        .map_err(|error| format!("Windows suspension provider is unavailable: {error}"))?;
    unsafe { GetProcAddress(module, PCSTR(name.as_ptr().cast())) }.ok_or_else(|| {
        format!(
            "Process suspension is unavailable on this Windows version ({}). No action was taken.",
            name.to_string_lossy()
        )
    })
}

#[cfg(windows)]
fn resume_external(identity: ProcessIdentity) -> Result<(), String> {
    use windows::Win32::System::Threading::PROCESS_SUSPEND_RESUME;
    type Resume = unsafe extern "system" fn(windows::Win32::Foundation::HANDLE) -> i32;
    let process = ProcessHandle::verified(identity, PROCESS_SUSPEND_RESUME)?;
    let resume: Resume = unsafe { std::mem::transmute(export(c"NtResumeProcess")?) };
    let status = unsafe { resume(process.0) };
    nt_result(status, "resume", identity.pid)
}

#[cfg(windows)]
fn nt_result(status: i32, verb: &str, pid: u32) -> Result<(), String> {
    if status >= 0 {
        return Ok(());
    }
    let reason = match status as u32 {
        0xc000_0022 => "Access denied by Windows.",
        0xc000_010a => "The process has exited or is terminating.",
        0xc000_00bb => "This operation is unsupported by Windows.",
        _ => "Windows refused the operation.",
    };
    Err(format!(
        "Could not {verb} PID {pid}: {reason} NTSTATUS 0x{:08X}.",
        status as u32
    ))
}

#[cfg(not(windows))]
#[derive(Default)]
pub struct Suspensions;
#[cfg(not(windows))]
impl Suspensions {
    pub fn held(&self) -> Vec<ProcessIdentity> {
        Vec::new()
    }
    pub fn prune_exited(&mut self) {}
    pub fn suspend(&mut self, _identity: ProcessIdentity) -> Result<(), String> {
        Err("Process suspension is only supported on Windows.".into())
    }
    pub fn resume(&mut self, _identity: ProcessIdentity) -> Result<(), String> {
        Err("Process resumption is only supported on Windows.".into())
    }
}

#[cfg(test)]
pub(crate) mod tests;
