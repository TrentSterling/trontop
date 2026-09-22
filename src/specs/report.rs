//! Plain-text rendering for Copy and for read-only probes. Private values are
//! masked unless the caller explicitly reveals them.
use super::live::{BridgeReadings, LiveKey, resolve};
use super::model::{Group, Item, Section, SectionId, SummaryLine, Value};
use super::worker::Snapshot;
use crate::model::SystemSnapshot;
use std::fmt::Write;
use std::time::Instant;

pub const HIDDEN: &str = "[hidden]";

/// Where live values come from. None prints the key itself (probes).
pub type LiveSource<'a> = Option<(&'a SystemSnapshot, &'a BridgeReadings, Instant)>;

pub fn value_text(value: &Value) -> String {
    match value {
        Value::Known(text) => text.clone(),
        Value::Unavailable(reason) => format!("Unavailable ({reason})"),
    }
}

fn live_text(key: &LiveKey, live: LiveSource<'_>) -> String {
    match live {
        Some((snapshot, bridge, now)) => value_text(&resolve(key, snapshot, bridge, now).value),
        None => format!("[live {key:?}]"),
    }
}

fn summary_line(line: &SummaryLine, live: LiveSource<'_>, reveal: bool) -> String {
    let mut text = if line.private && !reveal {
        HIDDEN.to_string()
    } else {
        value_text(&line.text)
    };
    if let Some(key) = &line.live {
        let _ = write!(text, "    {}", live_text(key, live));
    }
    text
}

fn group(out: &mut String, group: &Group, depth: usize, live: LiveSource<'_>, reveal: bool) {
    let indent = "    ".repeat(depth);
    let _ = write!(out, "{indent}{}", group.title);
    if let Some(key) = &group.live {
        let _ = write!(out, "    {}", live_text(key, live));
    }
    out.push('\n');
    for item in &group.items {
        match item {
            Item::Group(child) => self::group(out, child, depth + 1, live, reveal),
            Item::Row(row) => {
                let value = if row.private && !reveal {
                    HIDDEN.to_string()
                } else if let Some(key) = &row.live {
                    live_text(key, live)
                } else {
                    row.display_text().unwrap_or_else(|| value_text(&row.value))
                };
                let _ = writeln!(out, "{indent}    {}: {value}", row.label);
            }
        }
    }
}

/// One section: summary lines, then its tree, then its issues.
pub fn section_text(out: &mut String, section: &Section, live: LiveSource<'_>, reveal: bool) {
    let _ = writeln!(out, "== {} ==", section.id.title());
    for line in &section.summary {
        let _ = writeln!(out, "  {}", summary_line(line, live, reveal));
    }
    for tree in &section.groups {
        group(out, tree, 0, live, reveal);
    }
    for issue in &section.issues {
        let _ = writeln!(out, "  Issue: {issue}");
    }
    out.push('\n');
}

/// The whole System page as text: Summary first, then every section.
pub fn text(snapshot: &Snapshot, live: LiveSource<'_>, reveal: bool) -> String {
    let mut out = format!(
        "Trontop system specs (v{})\nPrivate values: {}\n\n== {} ==\n",
        env!("CARGO_PKG_VERSION"),
        if reveal { "shown" } else { "hidden" },
        SectionId::Summary.title()
    );
    for entry in &snapshot.entries {
        if entry.id == SectionId::SensorBridge {
            continue;
        }
        let _ = writeln!(out, "{}", entry.id.title());
        match &entry.section {
            Some(section) => {
                for line in &section.summary {
                    let _ = writeln!(out, "    {}", summary_line(line, live, reveal));
                }
            }
            None => {
                let _ = writeln!(out, "    Unavailable ({})", entry.health.state.label());
            }
        }
    }
    out.push('\n');
    for entry in &snapshot.entries {
        match &entry.section {
            Some(section) => section_text(&mut out, section, live, reveal),
            None => {
                let _ = writeln!(
                    out,
                    "== {} ==\n  Unavailable ({})\n",
                    entry.id.title(),
                    entry.health.state.label()
                );
            }
        }
    }
    out
}

/// Probe output: private values masked, live keys printed as keys.
pub fn probe_text(sections: &[Section]) -> String {
    let mut out = String::new();
    for section in sections {
        section_text(&mut out, section, None, false);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::model::Row;
    use super::*;

    #[test]
    fn private_values_are_masked_unless_revealed_and_live_keys_resolve() {
        let section = Section::new(SectionId::Motherboard)
            .summary_line(SummaryLine::known("Fixture board"))
            .group(
                Group::new("Fixture board")
                    .row(Row::known("Serial", "FIXTURE-SERIAL-123").private())
                    .row(Row::unavailable("Chipset", "requires administrator"))
                    .row(Row::live("Uptime", LiveKey::Uptime)),
            )
            .issue("fixture issue");
        let masked = probe_text(std::slice::from_ref(&section));
        assert!(!masked.contains("FIXTURE-SERIAL-123"));
        assert!(masked.contains("Serial: [hidden]"));
        assert!(masked.contains("Chipset: Unavailable (requires administrator)"));
        assert!(masked.contains("Uptime: [live Uptime]"));
        assert!(masked.contains("Issue: fixture issue"));
        let mut revealed = String::new();
        let sampler = SystemSnapshot {
            sequence: 1,
            uptime_seconds: 3_661,
            ..Default::default()
        };
        section_text(
            &mut revealed,
            &section,
            Some((&sampler, &BridgeReadings::default(), Instant::now())),
            true,
        );
        assert!(revealed.contains("Serial: FIXTURE-SERIAL-123"));
        assert!(!revealed.contains("[live"));
    }
}
