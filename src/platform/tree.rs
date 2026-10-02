//! Batch termination: verify every handle before sending the first request.
use super::*;
use crate::process_actions::tree::Plan;

#[cfg(windows)]
pub fn terminate_tree(plan: &Plan) -> Result<(), String> {
    use windows::Win32::Foundation::ERROR_INVALID_PARAMETER;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, TerminateProcess,
    };
    let mut handles = Vec::new();
    // The plan has private fields and is constructed through identity guards.
    for target in plan.targets() {
        can_terminate(target.identity.pid)?;
        let handle = match unsafe {
            OpenProcess(
                PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                target.identity.pid,
            )
        } {
            Ok(handle) => ProcessHandle(handle),
            Err(error)
                if error.code()
                    == windows::core::HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) =>
            {
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "No termination requests were sent. Could not open {} (PID {}): {error}",
                    target.name, target.identity.pid
                ));
            }
        };
        handle.validate_identity(target.identity).map_err(|error| {
            format!(
                "No termination requests were sent. {} (PID {}): {error}",
                target.name, target.identity.pid
            )
        })?;
        if handle.has_exited() {
            continue;
        }
        if plan.scope() == crate::process_actions::tree::Scope::AllInstances
            && target.parent.is_none()
        {
            let path = executable_path(&handle)?;
            if plan.executable() != Some(crate::process_actions::tree::path_key(&path).as_str()) {
                return Err(format!(
                    "No termination requests were sent. PID {} does not match the reviewed executable path.",
                    target.identity.pid
                ));
            }
        }
        if let Some(parent) = target.parent {
            let actual = parent_pid(&handle)?;
            if actual != parent.pid || parent.created_at_100ns > target.identity.created_at_100ns {
                return Err(format!(
                    "No termination requests were sent. PID {} no longer matches the reviewed parent relationship.",
                    target.identity.pid
                ));
            }
        }
        handles.push((target, handle));
    }
    let mut accepted = 0;
    let mut failures = Vec::new();
    // Parents precede their descendants. Never discover or chase new PIDs here.
    for (target, handle) in handles {
        if handle.has_exited() {
            continue;
        }
        let result = handle.validate_identity(target.identity).and_then(|()| {
            unsafe { TerminateProcess(handle.0, 1) }.map_err(|error| error.to_string())
        });
        match result {
            Ok(()) => accepted += 1,
            Err(_) if handle.has_exited() => {}
            Err(error) => failures.push(format!(
                "{} (PID {}): {error}",
                target.name, target.identity.pid
            )),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Partial result: {accepted} termination requests accepted; {} failed. {}",
            failures.len(),
            failures.join("; ")
        ))
    }
}

#[cfg(windows)]
fn executable_path(handle: &ProcessHandle) -> Result<std::path::PathBuf, String> {
    use windows::Win32::System::Threading::{PROCESS_NAME_WIN32, QueryFullProcessImageNameW};
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .map_err(|error| {
        format!("No termination requests were sent. Could not verify an executable path: {error}")
    })?;
    Ok(std::path::PathBuf::from(String::from_utf16_lossy(
        &buffer[..length as usize],
    )))
}

#[cfg(windows)]
fn parent_pid(handle: &ProcessHandle) -> Result<u32, String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::System::Threading::PROCESS_BASIC_INFORMATION;
    use windows::core::{PCSTR, w};
    type Query =
        unsafe extern "system" fn(HANDLE, u32, *mut std::ffi::c_void, u32, *mut u32) -> i32;
    let module = unsafe { GetModuleHandleW(w!("ntdll.dll")) }.map_err(|error| error.to_string())?;
    let export =
        unsafe { GetProcAddress(module, PCSTR(c"NtQueryInformationProcess".as_ptr().cast())) }
            .ok_or("No termination requests were sent. Parent verification is unavailable.")?;
    let query: Query = unsafe { std::mem::transmute(export) };
    let mut info = PROCESS_BASIC_INFORMATION::default();
    let mut returned = 0;
    let status = unsafe {
        query(
            handle.0,
            0,
            (&mut info as *mut PROCESS_BASIC_INFORMATION).cast(),
            std::mem::size_of_val(&info) as u32,
            &mut returned,
        )
    };
    if status < 0 || returned as usize != std::mem::size_of_val(&info) {
        return Err(format!(
            "No termination requests were sent. Could not verify a parent relationship (NTSTATUS 0x{:08X}).",
            status as u32
        ));
    }
    u32::try_from(info.InheritedFromUniqueProcessId).map_err(|_| {
        "No termination requests were sent. Windows returned an invalid parent PID.".into()
    })
}

#[cfg(not(windows))]
pub fn terminate_tree(_plan: &Plan) -> Result<(), String> {
    Err("Ending process trees is only supported on Windows.".into())
}

#[cfg(test)]
mod tests;
