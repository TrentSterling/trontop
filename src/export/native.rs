//! Native Save As, invoked only by an explicitly submitted export job.
use super::Format;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::*;
use windows::core::{HRESULT, w};

struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

pub(super) fn choose_path(format: Format) -> Result<Option<PathBuf>, String> {
    // All COM interfaces and task-allocated strings remain on this one STA worker.
    let result = unsafe { choose(format) };
    result.map_err(|e| format!("Windows Save As failed (0x{:08X}).", e.code().0 as u32))
}

unsafe fn choose(format: Format) -> windows::core::Result<Option<PathBuf>> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;
        let _com = Com;
        let dialog: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
        dialog.SetOptions(
            dialog.GetOptions()?
                | FOS_FORCEFILESYSTEM
                | FOS_PATHMUSTEXIST
                | FOS_OVERWRITEPROMPT
                | FOS_NOCHANGEDIR
                | FOS_DONTADDTORECENT,
        )?;
        let (label, pattern, extension, filename) = match format {
            Format::Json => (
                w!("Trontop snapshot (JSON)"),
                w!("*.json"),
                w!("json"),
                w!("trontop-snapshot.json"),
            ),
            Format::Csv => (
                w!("Trontop processes (CSV)"),
                w!("*.csv"),
                w!("csv"),
                w!("trontop-processes.csv"),
            ),
        };
        dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
            pszName: label,
            pszSpec: pattern,
        }])?;
        dialog.SetDefaultExtension(extension)?;
        dialog.SetFileName(filename)?;
        dialog.SetTitle(w!("Save Trontop export"))?;
        match dialog.Show(None) {
            Ok(()) => {}
            Err(e) if e.code() == HRESULT::from_win32(1223) => return Ok(None),
            Err(e) => return Err(e),
        }
        let item = dialog.GetResult()?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = PathBuf::from(std::ffi::OsString::from_wide(name.as_wide()));
        CoTaskMemFree(Some(name.0.cast()));
        Ok(Some(path))
    }
}
