//! Network interface identity. Windows lists NDIS filter modules (QoS Packet
//! Scheduler, WFP and Native WiFi filters) as separate interfaces stacked on the
//! same adapter, with the same traffic counters. sysinfo keys interfaces by alias,
//! so one Wi-Fi adapter could show up twice ("Wi-Fi" and
//! "Wi-Fi-Native WiFi Filter Driver-0000"). Only the adapter itself is kept.
use std::collections::HashSet;

/// Aliases of interfaces that Windows flags as NDIS filter modules.
#[derive(Default)]
pub struct FilterAliases {
    aliases: HashSet<String>,
    known: HashSet<String>,
    pub error: Option<String>,
}

impl FilterAliases {
    /// Re-read interface flags only when the interface name set changes. Worker
    /// thread only; the native call never runs on the UI thread.
    pub fn refresh<'a>(&mut self, names: impl IntoIterator<Item = &'a String>) {
        let names: HashSet<String> = names.into_iter().cloned().collect();
        if names == self.known && self.error.is_none() {
            return;
        }
        match native_filter_aliases() {
            Ok(aliases) => {
                self.aliases = aliases;
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
        self.known = names;
    }

    pub fn is_filter(&self, alias: &str) -> bool {
        self.aliases.contains(alias)
    }
}

#[cfg(windows)]
fn native_filter_aliases() -> Result<HashSet<String>, String> {
    use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
    // MIB_IF_ROW2 InterfaceAndOperStatusFlags: bit 1 is FilterInterface.
    const FILTER_INTERFACE: u8 = 0x02;
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    unsafe {
        GetIfTable2(&mut table)
            .ok()
            .map_err(|error| format!("GetIfTable2 failed: {error}"))?;
        if table.is_null() {
            return Err("GetIfTable2 returned no table".into());
        }
        let rows =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let aliases = rows
            .iter()
            .filter(|row| row.InterfaceAndOperStatusFlags._bitfield & FILTER_INTERFACE != 0)
            .map(|row| {
                let end = row
                    .Alias
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(row.Alias.len());
                String::from_utf16_lossy(&row.Alias[..end])
            })
            .collect();
        FreeMibTable(table.cast());
        Ok(aliases)
    }
}

#[cfg(not(windows))]
fn native_filter_aliases() -> Result<HashSet<String>, String> {
    Ok(HashSet::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndis_filter_modules_are_not_separate_adapters() {
        let filters = FilterAliases {
            aliases: [
                "Wi-Fi-Native WiFi Filter Driver-0000".to_string(),
                "Wi-Fi-QoS Packet Scheduler-0000".to_string(),
            ]
            .into(),
            ..Default::default()
        };
        let names = ["Wi-Fi", "Wi-Fi-Native WiFi Filter Driver-0000", "Ethernet"];
        let kept: Vec<_> = names.iter().filter(|n| !filters.is_filter(n)).collect();
        assert_eq!(kept, [&"Wi-Fi", &"Ethernet"]);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only GetIfTable2 probe; no traffic, window or input"]
    fn native_filter_interfaces_hide_sysinfo_duplicates() {
        let networks = sysinfo::Networks::new_with_refreshed_list();
        let mut filters = FilterAliases::default();
        filters.refresh(networks.keys());
        assert!(filters.error.is_none(), "{:?}", filters.error);
        let kept: Vec<_> = networks.keys().filter(|n| !filters.is_filter(n)).collect();
        eprintln!(
            "sysinfo: {:?} kept: {kept:?}",
            networks.keys().collect::<Vec<_>>()
        );
        // Two kept interfaces must never carry identical lifetime counters.
        let totals: HashSet<_> = kept
            .iter()
            .map(|n| {
                let d = &networks[*n];
                (d.total_received(), d.total_transmitted())
            })
            .filter(|t| *t != (0, 0))
            .collect();
        assert_eq!(
            totals.len(),
            kept.iter()
                .filter(|n| {
                    let d = &networks[**n];
                    (d.total_received(), d.total_transmitted()) != (0, 0)
                })
                .count()
        );
    }
}
