//! Display model shared by every specs provider. Plain data: no native calls,
//! no egui types, cheap to clone into immutable Arc snapshots.
use super::live::LiveKey;
use std::time::{Duration, Instant, SystemTime};

/// The reason every scaffold provider returns until its lane replaces it.
pub const NOT_IMPLEMENTED: &str = "not implemented";
/// Shown for a Known value that turned out to be blank text.
pub const NOT_REPORTED: &str = "not reported by the device or Windows";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SectionId {
    /// Derived from every other section's summary lines; never collected.
    Summary,
    OperatingSystem,
    Cpu,
    Memory,
    Motherboard,
    Graphics,
    Storage,
    OpticalDrives,
    Audio,
    Peripherals,
    Network,
    /// External read-only sensor sources that feed live values.
    SensorBridge,
}

impl SectionId {
    /// Sidebar order.
    pub const ALL: [Self; 12] = [
        Self::Summary,
        Self::OperatingSystem,
        Self::Cpu,
        Self::Memory,
        Self::Motherboard,
        Self::Graphics,
        Self::Storage,
        Self::OpticalDrives,
        Self::Audio,
        Self::Peripherals,
        Self::Network,
        Self::SensorBridge,
    ];

    /// Sections with their own provider worker, in sidebar order.
    pub const COLLECTED: [Self; 11] = [
        Self::OperatingSystem,
        Self::Cpu,
        Self::Memory,
        Self::Motherboard,
        Self::Graphics,
        Self::Storage,
        Self::OpticalDrives,
        Self::Audio,
        Self::Peripherals,
        Self::Network,
        Self::SensorBridge,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::OperatingSystem => "Operating System",
            Self::Cpu => "CPU",
            Self::Memory => "RAM",
            Self::Motherboard => "Motherboard",
            Self::Graphics => "Graphics",
            Self::Storage => "Storage",
            Self::OpticalDrives => "Optical Drives",
            Self::Audio => "Audio",
            Self::Peripherals => "Peripherals",
            Self::Network => "Network",
            Self::SensorBridge => "Sensor Sources",
        }
    }

    /// Stable ASCII identifier for text reports, thread names and tests.
    pub fn key(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::OperatingSystem => "os",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Motherboard => "board",
            Self::Graphics => "graphics",
            Self::Storage => "storage",
            Self::OpticalDrives => "optical",
            Self::Audio => "audio",
            Self::Peripherals => "peripherals",
            Self::Network => "network",
            Self::SensorBridge => "bridge",
        }
    }
}

/// A displayed value is either real text or an explicit reason it is absent.
/// There is no estimated, default or placeholder variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Known(String),
    Unavailable(String),
}

impl Value {
    /// Blank text is not a reading: it becomes Unavailable(NOT_REPORTED).
    pub fn known(text: impl Into<String>) -> Self {
        let text = text.into();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            Self::Unavailable(NOT_REPORTED.into())
        } else if trimmed.len() == text.len() {
            Self::Known(text)
        } else {
            Self::Known(trimmed.to_string())
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable(reason.into())
    }

    pub fn not_implemented() -> Self {
        Self::Unavailable(NOT_IMPLEMENTED.into())
    }

    pub fn from_option<T: Into<String>>(value: Option<T>, reason: impl Into<String>) -> Self {
        value.map_or_else(|| Self::unavailable(reason), Self::known)
    }

    /// The error's Display text becomes the reason; keep it free of private data.
    pub fn from_result<T: Into<String>, E: std::fmt::Display>(value: Result<T, E>) -> Self {
        match value {
            Ok(text) => Self::known(text),
            Err(error) => Self::unavailable(error.to_string()),
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Known(text) => Some(text),
            Self::Unavailable(_) => None,
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Known(_) => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }

    pub fn is_known(&self) -> bool {
        matches!(self, Self::Known(_))
    }
}

/// One label/value line. With `live`, the UI shows the resolved live reading
/// instead of `value`; `Row::live` fills `value` with a placeholder reason.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: Value,
    /// Appended after a Known value with one space, e.g. "MB". Ignored for live rows.
    pub unit: Option<String>,
    pub live: Option<LiveKey>,
    /// Serial numbers, MAC/IP addresses, product keys, user names, SSIDs, GUIDs...
    pub private: bool,
    /// Optional non-private hover detail such as the source or a caveat.
    pub note: Option<String>,
}

impl Row {
    pub fn new(label: impl Into<String>, value: Value) -> Self {
        Self {
            label: label.into(),
            value,
            unit: None,
            live: None,
            private: false,
            note: None,
        }
    }

    pub fn known(label: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(label, Value::known(text))
    }

    pub fn unavailable(label: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(label, Value::unavailable(reason))
    }

    /// A row whose value is only ever the resolved live reading.
    pub fn live(label: impl Into<String>, key: LiveKey) -> Self {
        let mut row = Self::new(label, Value::unavailable("live value"));
        row.live = Some(key);
        row
    }

    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    pub fn private(mut self) -> Self {
        self.private = true;
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Static text with its unit, or None when Unavailable.
    pub fn display_text(&self) -> Option<String> {
        let text = self.value.text()?;
        Some(match &self.unit {
            Some(unit) => format!("{text} {unit}"),
            None => text.to_string(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Row(Row),
    Group(Group),
}

/// A collapsible tree node. Titles are shown unmasked: never put private data
/// in a title; use a private Row instead.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub title: String,
    /// Optional live reading shown beside the title, e.g. a drive temperature.
    pub live: Option<LiveKey>,
    /// Initial open state; the UI remembers later user toggles.
    pub expanded: bool,
    pub items: Vec<Item>,
}

impl Group {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            live: None,
            expanded: true,
            items: Vec::new(),
        }
    }

    pub fn collapsed(mut self) -> Self {
        self.expanded = false;
        self
    }

    pub fn live(mut self, key: LiveKey) -> Self {
        self.live = Some(key);
        self
    }

    pub fn row(mut self, row: Row) -> Self {
        self.items.push(Item::Row(row));
        self
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = Row>) -> Self {
        self.items.extend(rows.into_iter().map(Item::Row));
        self
    }

    /// Shorthand for `.row(Row::new(label, value))`.
    pub fn kv(self, label: impl Into<String>, value: Value) -> Self {
        self.row(Row::new(label, value))
    }

    pub fn group(mut self, group: Group) -> Self {
        self.items.push(Item::Group(group));
        self
    }

    pub fn push_row(&mut self, row: Row) {
        self.items.push(Item::Row(row));
    }

    pub fn push_group(&mut self, group: Group) {
        self.items.push(Item::Group(group));
    }

    /// Rows in this group and every nested group.
    pub fn row_count(&self) -> usize {
        self.items
            .iter()
            .map(|item| match item {
                Item::Row(_) => 1,
                Item::Group(group) => group.row_count(),
            })
            .sum()
    }
}

/// A Speccy-style headline on the Summary page, e.g. the CPU model with its
/// package temperature beside it.
#[derive(Clone, Debug, PartialEq)]
pub struct SummaryLine {
    pub text: Value,
    pub live: Option<LiveKey>,
    pub private: bool,
}

impl SummaryLine {
    pub fn new(text: Value) -> Self {
        Self {
            text,
            live: None,
            private: false,
        }
    }

    pub fn known(text: impl Into<String>) -> Self {
        Self::new(Value::known(text))
    }

    pub fn live(mut self, key: LiveKey) -> Self {
        self.live = Some(key);
        self
    }

    pub fn private(mut self) -> Self {
        self.private = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completeness {
    Complete,
    /// Some rows or sub-queries failed; `issues` says which.
    Partial,
    /// No groups at all.
    Unavailable,
}

/// One provider's result. Timing and freshness live in `SectionHealth`, which
/// the worker records; providers never measure themselves.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub id: SectionId,
    pub summary: Vec<SummaryLine>,
    pub groups: Vec<Group>,
    /// Non-private partial-failure reasons, e.g. "SMBIOS type 17: table absent".
    pub issues: Vec<String>,
}

impl Section {
    pub fn new(id: SectionId) -> Self {
        Self {
            id,
            summary: Vec::new(),
            groups: Vec::new(),
            issues: Vec::new(),
        }
    }

    /// A whole section that could not be read, with the reason shown everywhere.
    pub fn unavailable(id: SectionId, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            id,
            summary: vec![SummaryLine::new(Value::unavailable(reason.clone()))],
            groups: Vec::new(),
            issues: vec![reason],
        }
    }

    pub fn not_implemented(id: SectionId) -> Self {
        Self::unavailable(id, NOT_IMPLEMENTED)
    }

    pub fn summary_line(mut self, line: SummaryLine) -> Self {
        self.summary.push(line);
        self
    }

    pub fn group(mut self, group: Group) -> Self {
        self.groups.push(group);
        self
    }

    pub fn issue(mut self, reason: impl Into<String>) -> Self {
        self.issues.push(reason.into());
        self
    }

    pub fn push_summary(&mut self, line: SummaryLine) {
        self.summary.push(line);
    }

    pub fn push_group(&mut self, group: Group) {
        self.groups.push(group);
    }

    pub fn push_issue(&mut self, reason: impl Into<String>) {
        self.issues.push(reason.into());
    }

    pub fn completeness(&self) -> Completeness {
        if self.groups.is_empty() {
            Completeness::Unavailable
        } else if self.issues.is_empty() {
            Completeness::Complete
        } else {
            Completeness::Partial
        }
    }

    pub fn row_count(&self) -> usize {
        self.groups.iter().map(Group::row_count).sum()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SectionState {
    /// The worker has not started its first read.
    #[default]
    Waiting,
    /// First read in flight; no data yet.
    Collecting,
    Complete,
    Partial,
    Unavailable,
    /// A read exceeded the reporting deadline; previous data (if any) is kept.
    Slow,
    /// The worker thread is gone; previous data (if any) is kept.
    Stopped,
}

impl SectionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting",
            Self::Collecting => "Collecting",
            Self::Complete => "Complete",
            Self::Partial => "Partial",
            Self::Unavailable => "Unavailable",
            Self::Slow => "Slow",
            Self::Stopped => "Stopped",
        }
    }
}

/// Per-section freshness, recorded by the worker around each provider call.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SectionHealth {
    pub state: SectionState,
    /// Start of the read that produced the published section.
    pub collected_at: Option<Instant>,
    /// Wall clock of the same moment, for text reports.
    pub collected_wall: Option<SystemTime>,
    /// Provider call duration for the published section.
    pub duration: Option<Duration>,
    /// Set while a read is in flight.
    pub collecting_since: Option<Instant>,
    /// Worker-level problems (timeouts, stopped worker). Provider issues stay
    /// on `Section::issues`.
    pub issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_known_text_is_never_a_reading() {
        assert_eq!(Value::known("  "), Value::unavailable(NOT_REPORTED));
        assert_eq!(Value::known(" 16 GB "), Value::Known("16 GB".into()));
        assert_eq!(
            Value::from_option(None::<String>, "requires administrator"),
            Value::unavailable("requires administrator")
        );
        assert_eq!(
            Value::from_result::<String, _>(Err("access denied")),
            Value::unavailable("access denied")
        );
    }

    #[test]
    fn builders_nest_rows_and_groups_in_order() {
        let section = Section::new(SectionId::Cpu)
            .summary_line(SummaryLine::known("Fixture CPU").live(LiveKey::CpuPackageTemperature))
            .group(
                Group::new("Fixture CPU")
                    .kv("Cores", Value::known("24"))
                    .row(Row::known("Cache", "36").unit("MB"))
                    .group(Group::new("Core 0").collapsed().row(Row::live(
                        "Clock",
                        LiveKey::CpuCoreClock {
                            group: 0,
                            number: 0,
                        },
                    ))),
            );
        assert_eq!(section.row_count(), 3);
        assert_eq!(section.completeness(), Completeness::Complete);
        let Item::Row(cache) = &section.groups[0].items[1] else {
            panic!("expected a row");
        };
        assert_eq!(cache.display_text().as_deref(), Some("36 MB"));
        let Item::Group(core) = &section.groups[0].items[2] else {
            panic!("expected a group");
        };
        assert!(!core.expanded);
        let partial = section.issue("SMBIOS unavailable");
        assert_eq!(partial.completeness(), Completeness::Partial);
        let missing = Section::not_implemented(SectionId::Audio);
        assert_eq!(missing.completeness(), Completeness::Unavailable);
        assert_eq!(missing.summary[0].text.reason(), Some(NOT_IMPLEMENTED));
    }

    #[test]
    fn section_ids_are_unique_and_collected_excludes_summary() {
        let keys = SectionId::ALL
            .iter()
            .map(|id| id.key())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(keys.len(), SectionId::ALL.len());
        assert!(!SectionId::COLLECTED.contains(&SectionId::Summary));
        assert_eq!(SectionId::COLLECTED.len() + 1, SectionId::ALL.len());
    }
}
