//! SMBIOS through GetSystemFirmwareTable('RSMB'): the raw DMI tables Windows
//! already copied at boot. No driver, no elevation. Per-DIMM memory (type 17),
//! board (type 2), BIOS (type 0), processor sockets (type 4) and more.
//!
//! Offsets passed to `Structure` accessors are from the start of the structure,
//! including its 4-byte header, exactly as the DMTF SMBIOS tables list them.
use super::NativeError;

/// Upper bound for a raw table copy; real tables are a few KB.
const MAX_TABLE_BYTES: usize = 16 * 1024 * 1024;
/// Defensive iteration bound; real tables have at most a few hundred entries.
const MAX_STRUCTURES: usize = 8192;
/// Type 127 marks the end of the table.
pub const END_OF_TABLE: u8 = 127;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub major: u8,
    pub minor: u8,
    pub revision: u8,
    data: Vec<u8>,
}

impl Table {
    /// Parses Windows' RawSMBIOSData: 1-byte calling method, major, minor, DMI
    /// revision, 4-byte table length, then the structure table.
    pub fn parse(blob: &[u8]) -> Result<Self, NativeError> {
        if blob.len() < 8 {
            return Err(NativeError::Malformed("SMBIOS header is truncated"));
        }
        let length = u32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]) as usize;
        let data = blob
            .get(8..8usize.saturating_add(length))
            .ok_or(NativeError::Malformed(
                "SMBIOS table length exceeds the buffer",
            ))?;
        Ok(Self {
            major: blob[1],
            minor: blob[2],
            revision: blob[3],
            data: data.to_vec(),
        })
    }

    /// A table from raw structure bytes; used by unit tests with synthetic data.
    #[cfg(test)]
    pub fn from_structures(major: u8, minor: u8, data: Vec<u8>) -> Self {
        Self {
            major,
            minor,
            revision: 0,
            data,
        }
    }

    #[cfg(test)]
    pub fn at_least(&self, major: u8, minor: u8) -> bool {
        (self.major, self.minor) >= (major, minor)
    }

    /// Every structure before type 127 or the first malformed one.
    pub fn structures(&self) -> Structures<'_> {
        Structures {
            data: &self.data,
            position: 0,
            yielded: 0,
            done: false,
        }
    }

    pub fn of_type(&self, kind: u8) -> impl Iterator<Item = Structure<'_>> {
        self.structures().filter(move |s| s.kind == kind)
    }

    /// True when iteration stopped early on malformed bytes rather than at
    /// type 127 or the exact end of the data.
    pub fn truncated(&self) -> bool {
        let mut structures = self.structures();
        for _ in structures.by_ref() {}
        structures.position != usize::MAX
    }
}

/// Reads the live table. Worker threads only.
#[cfg(windows)]
pub fn read() -> Result<Table, NativeError> {
    use windows::Win32::System::SystemInformation::{
        FIRMWARE_TABLE_PROVIDER, GetSystemFirmwareTable,
    };
    const API: &str = "GetSystemFirmwareTable";
    let provider = FIRMWARE_TABLE_PROVIDER(u32::from_be_bytes(*b"RSMB"));
    // The table can in theory change between the size and copy calls.
    for _ in 0..3 {
        // SAFETY: a null buffer only asks for the required size.
        let size = unsafe { GetSystemFirmwareTable(provider, 0, None) } as usize;
        if size == 0 {
            return Err(NativeError::last_error(API));
        }
        if size > MAX_TABLE_BYTES {
            return Err(NativeError::Malformed("SMBIOS table size is implausible"));
        }
        let mut buffer = vec![0u8; size];
        // SAFETY: the slice length is passed as the buffer size.
        let written = unsafe { GetSystemFirmwareTable(provider, 0, Some(&mut buffer)) } as usize;
        if written == 0 {
            return Err(NativeError::last_error(API));
        }
        if written <= size {
            buffer.truncate(written);
            return Table::parse(&buffer);
        }
    }
    Err(NativeError::Malformed("SMBIOS table size kept changing"))
}

pub struct Structures<'a> {
    data: &'a [u8],
    position: usize,
    yielded: usize,
    done: bool,
}

impl<'a> Iterator for Structures<'a> {
    type Item = Structure<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let start = self.position;
        if start == self.data.len() {
            self.finish(true);
            return None;
        }
        let header = self.data.get(start..start + 4);
        let formatted_len = header.map_or(0, |h| h[1] as usize);
        let strings_start = start + formatted_len;
        if header.is_none() || formatted_len < 4 || strings_start > self.data.len() {
            self.finish(false);
            return None;
        }
        // The string-set ends with two NULs (just "\0\0" when there are none).
        let mut end = strings_start;
        loop {
            match self.data.get(end..end + 2) {
                Some([0, 0]) => break,
                Some(_) => end += 1,
                None => {
                    self.finish(false);
                    return None;
                }
            }
        }
        let kind = self.data[start];
        if kind == END_OF_TABLE {
            self.finish(true);
            return None;
        }
        self.yielded += 1;
        if self.yielded > MAX_STRUCTURES {
            self.finish(false);
            return None;
        }
        self.position = end + 2;
        Some(Structure {
            kind,
            handle: u16::from_le_bytes([self.data[start + 2], self.data[start + 3]]),
            formatted: &self.data[start..strings_start],
            strings: &self.data[strings_start..end],
        })
    }
}

impl Structures<'_> {
    /// `position == usize::MAX` records a clean end for `Table::truncated`.
    fn finish(&mut self, clean: bool) {
        self.done = true;
        if clean {
            self.position = usize::MAX;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Structure<'a> {
    pub kind: u8,
    pub handle: u16,
    /// The formatted area, including the 4-byte header.
    formatted: &'a [u8],
    /// The string-set without its final double NUL.
    strings: &'a [u8],
}

impl<'a> Structure<'a> {
    /// Fields beyond the formatted-area length from the header are absent
    /// (older SMBIOS versions), and every accessor returns None for them.
    pub fn bytes(&self, offset: usize, len: usize) -> Option<&'a [u8]> {
        self.formatted.get(offset..offset.checked_add(len)?)
    }

    pub fn byte(&self, offset: usize) -> Option<u8> {
        self.formatted.get(offset).copied()
    }

    pub fn word(&self, offset: usize) -> Option<u16> {
        self.bytes(offset, 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn dword(&self, offset: usize) -> Option<u32> {
        self.bytes(offset, 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn qword(&self, offset: usize) -> Option<u64> {
        let b = self.bytes(offset, 8)?;
        Some(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// The string referenced by the string-number byte at `offset`.
    /// None for number 0, a missing entry, or blank text.
    pub fn string(&self, offset: usize) -> Option<String> {
        self.string_number(self.byte(offset)?)
    }

    /// 1-based string-set lookup, trimmed; lossy for non-UTF-8 bytes.
    pub fn string_number(&self, number: u8) -> Option<String> {
        if number == 0 || self.strings.is_empty() {
            return None;
        }
        let raw = self.strings.split(|b| *b == 0).nth(number as usize - 1)?;
        let text = String::from_utf8_lossy(raw);
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_string())
    }

    /// A 16-byte SMBIOS UUID field in canonical text form. SMBIOS 2.6+ stores
    /// the first three fields little-endian. None when all 0x00 or all 0xFF
    /// ("not present" / "not set"). Always a private value.
    pub fn uuid(&self, offset: usize) -> Option<String> {
        let b = self.bytes(offset, 16)?;
        if b.iter().all(|v| *v == 0) || b.iter().all(|v| *v == 0xff) {
            return None;
        }
        Some(format!(
            "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            b[3],
            b[2],
            b[1],
            b[0],
            b[5],
            b[4],
            b[7],
            b[6],
            b[8],
            b[9],
            b[10],
            b[11],
            b[12],
            b[13],
            b[14],
            b[15]
        ))
    }
}

/// OEM filler that must be shown as "not set", never as a real value.
pub fn is_placeholder(text: &str) -> bool {
    const FILLERS: [&str; 16] = [
        "to be filled by o.e.m.",
        "to be filled by oem",
        "default string",
        "not specified",
        "not applicable",
        "not available",
        "system product name",
        "system manufacturer",
        "system version",
        "system serial number",
        "base board serial number",
        "chassis serial number",
        "none",
        "n/a",
        "0123456789",
        "123456789",
    ];
    let text = text.trim();
    text.is_empty()
        || text.bytes().all(|b| b == b'0' || b == b' ')
        || FILLERS.iter().any(|f| text.eq_ignore_ascii_case(f))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Synthetic structure bytes: header, formatted body, string-set.
    pub(crate) fn structure(kind: u8, handle: u16, body: &[u8], strings: &[&str]) -> Vec<u8> {
        let mut bytes = vec![kind, (4 + body.len()) as u8];
        bytes.extend_from_slice(&handle.to_le_bytes());
        bytes.extend_from_slice(body);
        if strings.is_empty() {
            bytes.extend_from_slice(&[0, 0]);
        } else {
            for text in strings {
                bytes.extend_from_slice(text.as_bytes());
                bytes.push(0);
            }
            bytes.push(0);
        }
        bytes
    }

    fn fixture() -> Vec<u8> {
        let mut data = structure(0, 0, &[1, 2, 0, 0], &["Fixture BIOS", "1.00"]);
        // Type 17: size word 16384 MB at offset 0x0C, locator string at 0x10.
        let mut dimm = vec![0u8; 0x18];
        dimm[0x08..0x0A].copy_from_slice(&0x4000u16.to_le_bytes());
        dimm[0x0C] = 1;
        data.extend(structure(17, 0x11, &dimm, &["DIMM_A1"]));
        data.extend(structure(2, 2, &[1, 0, 0, 0], &[]));
        data.extend(structure(END_OF_TABLE, 0xffff, &[], &[]));
        data
    }

    #[test]
    fn raw_blob_header_and_structures_parse() {
        let data = fixture();
        let mut blob = vec![0, 3, 7, 0];
        blob.extend_from_slice(&(data.len() as u32).to_le_bytes());
        blob.extend_from_slice(&data);
        blob.extend_from_slice(&[0xAA; 3]); // trailing bytes beyond the length
        let table = Table::parse(&blob).unwrap();
        assert!(table.at_least(3, 7) && !table.at_least(3, 8));
        let kinds = table.structures().map(|s| s.kind).collect::<Vec<_>>();
        assert_eq!(kinds, [0, 17, 2]);
        assert!(!table.truncated());
        let bios = table.of_type(0).next().unwrap();
        assert_eq!(bios.string(4).as_deref(), Some("Fixture BIOS"));
        assert_eq!(bios.string(5).as_deref(), Some("1.00"));
        assert_eq!(bios.string(6), None); // string number 0
        assert_eq!(bios.string(8), None); // beyond the formatted area
        let dimm = table.of_type(17).next().unwrap();
        assert_eq!(dimm.handle, 0x11);
        assert_eq!(dimm.word(0x0C), Some(0x4000));
        assert_eq!(dimm.string(0x10).as_deref(), Some("DIMM_A1"));
        assert_eq!(dimm.qword(0x14), Some(0));
        assert_eq!(dimm.qword(0x18), None);
        let board = table.of_type(2).next().unwrap();
        assert_eq!(board.string(4), None);
        assert_eq!(board.string_number(1), None);
    }

    #[test]
    fn malformed_tables_stop_without_panicking() {
        assert!(Table::parse(&[0, 3, 0]).is_err());
        assert!(Table::parse(&[0, 3, 0, 0, 0xff, 0, 0, 0]).is_err());
        let good = fixture();
        // Every truncation and every single-byte corruption must be safe.
        for cut in 0..good.len() {
            let table = Table::from_structures(3, 0, good[..cut].to_vec());
            let count = table.structures().count();
            assert!(count <= 3);
            for s in table.structures() {
                for offset in 0..40 {
                    let _ = (s.string(offset), s.qword(offset), s.uuid(offset));
                }
            }
        }
        for index in 0..good.len() {
            for value in [0u8, 1, 3, 4, 0x7f, 0xff] {
                let mut bad = good.clone();
                bad[index] = value;
                let table = Table::from_structures(3, 0, bad);
                for s in table.structures() {
                    let _ = (s.string(4), s.string(0x10), s.dword(0x08));
                }
                let _ = table.truncated();
            }
        }
        // A header length below 4 ends iteration as truncated.
        let table = Table::from_structures(3, 0, vec![1, 2, 0, 0, 0, 0]);
        assert_eq!(table.structures().count(), 0);
        assert!(table.truncated());
        // A missing double-NUL terminator also ends iteration.
        let table = Table::from_structures(3, 0, vec![1, 4, 0, 0, b'A', 0]);
        assert_eq!(table.structures().count(), 0);
        assert!(table.truncated());
    }

    #[test]
    fn uuid_byte_order_and_placeholders() {
        let body = (0u8..16).collect::<Vec<_>>();
        let data = structure(1, 1, &[&[0u8; 4][..], &body].concat(), &[]);
        let table = Table::from_structures(3, 0, data);
        let system = table.structures().next().unwrap();
        assert_eq!(
            system.uuid(8).as_deref(),
            Some("03020100-0504-0706-0809-0A0B0C0D0E0F")
        );
        let blank = structure(1, 1, &[0u8; 20], &[]);
        let table = Table::from_structures(3, 0, blank);
        assert_eq!(table.structures().next().unwrap().uuid(8), None);
        for filler in [
            "To Be Filled By O.E.M.",
            " Default string ",
            "0000 0000",
            "",
        ] {
            assert!(is_placeholder(filler), "{filler}");
        }
        assert!(!is_placeholder("Z890-C"));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only SMBIOS table copy; no driver, elevation, window or input"]
    fn native_specs_smbios_read_only_probe() {
        let started = std::time::Instant::now();
        let table = read().expect("SMBIOS table");
        let elapsed = started.elapsed();
        let mut counts = std::collections::BTreeMap::<u8, usize>::new();
        for s in table.structures() {
            *counts.entry(s.kind).or_default() += 1;
        }
        println!(
            "SMBIOS {}.{} truncated={} types={counts:?} in {:.3} ms",
            table.major,
            table.minor,
            table.truncated(),
            elapsed.as_secs_f64() * 1000.0
        );
        for dimm in table.of_type(17) {
            println!(
                "type17 handle={:#06x} size_word={:?} locator={:?} bank={:?} speed={:?}",
                dimm.handle,
                dimm.word(0x0C),
                dimm.string(0x10),
                dimm.string(0x11),
                dimm.word(0x15)
            );
        }
    }
}
