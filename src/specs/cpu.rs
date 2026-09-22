//! CPU section (lane: cpu).
//!
//! Scope: full CPUID brand string (never a bare "Intel Core"), vendor, family/
//! model/stepping, codename only from a documented table, sockets/cores/threads
//! per core type on hybrid parts (P/E), per-core-type cache sizes (L1d/L1i/L2/
//! L3 from GetLogicalProcessorInformationEx), instruction set flags, base clock
//! and live clocks (LiveKey::CpuClockAverage/CpuClockFastest/CpuCoreClock),
//! virtualization capability vs enabled vs in use, package temperature only via
//! LiveKey::CpuPackageTemperature (bridge). No ACPI thermal zones as CPU temps.
use super::{Context, Group, LiveKey, Row, Section, SectionId, SummaryLine, Value};
use std::collections::BTreeMap;

#[cfg(windows)]
mod native;

/// Bridge sensor ids for CPU readings that have no dedicated LiveKey.
pub const PACKAGE_POWER: &str = "cpu.package_power";
pub const CORE_VOLTAGE: &str = "cpu.core_voltage";

const NO_DRIVER: &str = "not exposed by Windows without a kernel driver";

/// One physical core from GetLogicalProcessorInformationEx.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Core {
    /// Higher is faster; equal on non-hybrid parts.
    efficiency: u8,
    smt: bool,
    /// Windows (processor group, number in group) of each logical processor.
    processors: Vec<(u16, u32)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Cache {
    level: u8,
    /// PROCESSOR_CACHE_TYPE: 0 unified, 1 instruction, 2 data, 3 trace.
    kind: u32,
    bytes: u32,
    line: u16,
    ways: u8,
    processors: Vec<(u16, u32)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Topology {
    packages: usize,
    cores: Vec<Core>,
    caches: Vec<Cache>,
}

impl Topology {
    fn threads(&self) -> usize {
        self.cores.iter().map(|c| c.processors.len()).sum()
    }

    fn classes(&self) -> Vec<u8> {
        let mut classes = self.cores.iter().map(|c| c.efficiency).collect::<Vec<_>>();
        classes.sort_unstable_by(|a, b| b.cmp(a));
        classes.dedup();
        classes
    }

    fn hybrid(&self) -> bool {
        self.classes().len() > 1
    }

    /// "Performance" for the fastest class, "Efficient" for the slowest.
    fn class_name(&self, class: u8) -> String {
        let classes = self.classes();
        if classes.len() < 2 {
            "Core".into()
        } else if Some(&class) == classes.first() {
            "Performance".into()
        } else if Some(&class) == classes.last() {
            "Efficient".into()
        } else {
            format!("Efficiency class {class}")
        }
    }

    fn class_of(&self, processor: (u16, u32)) -> Option<u8> {
        self.cores
            .iter()
            .find(|c| c.processors.contains(&processor))
            .map(|c| c.efficiency)
    }
}

/// Parses a RelationAll GetLogicalProcessorInformationEx buffer by offsets,
/// never by casting driver-sized records. None for malformed data.
fn parse_topology(bytes: &[u8]) -> Option<Topology> {
    let u16_at = |at: usize| -> Option<u16> {
        bytes
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_at = |at: usize| -> Option<u32> {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    // GROUP_AFFINITY: 8-byte mask, 2-byte group, 6 reserved bytes.
    let masks = |at: usize, count: u16, end: usize| -> Option<Vec<(u16, u32)>> {
        let mut processors = Vec::new();
        for index in 0..count.max(1) as usize {
            let offset = at + index * 16;
            if offset + 16 > end {
                return None;
            }
            let mask = u64::from_le_bytes(bytes.get(offset..offset + 8)?.try_into().ok()?);
            let group = u16_at(offset + 8)?;
            processors.extend(
                (0..64)
                    .filter(|bit| mask & (1u64 << bit) != 0)
                    .map(|bit| (group, bit)),
            );
        }
        Some(processors)
    };
    let mut topology = Topology::default();
    let mut at = 0;
    while at < bytes.len() {
        let relation = u32_at(at)?;
        let size = u32_at(at + 4)? as usize;
        let end = at.checked_add(size)?;
        if size < 8 || end > bytes.len() {
            return None;
        }
        match relation {
            // RelationProcessorCore
            0 => {
                let flags = *bytes.get(at + 8)?;
                let efficiency = *bytes.get(at + 9)?;
                let count = u16_at(at + 30)?;
                topology.cores.push(Core {
                    efficiency,
                    smt: flags & 1 != 0,
                    processors: masks(at + 32, count, end)?,
                });
            }
            // RelationCache
            2 => {
                let count = u16_at(at + 38)?;
                topology.caches.push(Cache {
                    level: *bytes.get(at + 8)?,
                    ways: *bytes.get(at + 9)?,
                    line: u16_at(at + 10)?,
                    bytes: u32_at(at + 12)?,
                    kind: u32_at(at + 16)?,
                    processors: masks(at + 40, count, end)?,
                });
            }
            // RelationProcessorPackage
            3 => topology.packages += 1,
            _ => {}
        }
        at = end;
    }
    (!topology.cores.is_empty()).then_some(topology)
}

/// Raw CPUID results the section needs. Filled on x86_64 only.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Cpuid {
    vendor: String,
    brand: Option<String>,
    /// Leaf 1 EAX.
    signature: u32,
    leaf1: (u32, u32),
    leaf7: (u32, u32, u32),
    leaf7_1_eax: u32,
    extended1: (u32, u32),
    /// Leaf 16h base, maximum and bus MHz (Intel), when non-zero.
    frequency: Option<(u32, u32, u32)>,
    /// Hypervisor vendor signature (leaf 4000_0000h), when the hypervisor bit is set.
    hypervisor: Option<String>,
    /// Microsoft hypervisor: CreatePartitions privilege, which only the root partition holds.
    root_partition: Option<bool>,
}

impl Cpuid {
    fn family_model_stepping(&self) -> (u32, u32, u32) {
        let base_family = (self.signature >> 8) & 0xF;
        let family = if base_family == 0xF {
            base_family + ((self.signature >> 20) & 0xFF)
        } else {
            base_family
        };
        let base_model = (self.signature >> 4) & 0xF;
        let model = if base_family == 0x6 || base_family == 0xF {
            base_model | (((self.signature >> 16) & 0xF) << 4)
        } else {
            base_model
        };
        (family, model, self.signature & 0xF)
    }

    fn hypervisor_bit(&self) -> bool {
        self.leaf1.0 & (1 << 31) != 0
    }

    fn vmx(&self) -> bool {
        self.leaf1.0 & (1 << 5) != 0
    }

    fn svm(&self) -> bool {
        self.extended1.0 & (1 << 2) != 0
    }

    fn intel(&self) -> bool {
        self.vendor == "GenuineIntel"
    }

    /// Instruction set extensions the processor reports, in Speccy's order.
    fn instructions(&self) -> Vec<&'static str> {
        let (ecx1, edx1) = self.leaf1;
        let (ebx7, ecx7, _) = self.leaf7;
        let (ecx_ext, edx_ext) = self.extended1;
        let bit = |value: u32, index: u32| value & (1 << index) != 0;
        [
            (bit(edx1, 23), "MMX"),
            (bit(edx1, 25), "SSE"),
            (bit(edx1, 26), "SSE2"),
            (bit(ecx1, 0), "SSE3"),
            (bit(ecx1, 9), "SSSE3"),
            (bit(ecx1, 19), "SSE4.1"),
            (bit(ecx1, 20), "SSE4.2"),
            (bit(ecx_ext, 6), "SSE4a"),
            (
                bit(edx_ext, 29),
                if self.intel() { "Intel 64" } else { "AMD64" },
            ),
            (bit(edx_ext, 20), "NX"),
            (bit(ecx1, 25), "AES"),
            (bit(ecx1, 1), "PCLMULQDQ"),
            (bit(ecx1, 28), "AVX"),
            (bit(ebx7, 5), "AVX2"),
            (bit(self.leaf7_1_eax, 4), "AVX-VNNI"),
            (bit(ebx7, 16), "AVX-512F"),
            (bit(ecx1, 12), "FMA3"),
            (bit(ecx1, 29), "F16C"),
            (bit(ebx7, 3), "BMI1"),
            (bit(ebx7, 8), "BMI2"),
            (bit(ebx7, 19), "ADX"),
            (bit(ebx7, 29), "SHA"),
            (bit(ecx7, 8), "GFNI"),
            (bit(ecx7, 9), "VAES"),
            (bit(ecx1, 30), "RDRAND"),
            (bit(ebx7, 18), "RDSEED"),
            (bit(ecx1, 22), "MOVBE"),
            (bit(ecx1, 23), "POPCNT"),
            (bit(ecx1, 5), "VT-x"),
            (bit(ecx_ext, 2), "AMD-V"),
        ]
        .into_iter()
        .filter_map(|(present, name)| present.then_some(name))
        .collect()
    }
}

/// Codenames from Linux arch/x86/include/asm/intel-family.h (Intel family 6)
/// and AMD's family numbering. Anything else is not guessed.
fn codename(vendor: &str, family: u32, model: u32) -> Option<&'static str> {
    match (vendor, family, model) {
        ("GenuineIntel", 6, 0xC6) => Some("Arrow Lake-S"),
        ("GenuineIntel", 6, 0xC5) => Some("Arrow Lake-H"),
        ("GenuineIntel", 6, 0xB5) => Some("Arrow Lake-U"),
        ("GenuineIntel", 6, 0xBD) => Some("Lunar Lake"),
        ("GenuineIntel", 6, 0xCC) => Some("Panther Lake"),
        ("GenuineIntel", 6, 0xAA | 0xAC) => Some("Meteor Lake"),
        ("GenuineIntel", 6, 0xB7 | 0xBA | 0xBF) => Some("Raptor Lake"),
        ("GenuineIntel", 6, 0x97 | 0x9A) => Some("Alder Lake"),
        ("GenuineIntel", 6, 0xBE) => Some("Alder Lake-N"),
        ("GenuineIntel", 6, 0xA7) => Some("Rocket Lake"),
        ("GenuineIntel", 6, 0xA5 | 0xA6) => Some("Comet Lake"),
        ("GenuineIntel", 6, 0x8C | 0x8D) => Some("Tiger Lake"),
        ("GenuineIntel", 6, 0x7D | 0x7E) => Some("Ice Lake"),
        ("GenuineIntel", 6, 0x8F) => Some("Sapphire Rapids"),
        ("GenuineIntel", 6, 0xCF) => Some("Emerald Rapids"),
        ("AuthenticAMD", 0x1A, _) => Some("Zen 5"),
        _ => None,
    }
}

/// Desktop sockets fixed by the CPU model (Intel ARK product data). Mobile
/// models (BGA) and anything not listed are not guessed.
fn model_socket(vendor: &str, family: u32, model: u32) -> Option<&'static str> {
    match (vendor, family, model) {
        ("GenuineIntel", 6, 0xC6) => Some("LGA1851"),
        ("GenuineIntel", 6, 0x97 | 0xB7 | 0xBF) => Some("LGA1700"),
        ("GenuineIntel", 6, 0xA5 | 0xA7) => Some("LGA1200"),
        _ => None,
    }
}
/// The registry's "Update Revision": 4 bytes on current Windows, or the raw
/// 8-byte MSR 8Bh value (Intel keeps the revision in the high half).
fn microcode(bytes: &[u8]) -> Option<u32> {
    let word = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let revision = match bytes.len() {
        4 => word(0)?,
        8 => word(4).filter(|high| *high != 0).or_else(|| word(0))?,
        _ => return None,
    };
    (revision != 0).then_some(revision)
}

fn size_text(bytes: u64) -> String {
    const KB: u64 = 1024;
    if bytes >= KB * KB && bytes.is_multiple_of(KB * KB / 4) {
        let mb = bytes as f64 / (KB * KB) as f64;
        if mb.fract() == 0.0 {
            format!("{mb:.0} MB")
        } else {
            format!("{mb:.2} MB")
        }
    } else {
        format!("{} KB", bytes / KB)
    }
}

fn cache_label(level: u8, kind: u32) -> String {
    match kind {
        1 => format!("L{level} instruction"),
        2 => format!("L{level} data"),
        3 => format!("L{level} trace"),
        _ => format!("L{level}"),
    }
}

/// One row per cache level and type: "8 x 48 KB (Performance), 16 x 32 KB
/// (Efficient)". Shared caches say which core types share them.
fn cache_rows(topology: &Topology) -> Vec<Row> {
    let mut grouped = BTreeMap::<(u8, u32), BTreeMap<(String, u32, u16, u8), usize>>::new();
    for cache in &topology.caches {
        let mut classes = cache
            .processors
            .iter()
            .filter_map(|p| topology.class_of(*p))
            .collect::<Vec<_>>();
        classes.sort_unstable_by(|a, b| b.cmp(a));
        classes.dedup();
        let users = if topology.hybrid() {
            classes
                .iter()
                .map(|c| topology.class_name(*c))
                .collect::<Vec<_>>()
                .join(" + ")
        } else {
            String::new()
        };
        *grouped
            .entry((cache.level, cache.kind))
            .or_default()
            .entry((users, cache.bytes, cache.line, cache.ways))
            .or_default() += 1;
    }
    grouped
        .into_iter()
        .map(|((level, kind), variants)| {
            let text = variants
                .iter()
                .rev()
                .map(|((users, bytes, _, _), count)| {
                    let size = size_text(u64::from(*bytes));
                    let amount = if *count == 1 {
                        size
                    } else {
                        format!("{count} x {size}")
                    };
                    if users.is_empty() {
                        amount
                    } else {
                        format!("{amount} ({users})")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let detail = variants
                .keys()
                .map(|(users, _, line, ways)| {
                    let ways = match ways {
                        0xFF => "fully associative".to_string(),
                        0 => "associativity not reported".to_string(),
                        w => format!("{w}-way"),
                    };
                    let who = if users.is_empty() { "" } else { users };
                    format!("{who} {line}-byte lines, {ways}")
                        .trim()
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join("; ");
            Row::known(cache_label(level, kind), text)
                .note(format!("GetLogicalProcessorInformationEx: {detail}"))
        })
        .collect()
}

fn core_counts(topology: &Topology) -> String {
    let cores = topology.cores.len();
    if !topology.hybrid() {
        return cores.to_string();
    }
    let parts = topology
        .classes()
        .into_iter()
        .map(|class| {
            let count = topology
                .cores
                .iter()
                .filter(|c| c.efficiency == class)
                .count();
            format!("{count} {}", topology.class_name(class))
        })
        .collect::<Vec<_>>()
        .join(" + ");
    format!("{cores} ({parts})")
}

/// Groups of per-core live clock rows, one group per core type.
fn core_groups(topology: &Topology) -> Vec<Group> {
    topology
        .classes()
        .into_iter()
        .map(|class| {
            let cores = topology
                .cores
                .iter()
                .filter(|c| c.efficiency == class)
                .collect::<Vec<_>>();
            let name = topology.class_name(class);
            let title = if topology.hybrid() {
                format!("{name} cores ({})", cores.len())
            } else {
                format!("Cores ({})", cores.len())
            };
            let mut group = Group::new(title).collapsed();
            for (index, core) in cores.iter().enumerate() {
                for (thread, (processor_group, number)) in core.processors.iter().enumerate() {
                    let label = if core.processors.len() > 1 {
                        format!("Core {index} thread {thread}")
                    } else {
                        format!("Core {index}")
                    };
                    group.push_row(
                        Row::live(
                            label,
                            LiveKey::CpuCoreClock {
                                group: *processor_group,
                                number: *number,
                            },
                        )
                        .note(format!(
                            "Windows logical processor: group {processor_group}, number {number}. \
                             Effective clock over the sampler interval."
                        )),
                    );
                }
            }
            group
        })
        .collect()
}

/// What the virtualization rows say, from CPUID, Windows and the hypervisor.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Virtualization {
    capability: Value,
    firmware: Value,
    slat: Value,
    hypervisor: Value,
}

fn virtualization(
    cpuid: &Cpuid,
    firmware_flag: Option<bool>,
    slat_flag: Option<bool>,
) -> Virtualization {
    let technology = if cpuid.intel() {
        "Intel VT-x"
    } else {
        "AMD-V (SVM)"
    };
    let yes_no = |flag: bool| if flag { "Yes" } else { "No" };
    let microsoft = cpuid.hypervisor.as_deref() == Some("Microsoft Hv");
    match (cpuid.hypervisor_bit(), microsoft, cpuid.root_partition) {
        // Windows is the Hyper-V root: the hypervisor hides VMX/SVM from CPUID,
        // but it cannot run at all without the capability, firmware enablement
        // and second level address translation.
        (true, true, Some(true)) => Virtualization {
            capability: Value::known(format!(
                "{technology}: supported (CPUID hides it while Hyper-V runs)"
            )),
            firmware: Value::known("Yes (required for the running hypervisor)"),
            slat: Value::known("Yes (required by Hyper-V)"),
            hypervisor: Value::known(
                "In use: Microsoft Hyper-V is running, Windows is the root partition",
            ),
        },
        (true, _, _) => {
            let name = cpuid.hypervisor.clone().unwrap_or_else(|| "unknown".into());
            Virtualization {
                capability: Value::unavailable(
                    "running inside a virtual machine; the host CPU's capability is not visible",
                ),
                firmware: Value::unavailable("not visible from inside a virtual machine"),
                slat: Value::unavailable("not visible from inside a virtual machine"),
                hypervisor: Value::known(format!("Guest of hypervisor \"{name}\"")),
            }
        }
        (false, _, _) => {
            let capable = cpuid.vmx() || cpuid.svm();
            Virtualization {
                capability: Value::known(if capable {
                    format!("{technology}: supported")
                } else {
                    format!("{technology}: not supported by this CPU")
                }),
                firmware: Value::from_option(
                    firmware_flag.map(yes_no),
                    "Windows did not report the firmware state",
                ),
                slat: Value::from_option(
                    slat_flag.map(yes_no),
                    "Windows did not report second level address translation",
                ),
                hypervisor: Value::known("Not in use: no hypervisor is running"),
            }
        }
    }
}

pub fn collect(ctx: &Context) -> Section {
    #[cfg(windows)]
    {
        native::collect(ctx)
    }
    #[cfg(not(windows))]
    {
        let _ = ctx;
        Section::unavailable(SectionId::Cpu, "CPU details are read on Windows only")
    }
}

/// Everything the native side gathered; mapping to rows is portable.
#[derive(Clone, Debug)]
struct Facts {
    cpuid: Option<Cpuid>,
    topology: Result<Topology, String>,
    /// PROCESSOR_POWER_INFORMATION MaxMhz by logical processor index (group 0).
    nominal_mhz: Result<Vec<u32>, String>,
    firmware_flag: Option<bool>,
    slat_flag: Option<bool>,
    microcode: Option<u32>,
    /// SMBIOS type 4: socket designation and external clock MHz.
    socket: Option<(Option<String>, Option<u16>)>,
    issues: Vec<String>,
}

impl Default for Facts {
    fn default() -> Self {
        Self {
            cpuid: None,
            topology: Err("not read".into()),
            nominal_mhz: Err("not read".into()),
            firmware_flag: None,
            slat_flag: None,
            microcode: None,
            socket: None,
            issues: Vec::new(),
        }
    }
}

fn build(facts: Facts) -> Section {
    let mut section = Section::new(SectionId::Cpu);
    for issue in &facts.issues {
        section.push_issue(issue.clone());
    }
    let cpuid = facts.cpuid.clone().unwrap_or_default();
    let brand = cpuid
        .brand
        .clone()
        .map(|b| b.split_whitespace().collect::<Vec<_>>().join(" "));
    let (family, model, stepping) = cpuid.family_model_stepping();
    let code = codename(&cpuid.vendor, family, model);
    let topology = facts.topology.as_ref().ok();
    let title = brand.clone().unwrap_or_else(|| "Processor".into());

    let mut headline = brand.clone().unwrap_or_else(|| "Processor".into());
    if let Some(code) = code {
        headline.push_str(&format!(" / {code}"));
    }
    if let Some(topology) = topology {
        let counts = topology
            .classes()
            .into_iter()
            .map(|class| {
                let count = topology
                    .cores
                    .iter()
                    .filter(|c| c.efficiency == class)
                    .count();
                let name = topology.class_name(class);
                format!("{count}{}", name.chars().next().unwrap_or('C'))
            })
            .collect::<Vec<_>>();
        headline.push_str(&if topology.hybrid() {
            format!(" / {} cores ({})", topology.cores.len(), counts.join(" + "))
        } else {
            format!(" / {} cores", topology.cores.len())
        });
    }
    section.push_summary(SummaryLine::known(headline).live(LiveKey::CpuPackageTemperature));

    let mut identity = Group::new(title.clone()).live(LiveKey::CpuPackageTemperature);
    identity.push_row(
        Row::new(
            "Name",
            Value::from_option(brand.clone(), "CPUID brand string unavailable"),
        )
        .note("CPUID leaves 8000_0002h to 8000_0004h (full brand string)"),
    );
    identity.push_row(Row::new(
        "Vendor",
        if cpuid.vendor.is_empty() {
            Value::unavailable("CPUID is not available on this architecture")
        } else {
            Value::known(match cpuid.vendor.as_str() {
                "GenuineIntel" => "Intel (GenuineIntel)".to_string(),
                "AuthenticAMD" => "AMD (AuthenticAMD)".to_string(),
                other => other.to_string(),
            })
        },
    ));
    identity.push_row(
        Row::new(
            "Codename",
            Value::from_option(code, "not in Trontop's documented family/model table"),
        )
        .note("From the CPUID family/model pair (Linux intel-family.h); never guessed"),
    );
    if cpuid.signature != 0 {
        identity.push_row(Row::known(
            "Family / model / stepping",
            format!("{family} / {model} ({model:02X}h) / {stepping}"),
        ));
        identity.push_row(
            Row::known("CPUID signature", format!("{:08X}h", cpuid.signature))
                .note("Leaf 1 EAX; identifies the model, not the individual chip"),
        );
    }
    identity.push_row(Row::new(
        "Microcode revision",
        Value::from_option(
            facts.microcode.map(|r| format!("{r:X}h")),
            "Windows did not record the loaded microcode revision",
        ),
    ));
    let board_socket = facts
        .socket
        .as_ref()
        .and_then(|s| s.0.clone())
        .filter(|name| {
            !name.eq_ignore_ascii_case("CPUSocket") && !name.eq_ignore_ascii_case("CPU 1")
        });
    let socket = match (model_socket(&cpuid.vendor, family, model), board_socket) {
        (Some(socket), Some(name)) => Some(format!("{socket} (board label \"{name}\")")),
        (Some(socket), None) => Some(socket.to_string()),
        (None, name) => name,
    };
    identity.push_row(
        Row::new(
            "Socket",
            Value::from_option(
                socket,
                "not identified by the CPU model or the board's SMBIOS",
            ),
        )
        .note("From the CPU model (desktop parts only) or the SMBIOS type 4 socket label"),
    );
    match topology {
        Some(topology) => {
            identity.push_row(Row::known("Packages", topology.packages.max(1).to_string()));
            identity.push_row(Row::known("Cores", core_counts(topology)));
            let smt = topology.cores.iter().any(|c| c.smt);
            identity.push_row(Row::known(
                "Threads",
                format!(
                    "{}{}",
                    topology.threads(),
                    if smt {
                        " (simultaneous multithreading on)"
                    } else {
                        " (one per core)"
                    }
                ),
            ));
            identity.push_row(Row::known(
                "Hybrid architecture",
                if topology.hybrid() {
                    "Yes (performance and efficient cores)"
                } else {
                    "No"
                },
            ));
        }
        None => identity.push_row(Row::new(
            "Cores",
            Value::from_result::<String, _>(facts.topology.clone().map(|_| String::new())),
        )),
    }
    let instructions = cpuid.instructions();
    identity.push_row(
        Row::new(
            "Instruction sets",
            if instructions.is_empty() {
                Value::unavailable("CPUID is not available on this architecture")
            } else {
                Value::known(instructions.join(", "))
            },
        )
        .note("As reported by CPUID; VT-x is hidden by CPUID while Hyper-V runs"),
    );
    identity.push_row(Row::live("Usage", LiveKey::CpuUsage));
    identity.push_row(
        Row::live("Package temperature", LiveKey::CpuPackageTemperature)
            .note("From a running sensor provider only; never an ACPI thermal zone"),
    );
    identity.push_row(Row::live(
        "Package power",
        LiveKey::Sensor {
            id: PACKAGE_POWER.into(),
        },
    ));
    identity.push_row(Row::live(
        "Core voltage",
        LiveKey::Sensor {
            id: CORE_VOLTAGE.into(),
        },
    ));
    identity.push_row(Row::unavailable("TDP", NO_DRIVER));
    section.push_group(identity);

    // Clocks.
    let mut clocks = Group::new("Clocks");
    clocks.push_row(
        Row::live("Average", LiveKey::CpuClockAverage)
            .note("Effective clock over the sampler interval, all logical processors"),
    );
    clocks.push_row(Row::live("Fastest core", LiveKey::CpuClockFastest));
    match &facts.nominal_mhz {
        Ok(mhz) if !mhz.is_empty() => {
            let mut per_class = BTreeMap::<u8, Vec<u32>>::new();
            let mut single = Vec::new();
            for (index, value) in mhz.iter().enumerate() {
                match topology.and_then(|t| t.class_of((0, index as u32))) {
                    Some(class) if topology.is_some_and(Topology::hybrid) => {
                        per_class.entry(class).or_default().push(*value)
                    }
                    _ => single.push(*value),
                }
            }
            if let Some(topology) = topology.filter(|t| t.hybrid()) {
                for (class, values) in per_class.iter().rev() {
                    let (low, high) = (
                        values.iter().min().copied().unwrap_or(0),
                        values.iter().max().copied().unwrap_or(0),
                    );
                    let text = if low == high {
                        format!("{low} MHz")
                    } else {
                        format!("{low} to {high} MHz")
                    };
                    clocks.push_row(
                        Row::known(
                            format!("Base clock ({})", topology.class_name(*class)),
                            text,
                        )
                        .note("CallNtPowerInformation ProcessorInformation MaxMhz (nominal)"),
                    );
                }
            } else if let Some(max) = single.iter().max() {
                clocks.push_row(
                    Row::known("Base clock", format!("{max} MHz"))
                        .note("CallNtPowerInformation ProcessorInformation MaxMhz (nominal)"),
                );
            }
        }
        Ok(_) => clocks.push_row(Row::unavailable("Base clock", "no processors reported")),
        Err(reason) => clocks.push_row(Row::unavailable("Base clock", reason.clone())),
    }
    match cpuid.frequency {
        Some((base, max, bus)) => {
            if max > 0 {
                clocks.push_row(
                    Row::known("Maximum (rated)", format!("{max} MHz"))
                        .note("CPUID leaf 16h, as the processor reports it"),
                );
            }
            if base > 0 {
                clocks.push_row(
                    Row::known("Base (rated)", format!("{base} MHz"))
                        .note("CPUID leaf 16h, as the processor reports it"),
                );
            }
            if bus > 0 {
                clocks.push_row(
                    Row::known("Bus (reference) clock", format!("{bus} MHz"))
                        .note("CPUID leaf 16h"),
                );
            }
        }
        None => {
            let external = facts.socket.as_ref().and_then(|s| s.1).filter(|v| *v > 0);
            clocks.push_row(
                Row::new(
                    "Bus (reference) clock",
                    Value::from_option(
                        external.map(|v| format!("{v} MHz")),
                        "not reported by CPUID leaf 16h or SMBIOS",
                    ),
                )
                .note("SMBIOS type 4 external clock"),
            );
        }
    }
    section.push_group(clocks);

    if let Some(topology) = topology {
        let mut caches = Group::new("Caches");
        caches.push_row(Row::known(
            "Per core type",
            if topology.hybrid() {
                "Yes: sizes below are split by Performance and Efficient cores"
            } else {
                "Not a hybrid processor"
            },
        ));
        for row in cache_rows(topology) {
            caches.push_row(row);
        }
        section.push_group(caches);
    }

    let virt = virtualization(&cpuid, facts.firmware_flag, facts.slat_flag);
    section.push_group(
        Group::new("Virtualization")
            .row(Row::new("Hardware virtualization", virt.capability).note(
                "CPU capability; CPUID leaf 1 ECX bit 5 (VT-x) or 8000_0001h ECX bit 2 (AMD-V)",
            ))
            .row(Row::new("Enabled in firmware", virt.firmware))
            .row(Row::new("Second level address translation", virt.slat))
            .row(
                Row::new("Hypervisor", virt.hypervisor)
                    .note("CPUID hypervisor leaves 4000_0000h and 4000_0003h"),
            ),
    );

    if let Some(topology) = topology {
        for group in core_groups(topology) {
            section.push_group(group);
        }
    }
    section
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic GLPI buffer: two P-cores, two E-cores, private and shared caches.
    fn glpi() -> Vec<u8> {
        fn record(relation: u32, body: &[u8]) -> Vec<u8> {
            let mut bytes = relation.to_le_bytes().to_vec();
            bytes.extend_from_slice(&((8 + body.len()) as u32).to_le_bytes());
            bytes.extend_from_slice(body);
            bytes
        }
        fn affinity(mask: u64) -> Vec<u8> {
            let mut bytes = mask.to_le_bytes().to_vec();
            bytes.extend_from_slice(&[0; 8]);
            bytes
        }
        fn core(efficiency: u8, mask: u64) -> Vec<u8> {
            let mut body = vec![0u8; 24];
            body[1] = efficiency;
            body[22] = 1;
            body.extend(affinity(mask));
            record(0, &body)
        }
        fn cache(level: u8, kind: u32, size: u32, mask: u64) -> Vec<u8> {
            let mut body = vec![0u8; 32];
            body[0] = level;
            body[1] = 12;
            body[2..4].copy_from_slice(&64u16.to_le_bytes());
            body[4..8].copy_from_slice(&size.to_le_bytes());
            body[8..12].copy_from_slice(&kind.to_le_bytes());
            body[30] = 1;
            body.extend(affinity(mask));
            record(2, &body)
        }
        let mut bytes = record(3, &[0u8; 32]);
        bytes.extend(core(1, 0b0001));
        bytes.extend(core(1, 0b0010));
        bytes.extend(core(0, 0b0100));
        bytes.extend(core(0, 0b1000));
        bytes.extend(cache(1, 2, 48 * 1024, 0b0001));
        bytes.extend(cache(1, 2, 48 * 1024, 0b0010));
        bytes.extend(cache(1, 2, 32 * 1024, 0b0100));
        bytes.extend(cache(1, 2, 32 * 1024, 0b1000));
        bytes.extend(cache(2, 0, 4 * 1024 * 1024, 0b1100));
        bytes.extend(cache(3, 0, 36 * 1024 * 1024, 0b1111));
        bytes.extend(record(1, &[0u8; 16]));
        bytes
    }

    #[test]
    fn topology_splits_cores_and_caches_by_core_type() {
        let topology = parse_topology(&glpi()).expect("topology");
        assert_eq!(topology.packages, 1);
        assert_eq!(topology.threads(), 4);
        assert!(topology.hybrid());
        assert_eq!(core_counts(&topology), "4 (2 Performance + 2 Efficient)");
        let rows = cache_rows(&topology);
        let text = |label: &str| {
            rows.iter()
                .find(|r| r.label == label)
                .and_then(Row::display_text)
                .unwrap_or_default()
        };
        assert_eq!(
            text("L1 data"),
            "2 x 48 KB (Performance), 2 x 32 KB (Efficient)"
        );
        assert_eq!(text("L2"), "4 MB (Efficient)");
        assert_eq!(text("L3"), "36 MB (Performance + Efficient)");
        let groups = core_groups(&topology);
        assert_eq!(groups[0].title, "Performance cores (2)");
        assert_eq!(groups[1].title, "Efficient cores (2)");
        assert_eq!(
            groups[1].items.len(),
            2,
            "one live clock row per logical processor"
        );
    }

    #[test]
    fn malformed_topology_never_panics() {
        let good = glpi();
        for cut in 0..good.len() {
            let _ = parse_topology(&good[..cut]);
        }
        for index in 0..good.len() {
            let mut bad = good.clone();
            bad[index] = 0xFF;
            let _ = parse_topology(&bad);
        }
        assert_eq!(parse_topology(&[]), None);
    }

    #[test]
    fn signature_decodes_family_model_and_codename() {
        let cpuid = Cpuid {
            vendor: "GenuineIntel".into(),
            signature: 0x000C_0662,
            ..Default::default()
        };
        assert_eq!(cpuid.family_model_stepping(), (6, 0xC6, 2));
        assert_eq!(codename("GenuineIntel", 6, 0xC6), Some("Arrow Lake-S"));
        assert_eq!(codename("GenuineIntel", 6, 0x01), None);
        let zen = Cpuid {
            signature: 0x00B4_0F40,
            ..Default::default()
        };
        assert_eq!(zen.family_model_stepping(), (0x1A, 0x44, 0));
    }

    #[test]
    fn virtualization_separates_capability_firmware_and_hypervisor() {
        let root = Cpuid {
            vendor: "GenuineIntel".into(),
            leaf1: (1 << 31, 0),
            hypervisor: Some("Microsoft Hv".into()),
            root_partition: Some(true),
            ..Default::default()
        };
        let v = virtualization(&root, Some(false), Some(false));
        assert!(v.capability.text().unwrap().contains("supported"));
        assert!(v.hypervisor.text().unwrap().contains("root partition"));
        let bare = Cpuid {
            vendor: "GenuineIntel".into(),
            leaf1: (1 << 5, 0),
            ..Default::default()
        };
        let v = virtualization(&bare, Some(true), None);
        assert_eq!(v.capability, Value::known("Intel VT-x: supported"));
        assert_eq!(v.firmware, Value::known("Yes"));
        assert!(!v.slat.is_known());
        let guest = Cpuid {
            leaf1: (1 << 31, 0),
            hypervisor: Some("VMwareVMware".into()),
            ..Default::default()
        };
        assert!(!virtualization(&guest, None, None).capability.is_known());
    }

    #[test]
    fn instruction_list_and_sizes_are_exact() {
        let cpuid = Cpuid {
            vendor: "GenuineIntel".into(),
            leaf1: ((1 << 0) | (1 << 28), (1 << 23) | (1 << 26)),
            leaf7: (1 << 5, 0, 0),
            extended1: (0, 1 << 29),
            ..Default::default()
        };
        assert_eq!(
            cpuid.instructions(),
            ["MMX", "SSE2", "SSE3", "Intel 64", "AVX", "AVX2"]
        );
        assert_eq!(size_text(48 * 1024), "48 KB");
        assert_eq!(size_text(3 * 1024 * 1024), "3 MB");
        assert_eq!(size_text(1280 * 1024), "1.25 MB");
        assert_eq!(microcode(&[0x17, 0x01, 0, 0]), Some(0x117));
        assert_eq!(microcode(&[0, 0, 0, 0, 0x2A, 0, 0, 0]), Some(0x2A));
        assert_eq!(microcode(&[0; 4]), None);
        assert_eq!(microcode(&[1, 2]), None);
    }

    #[test]
    fn built_section_uses_the_full_brand_and_never_invents_temperatures() {
        let facts = Facts {
            cpuid: Some(Cpuid {
                vendor: "GenuineIntel".into(),
                brand: Some("Intel(R) Core(TM) Ultra 9 285K   ".into()),
                signature: 0x000C_0662,
                ..Default::default()
            }),
            topology: parse_topology(&glpi()).ok_or_else(String::new),
            nominal_mhz: Ok(vec![3700, 3700, 3200, 3200]),
            ..Default::default()
        };
        let section = build(facts);
        assert_eq!(section.groups[0].title, "Intel(R) Core(TM) Ultra 9 285K");
        let headline = section.summary[0].text.text().unwrap();
        assert!(headline.contains("Arrow Lake-S"), "{headline}");
        assert_eq!(
            section.summary[0].live,
            Some(LiveKey::CpuPackageTemperature)
        );
        let text = crate::specs::probe_text(std::slice::from_ref(&section));
        assert!(
            text.contains("Base clock (Performance): 3700 MHz"),
            "{text}"
        );
        assert!(text.contains("Base clock (Efficient): 3200 MHz"), "{text}");
        assert!(!text.contains("C:"), "no temperature text is invented");
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only CPU specs probe; no driver, elevation, window or input"]
    fn native_specs_cpu_read_only_probe() {
        let started = std::time::Instant::now();
        let sections = [collect(&Context::probe())];
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        println!("{}", crate::specs::probe_text(&sections));
        println!("collected in {elapsed:.3} ms (private values masked)");
    }
}
