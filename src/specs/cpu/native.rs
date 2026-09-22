//! Windows and CPUID reads for the CPU section. Read-only: CPUID, processor
//! topology, power information, one registry value and the SMBIOS copy.
use super::*;
use crate::specs::native::{NativeError, registry, smbios};
use windows::Win32::System::Power::{CallNtPowerInformation, ProcessorInformation};
use windows::Win32::System::SystemInformation::{
    GetLogicalProcessorInformationEx, GetSystemInfo, RelationAll, SYSTEM_INFO,
};
use windows::Win32::System::Threading::{IsProcessorFeaturePresent, PROCESSOR_FEATURE_ID};

/// PF_SECOND_LEVEL_ADDRESS_TRANSLATION and PF_VIRT_FIRMWARE_ENABLED (winnt.h).
const PF_SLAT: u32 = 20;
const PF_VIRT_FIRMWARE: u32 = 21;

pub(super) fn collect(ctx: &Context) -> Section {
    let mut facts = Facts {
        cpuid: cpuid(),
        topology: topology().map_err(|e| e.to_string()),
        nominal_mhz: nominal_mhz().map_err(|e| e.to_string()),
        ..Default::default()
    };
    // SAFETY: plain feature queries with documented constants.
    unsafe {
        facts.firmware_flag =
            Some(IsProcessorFeaturePresent(PROCESSOR_FEATURE_ID(PF_VIRT_FIRMWARE)).as_bool());
        facts.slat_flag = Some(IsProcessorFeaturePresent(PROCESSOR_FEATURE_ID(PF_SLAT)).as_bool());
    }
    if let Err(reason) = &facts.topology {
        facts.issues.push(format!("Processor topology: {reason}"));
    }
    if ctx.should_stop() {
        facts
            .issues
            .push("Read budget exhausted; microcode and socket not read.".into());
        return build(facts);
    }
    facts.microcode = match registry::read(
        registry::Hive::LocalMachine,
        r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
        "Update Revision",
    ) {
        Ok(Some(registry::RegValue::Binary(bytes))) => microcode(&bytes),
        _ => None,
    };
    match smbios::read() {
        Ok(table) => {
            facts.socket = table.of_type(4).next().map(|s| {
                (
                    s.string(0x04).filter(|t| !smbios::is_placeholder(t)),
                    s.word(0x12),
                )
            });
        }
        Err(error) => facts.issues.push(format!("SMBIOS: {error}")),
    }
    build(facts)
}

/// Register bytes in the order given, as ASCII up to the first NUL.
#[cfg(target_arch = "x86_64")]
fn ascii(registers: &[u32]) -> String {
    let bytes = registers
        .iter()
        .flat_map(|r| r.to_le_bytes())
        .take_while(|b| *b != 0)
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

#[cfg(target_arch = "x86_64")]
fn cpuid() -> Option<Cpuid> {
    use std::arch::x86_64::__cpuid_count;
    // SAFETY: CPUID is always available on x86_64 and only reads identification registers.
    let leaf = |leaf: u32, sub: u32| unsafe { __cpuid_count(leaf, sub) };
    let zero = leaf(0, 0);
    let max = zero.eax;
    let mut result = Cpuid {
        vendor: ascii(&[zero.ebx, zero.edx, zero.ecx]),
        ..Default::default()
    };
    if max >= 1 {
        let one = leaf(1, 0);
        result.signature = one.eax;
        result.leaf1 = (one.ecx, one.edx);
    }
    if max >= 7 {
        let seven = leaf(7, 0);
        result.leaf7 = (seven.ebx, seven.ecx, seven.edx);
        if seven.eax >= 1 {
            result.leaf7_1_eax = leaf(7, 1).eax;
        }
    }
    if max >= 0x16 {
        let f = leaf(0x16, 0);
        let values = (f.eax & 0xFFFF, f.ebx & 0xFFFF, f.ecx & 0xFFFF);
        if values != (0, 0, 0) {
            result.frequency = Some(values);
        }
    }
    let extended = leaf(0x8000_0000, 0).eax;
    if extended >= 0x8000_0001 {
        let e = leaf(0x8000_0001, 0);
        result.extended1 = (e.ecx, e.edx);
    }
    if extended >= 0x8000_0004 {
        let registers = (0x8000_0002..=0x8000_0004)
            .flat_map(|l| {
                let r = leaf(l, 0);
                [r.eax, r.ebx, r.ecx, r.edx]
            })
            .collect::<Vec<_>>();
        result.brand = Some(ascii(&registers)).filter(|b| !b.is_empty());
    }
    if result.hypervisor_bit() {
        let h = leaf(0x4000_0000, 0);
        result.hypervisor = Some(ascii(&[h.ebx, h.ecx, h.edx])).filter(|v| !v.is_empty());
        if result.hypervisor.as_deref() == Some("Microsoft Hv") && h.eax >= 0x4000_0003 {
            // Partition privilege mask, bit 32 of the mask (EBX bit 0): CreatePartitions.
            result.root_partition = Some(leaf(0x4000_0003, 0).ebx & 1 != 0);
        }
    }
    Some(result)
}

#[cfg(not(target_arch = "x86_64"))]
fn cpuid() -> Option<Cpuid> {
    None
}

fn topology() -> Result<Topology, NativeError> {
    const API: &str = "GetLogicalProcessorInformationEx";
    let mut length = 0u32;
    // SAFETY: size query with no buffer.
    let _ = unsafe { GetLogicalProcessorInformationEx(RelationAll, None, &mut length) };
    if !(8..=1024 * 1024).contains(&length) {
        return Err(NativeError::last_error(API));
    }
    // u64 storage keeps the records 8-byte aligned.
    let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
    // SAFETY: the buffer holds `length` bytes.
    unsafe {
        GetLogicalProcessorInformationEx(RelationAll, Some(buffer.as_mut_ptr().cast()), &mut length)
    }
    .map_err(|e| NativeError::from_windows(API, &e))?;
    let bytes = buffer
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .take(length as usize)
        .collect::<Vec<_>>();
    parse_topology(&bytes).ok_or(NativeError::Malformed("processor topology records"))
}

fn nominal_mhz() -> Result<Vec<u32>, NativeError> {
    let mut info = SYSTEM_INFO::default();
    // SAFETY: writable output structure.
    unsafe { GetSystemInfo(&mut info) };
    let count = info.dwNumberOfProcessors.clamp(1, 1024) as usize;
    // PROCESSOR_POWER_INFORMATION: six ULONGs; MaxMhz is the second.
    let mut buffer = vec![0u32; count * 6];
    // SAFETY: output size matches the buffer.
    let status = unsafe {
        CallNtPowerInformation(
            ProcessorInformation,
            None,
            0,
            Some(buffer.as_mut_ptr().cast()),
            (buffer.len() * 4) as u32,
        )
    };
    if status.0 != 0 {
        return Err(NativeError::Windows {
            api: "CallNtPowerInformation",
            code: status.0 as u32,
        });
    }
    Ok(buffer.chunks_exact(6).map(|p| p[1]).collect())
}
