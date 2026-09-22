//! Windows reads for the Operating System section: WMI, a few registry values,
//! GetFirmwareType, the TPM base services device info and the user locale.
use super::*;
use crate::specs::native::{NativeError, registry, wmi};
use crate::specs::{Group, LiveKey, Row, SummaryLine, Value};
use std::time::Duration;
use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::System::SystemInformation::{FIRMWARE_TYPE, GetFirmwareType};
use windows::Win32::System::TpmBaseServices::{TPM_DEVICE_INFO, Tbsi_GetDeviceInfo};

const VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const WMI_TIMEOUT: Duration = Duration::from_secs(8);

fn reg_text(path: &str, name: &str) -> Option<String> {
    match registry::read(registry::Hive::LocalMachine, path, name) {
        Ok(Some(value)) => value
            .text()
            .map(str::to_string)
            .or_else(|| value.as_u64().map(|v| v.to_string())),
        _ => None,
    }
}

fn reg_u64(path: &str, name: &str) -> Result<Option<u64>, NativeError> {
    registry::read(registry::Hive::LocalMachine, path, name).map(|v| v.and_then(|v| v.as_u64()))
}

fn date(text: Option<String>) -> Option<String> {
    text.as_deref().and_then(wmi::dmtf_date)
}

/// "20260921083015.500000-300" as "2026-09-21 08:30 (UTC-05:00)".
fn date_time(text: Option<String>) -> Option<String> {
    let text = text?;
    let day = wmi::dmtf_date(&text)?;
    let time = text
        .get(8..12)
        .filter(|t| t.bytes().all(|b| b.is_ascii_digit()))?;
    let offset = text
        .get(21..25)
        .and_then(|o| o.parse::<i32>().ok())
        .map(|minutes| {
            format!(
                " (UTC{}{:02}:{:02})",
                if minutes < 0 { '-' } else { '+' },
                minutes.abs() / 60,
                minutes.abs() % 60
            )
        })
        .unwrap_or_default();
    Some(format!("{day} {}:{}{offset}", &time[..2], &time[2..]))
}

pub(super) fn collect(ctx: &Context) -> Section {
    let mut section = Section::new(SectionId::OperatingSystem);
    let cimv2 = wmi::Wmi::connect(r"ROOT\CIMV2");
    let query = |wql: &str| -> Result<Vec<wmi::WmiRow>, NativeError> {
        match &cimv2 {
            Ok(connection) => connection.query(wql, ctx.timeout(WMI_TIMEOUT)),
            Err(error) => Err(error.clone()),
        }
    };
    let os = query(
        "SELECT Caption, Version, BuildNumber, OSArchitecture, InstallDate, LastBootUpTime, \
         SerialNumber, RegisteredUser, Organization, CSName, SystemDrive, WindowsDirectory \
         FROM Win32_OperatingSystem",
    );
    if let Err(error) = &os {
        section.push_issue(format!("Win32_OperatingSystem: {error}"));
    }
    let os = os
        .ok()
        .and_then(|rows| rows.into_iter().next())
        .unwrap_or_default();
    let build = reg_text(VERSION_KEY, "CurrentBuild")
        .or_else(|| os.text("BuildNumber"))
        .and_then(|b| b.parse::<u32>().ok());
    let ubr = reg_u64(VERSION_KEY, "UBR").ok().flatten();
    let caption = os
        .text("Caption")
        .or_else(|| reg_text(VERSION_KEY, "ProductName"));
    let name = caption.as_deref().map(|c| windows_name(c, build));
    let display_version = reg_text(VERSION_KEY, "DisplayVersion");
    let architecture = os.text("OSArchitecture");
    let build_text = build.map(|b| match ubr {
        Some(ubr) => format!("{b}.{ubr}"),
        None => b.to_string(),
    });

    let mut headline = name.clone().unwrap_or_else(|| "Windows".into());
    if let Some(arch) = &architecture {
        headline.push(' ');
        headline.push_str(arch);
    }
    match (&display_version, &build_text) {
        (Some(version), Some(build)) => headline.push_str(&format!(" ({version}, build {build})")),
        (None, Some(build)) => headline.push_str(&format!(" (build {build})")),
        _ => {}
    }
    section.push_summary(SummaryLine::known(headline));

    let mut windows = Group::new("Windows")
        .kv(
            "Edition",
            Value::from_option(name, "Windows did not report its edition"),
        )
        .row(
            Row::new(
                "Version",
                Value::from_option(display_version, "no DisplayVersion in the registry"),
            )
            .note("Registry CurrentVersion\\DisplayVersion"),
        )
        .row(
            Row::new(
                "Build",
                Value::from_option(build_text, "Windows did not report its build"),
            )
            .note("CurrentBuild.UBR (update build revision)"),
        )
        .kv(
            "Kernel version",
            Value::from_option(os.text("Version"), NOT_REPORTED_OS),
        )
        .kv(
            "Architecture",
            Value::from_option(architecture, NOT_REPORTED_OS),
        )
        .kv(
            "Installation type",
            Value::from_option(reg_text(VERSION_KEY, "InstallationType"), NOT_REPORTED_OS),
        )
        .kv(
            "Installed",
            Value::from_option(date(os.text("InstallDate")), NOT_REPORTED_OS),
        )
        .kv(
            "Last boot",
            Value::from_option(date_time(os.text("LastBootUpTime")), NOT_REPORTED_OS),
        )
        .row(Row::live("Uptime", LiveKey::Uptime))
        .row(
            Row::new(
                "Computer name",
                Value::from_option(os.text("CSName"), NOT_REPORTED_OS),
            )
            .private(),
        )
        .row(
            Row::new(
                "Registered user",
                Value::from_option(os.text("RegisteredUser"), NOT_REPORTED_OS),
            )
            .private(),
        )
        .row(
            Row::new(
                "Product ID",
                Value::from_option(os.text("SerialNumber"), NOT_REPORTED_OS),
            )
            .private(),
        )
        .kv(
            "Windows folder",
            Value::from_option(os.text("WindowsDirectory"), NOT_REPORTED_OS),
        );
    let mut locale = [0u16; 85];
    // SAFETY: writable buffer of LOCALE_NAME_MAX_LENGTH characters.
    let written = unsafe { GetUserDefaultLocaleName(&mut locale) };
    windows.push_row(Row::new(
        "Locale",
        if written > 0 {
            Value::known(crate::specs::native::utf16_until_nul(&locale))
        } else {
            Value::unavailable(NativeError::last_error("GetUserDefaultLocaleName").to_string())
        },
    ));
    windows.push_row(Row::new(
        "Time zone",
        Value::from_option(time_zone(), NOT_REPORTED_OS),
    ));
    windows.push_row(Row::new("Power plan", Value::from_result(power_plan())));
    let dotnet = reg_u64(
        r"SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full",
        "Release",
    );
    windows.push_row(
        Row::new(
            ".NET Framework",
            match dotnet {
                Ok(Some(release)) => Value::from_option(
                    dotnet_version(release),
                    format!("release {release} is not in the documented table"),
                ),
                Ok(None) => Value::unavailable(".NET Framework 4.5 or later is not installed"),
                Err(error) => Value::unavailable(error.to_string()),
            },
        )
        .note("NDP\\v4\\Full Release value"),
    );
    section.push_group(windows);

    if ctx.should_stop() {
        section.push_issue("Read budget exhausted; security and page file details not read.");
        return section;
    }

    // Firmware and security.
    let mut security = Group::new("Firmware and security");
    let mut firmware = FIRMWARE_TYPE::default();
    // SAFETY: writable output value.
    let firmware_text = match unsafe { GetFirmwareType(&mut firmware) } {
        Ok(()) => match firmware.0 {
            1 => Value::known("Legacy BIOS"),
            2 => Value::known("UEFI"),
            _ => Value::unavailable("Windows reported an unknown firmware type"),
        },
        Err(error) => {
            Value::unavailable(NativeError::from_windows("GetFirmwareType", &error).to_string())
        }
    };
    security.push_row(Row::new("Firmware type", firmware_text));
    security.push_row(Row::new(
        "Secure Boot",
        match reg_u64(
            r"SYSTEM\CurrentControlSet\Control\SecureBoot\State",
            "UEFISecureBootEnabled",
        ) {
            Ok(Some(1)) => Value::known("On"),
            Ok(Some(_)) => Value::known("Off"),
            Ok(None) => Value::unavailable("not supported by this firmware"),
            Err(error) => Value::unavailable(error.to_string()),
        },
    ));
    let mut info = TPM_DEVICE_INFO::default();
    // SAFETY: the size passed is the structure size.
    let status = unsafe {
        Tbsi_GetDeviceInfo(
            std::mem::size_of::<TPM_DEVICE_INFO>() as u32,
            (&mut info as *mut TPM_DEVICE_INFO).cast(),
        )
    };
    security.push_row(
        Row::new(
            "TPM",
            match status {
                0 => Value::known(match info.tpmVersion {
                    1 => "Present, version 1.2".to_string(),
                    2 => "Present, version 2.0".to_string(),
                    other => format!("Present, version code {other}"),
                }),
                0x8028_400F => Value::known("Not present"),
                code => Value::unavailable(
                    NativeError::Windows {
                        api: "Tbsi_GetDeviceInfo",
                        code,
                    }
                    .to_string(),
                ),
            },
        )
        .note("TPM Base Services device information"),
    );
    let system = query("SELECT HypervisorPresent FROM Win32_ComputerSystem");
    security.push_row(Row::new(
        "Hypervisor running",
        Value::from_result(system.and_then(|rows| {
            rows.first()
                .and_then(|r| r.bool("HypervisorPresent"))
                .map(|b| if b { "Yes" } else { "No" }.to_string())
                .ok_or(NativeError::Unsupported("not reported"))
        })),
    ));
    match wmi::query_once(
        r"ROOT\Microsoft\Windows\DeviceGuard",
        "SELECT VirtualizationBasedSecurityStatus, SecurityServicesRunning, \
         SecurityServicesConfigured FROM Win32_DeviceGuard",
        ctx.timeout(WMI_TIMEOUT),
    ) {
        Ok(rows) => {
            let row = rows.into_iter().next().unwrap_or_default();
            let services = |name: &str| {
                row.get(name)
                    .map(|v| match v {
                        wmi::WmiValue::Array(items) => {
                            items.iter().filter_map(wmi::WmiValue::as_u64).collect()
                        }
                        other => other.as_u64().into_iter().collect::<Vec<_>>(),
                    })
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|id| *id != 0)
                    .collect::<Vec<_>>()
            };
            let running = services("SecurityServicesRunning");
            security.push_row(Row::new(
                "Virtualization-based security",
                Value::from_option(
                    row.u64("VirtualizationBasedSecurityStatus")
                        .map(|s| vbs_status(s).to_string()),
                    "not reported",
                ),
            ));
            security.push_row(Row::known(
                "Memory integrity",
                if running.contains(&2) {
                    "Running"
                } else if services("SecurityServicesConfigured").contains(&2) {
                    "Configured, not running"
                } else {
                    "Off"
                },
            ));
            security.push_row(Row::known(
                "Security services running",
                if running.is_empty() {
                    "None".to_string()
                } else {
                    running
                        .iter()
                        .map(|id| security_service(*id))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
            ));
        }
        Err(error) => {
            security.push_row(Row::unavailable(
                "Virtualization-based security",
                error.to_string(),
            ));
        }
    }
    let antivirus = wmi::query_once(
        r"ROOT\SecurityCenter2",
        "SELECT displayName FROM AntiVirusProduct",
        ctx.timeout(WMI_TIMEOUT),
    );
    security.push_row(
        Row::new(
            "Antivirus",
            Value::from_result(antivirus.and_then(|rows| {
                let mut names = rows
                    .iter()
                    .filter_map(|r| r.text("displayName"))
                    .collect::<Vec<_>>();
                names.dedup();
                if names.is_empty() {
                    Err(NativeError::Unsupported("no antivirus product registered"))
                } else {
                    Ok(names.join(", "))
                }
            })),
        )
        .note("Registered with Windows Security Center"),
    );
    section.push_group(security);

    // Page files.
    let mut memory =
        Group::new("Virtual memory").row(Row::live("Commit charge", LiveKey::MemoryCommit));
    match query("SELECT Name, AllocatedBaseSize, CurrentUsage, PeakUsage FROM Win32_PageFileUsage")
    {
        Ok(rows) if rows.is_empty() => {
            memory.push_row(Row::known("Page file", "None (paging file disabled)"));
        }
        Ok(rows) => {
            for row in rows {
                let label = row
                    .text("Name")
                    .map_or_else(|| "Page file".to_string(), |n| format!("Page file {n}"));
                let text = match (row.u64("AllocatedBaseSize"), row.u64("CurrentUsage")) {
                    (Some(size), Some(used)) => Some(format!(
                        "{} allocated, {} in use{}",
                        crate::format::bytes(size * 1024 * 1024),
                        crate::format::bytes(used * 1024 * 1024),
                        row.u64("PeakUsage")
                            .map_or_else(String::new, |peak| format!(
                                " (peak {})",
                                crate::format::bytes(peak * 1024 * 1024)
                            ))
                    )),
                    _ => None,
                };
                memory.push_row(Row::new(label, Value::from_option(text, NOT_REPORTED_OS)));
            }
        }
        Err(error) => memory.push_row(Row::unavailable("Page file", error.to_string())),
    }
    section.push_group(memory);
    section
}

const NOT_REPORTED_OS: &str = "not reported by Windows";

/// Standard time zone name and its UTC offset, from the dynamic time zone.
fn time_zone() -> Option<String> {
    use windows::Win32::System::Time::{
        DYNAMIC_TIME_ZONE_INFORMATION, GetDynamicTimeZoneInformation,
    };
    let mut info = DYNAMIC_TIME_ZONE_INFORMATION::default();
    // SAFETY: writable output structure.
    let kind = unsafe { GetDynamicTimeZoneInformation(&mut info) };
    if kind == u32::MAX {
        return None;
    }
    let name = crate::specs::native::utf16_until_nul(&info.StandardName);
    let offset = -info.Bias;
    let text = format!(
        "{name} (UTC{}{:02}:{:02})",
        if offset < 0 { '-' } else { '+' },
        offset.abs() / 60,
        offset.abs() % 60
    );
    (!name.trim().is_empty()).then_some(text)
}

/// The active power scheme's friendly name. Read-only power policy calls.
fn power_plan() -> Result<String, NativeError> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::System::Power::{PowerGetActiveScheme, PowerReadFriendlyName};
    let mut scheme: *mut windows::core::GUID = std::ptr::null_mut();
    // SAFETY: output pointer allocated by Windows and freed below.
    let status = unsafe { PowerGetActiveScheme(None, &mut scheme) };
    if status.0 != 0 || scheme.is_null() {
        return Err(NativeError::win32("PowerGetActiveScheme", status.0));
    }
    let mut size = 0u32;
    // SAFETY: size query for the scheme's name.
    let _ = unsafe { PowerReadFriendlyName(None, Some(scheme), None, None, None, &mut size) };
    let result = if (2..=4096).contains(&size) {
        let mut buffer = vec![0u8; size as usize];
        // SAFETY: the buffer holds `size` bytes.
        let status = unsafe {
            PowerReadFriendlyName(
                None,
                Some(scheme),
                None,
                None,
                Some(buffer.as_mut_ptr()),
                &mut size,
            )
        };
        if status.0 == 0 {
            crate::specs::native::utf16_bytes_until_nul(buffer.get(..size as usize).unwrap_or(&[]))
                .filter(|name| !name.trim().is_empty())
                .ok_or(NativeError::Malformed("power scheme name"))
        } else {
            Err(NativeError::win32("PowerReadFriendlyName", status.0))
        }
    } else {
        Err(NativeError::Malformed("power scheme name size"))
    };
    // SAFETY: frees the scheme GUID PowerGetActiveScheme allocated.
    unsafe {
        let _ = LocalFree(Some(HLOCAL(scheme.cast())));
    }
    result
}
