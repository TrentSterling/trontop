//! Extract only embedded executable resources; no shell association handlers or launch.
use super::{PIXEL_BYTES, Pixels, SIDE, local_executable};
use std::path::Path;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GdiFlush, HBITMAP, HDC, HGDIOBJ, SelectObject,
};
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
    FILE_ATTRIBUTE_RECALL_ON_OPEN, FILE_ATTRIBUTE_REPARSE_POINT, GetDriveTypeW, GetFileAttributesW,
    INVALID_FILE_ATTRIBUTES,
};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DestroyIcon, DrawIconEx, HICON};
use windows::core::PCWSTR;

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

/// Best-effort local-only gate, not a security boundary against concurrent path changes.
fn eligible(path: &Path) -> Option<Vec<u16>> {
    eligible_with(
        path,
        |root| unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) },
        |path| unsafe { GetFileAttributesW(PCWSTR(path.as_ptr())) },
    )
}

fn eligible_with(
    path: &Path,
    mut drive_type: impl FnMut(&[u16]) -> u32,
    mut attributes: impl FnMut(&[u16]) -> u32,
) -> Option<Vec<u16>> {
    let path = local_executable(path)?;
    let text = path.to_str()?;
    let root = wide(&text[..3]);
    // DRIVE_FIXED: skip mapped network drives, removable media and unknown drives.
    if drive_type(&root) != 3 {
        return None;
    }
    let forbidden = FILE_ATTRIBUTE_REPARSE_POINT.0
        | FILE_ATTRIBUTE_OFFLINE.0
        | FILE_ATTRIBUTE_RECALL_ON_OPEN.0
        | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS.0;
    // Check each ancestor before accessing the next component. No canonicalization
    // that could itself walk through a junction into a remote or cloud-backed path.
    for end in text
        .char_indices()
        .filter_map(|(i, c)| (c == '\\' && i > 2).then_some(i))
        .chain(Some(text.len()))
    {
        let prefix = wide(&text[..end]);
        let attributes = attributes(&prefix);
        if attributes == INVALID_FILE_ATTRIBUTES || attributes & forbidden != 0 {
            return None;
        }
        if end == text.len() && attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0 {
            return None;
        }
    }
    Some(wide(text))
}

struct OwnedIcon(HICON);
impl Drop for OwnedIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.0);
        }
    }
}

struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u8,
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            if !self.previous.0.is_null() {
                SelectObject(self.dc, self.previous);
            }
            if !self.bitmap.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            }
            let _ = DeleteDC(self.dc);
        }
    }
}
impl Surface {
    fn new() -> Option<Self> {
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.0.is_null() {
            return None;
        }
        let mut surface = Self {
            dc,
            bitmap: HBITMAP::default(),
            previous: HGDIOBJ::default(),
            bits: std::ptr::null_mut(),
        };
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIDE as i32,
                biHeight: -(SIDE as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        surface.bitmap =
            unsafe { CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) }
                .ok()?;
        if bits.is_null() {
            return None;
        }
        surface.bits = bits.cast();
        let previous = unsafe { SelectObject(dc, HGDIOBJ(surface.bitmap.0)) };
        if previous.0.is_null() || previous.0 as isize == -1 {
            return None;
        }
        surface.previous = previous;
        Some(surface)
    }

    fn draw(&mut self, icon: HICON, background: u8) -> Option<Vec<u8>> {
        // Positive width, top-down 32-bit DIB has exactly SIDE*SIDE*4 bytes.
        {
            let pixels = unsafe { std::slice::from_raw_parts_mut(self.bits, PIXEL_BYTES) };
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[background, background, background, 255]);
            }
        }
        unsafe {
            DrawIconEx(
                self.dc,
                0,
                0,
                icon,
                SIDE as i32,
                SIDE as i32,
                0,
                None,
                DI_NORMAL,
            )
            .ok()?;
            // Synchronize GDI before CPU access to the section memory.
            if !GdiFlush().as_bool() {
                return None;
            }
        }
        Some(unsafe { std::slice::from_raw_parts(self.bits, PIXEL_BYTES) }.to_vec())
    }
}

pub(super) fn extract(path: &Path) -> Option<Pixels> {
    let path = eligible(path)?;
    let mut icon = HICON::default();
    let count = unsafe { ExtractIconExW(PCWSTR(path.as_ptr()), 0, Some(&mut icon), None, 1) };
    // Own any returned handle even on an unusual partial-error result.
    let owned = (!icon.0.is_null()).then(|| OwnedIcon(icon));
    if count != 1 {
        return None;
    }
    let icon = owned?;
    let mut surface = Surface::new()?;
    let black = surface.draw(icon.0, 0)?;
    let white = surface.draw(icon.0, 255)?;
    let rgba = rgba_from_backgrounds(&black, &white)?;
    rgba.chunks_exact(4)
        .any(|p| p[3] != 0)
        .then_some(Pixels(rgba))
}

/// DrawIconEx handles alpha icons and legacy AND masks. Two opaque backgrounds
/// recover coverage without trusting the undefined alpha bytes left by legacy GDI.
/// Legacy XOR inversion is approximated as opaque artwork: a texture cannot invert
/// arbitrary UI backgrounds the way a monochrome Windows icon can.
fn rgba_from_backgrounds(black: &[u8], white: &[u8]) -> Option<Vec<u8>> {
    if black.len() != PIXEL_BYTES || white.len() != PIXEL_BYTES {
        return None;
    }
    let mut rgba = Vec::with_capacity(PIXEL_BYTES);
    for (b, w) in black.chunks_exact(4).zip(white.chunks_exact(4)) {
        let transmission = (0..3)
            .map(|i| u16::from(w[i].saturating_sub(b[i])))
            .sum::<u16>()
            / 3;
        let alpha = 255 - transmission;
        let straight = |channel: u8| {
            if alpha == 0 {
                0
            } else {
                ((u16::from(channel) * 255 + alpha / 2) / alpha).min(255) as u8
            }
        };
        rgba.extend_from_slice(&[straight(b[2]), straight(b[1]), straight(b[0]), alpha as u8]);
    }
    Some(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_path_gate_stops_before_remote_or_reparse_descendants() {
        let path = Path::new(r"C:\Fixture\app.exe");
        for drive in [0, 1, 2, 4, 5, 6] {
            assert!(
                eligible_with(path, |_| drive, |_| panic!("non-fixed drive accessed")).is_none()
            );
        }
        for rejected in [
            INVALID_FILE_ATTRIBUTES,
            FILE_ATTRIBUTE_REPARSE_POINT.0,
            FILE_ATTRIBUTE_OFFLINE.0,
            FILE_ATTRIBUTE_RECALL_ON_OPEN.0,
            FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS.0,
        ] {
            let mut calls = 0;
            assert!(
                eligible_with(
                    path,
                    |_| 3,
                    |part| {
                        calls += 1;
                        assert_eq!(part, wide(r"C:\Fixture"));
                        rejected
                    }
                )
                .is_none()
            );
            assert_eq!(calls, 1);
        }
        let mut parts = Vec::new();
        let accepted = eligible_with(
            path,
            |_| 3,
            |part| {
                parts.push(String::from_utf16(&part[..part.len() - 1]).unwrap());
                if parts.len() == 1 {
                    FILE_ATTRIBUTE_DIRECTORY.0
                } else {
                    0
                }
            },
        );
        assert_eq!(accepted, Some(wide(r"C:\Fixture\app.exe")));
        assert_eq!(parts, [r"C:\Fixture", r"C:\Fixture\app.exe"]);
        assert!(eligible_with(path, |_| 3, |_| FILE_ATTRIBUTE_DIRECTORY.0).is_none());
    }

    #[test]
    fn transparent_black_and_antialiased_pixels_keep_their_coverage() {
        let mut black = vec![0; PIXEL_BYTES];
        let mut white = vec![255; PIXEL_BYTES];
        // Opaque black, translucent red, transparent pixel, opaque colored pixel.
        black[..16].copy_from_slice(&[0, 0, 0, 0, 0, 0, 128, 0, 0, 0, 0, 0, 20, 40, 60, 0]);
        white[..16].copy_from_slice(&[
            0, 0, 0, 0, 127, 127, 255, 0, 255, 255, 255, 0, 20, 40, 60, 0,
        ]);
        let rgba = rgba_from_backgrounds(&black, &white).unwrap();
        assert_eq!(
            &rgba[..16],
            &[0, 0, 0, 255, 255, 0, 0, 128, 0, 0, 0, 0, 60, 40, 20, 255]
        );
        assert!(rgba_from_backgrounds(&[], &white).is_none());
    }

    #[test]
    #[ignore = "Read-only embedded icon extraction from this test executable; no window, shell launch or input"]
    fn native_executable_icon_read_only_probe() {
        use windows::Win32::System::Threading::{
            GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess, GetGuiResources,
        };
        let path = std::env::current_exe().unwrap();
        let started = std::time::Instant::now();
        let first = extract(&path).expect("test executable embedded icon unavailable");
        let elapsed = started.elapsed();
        assert_eq!(first.0.len(), PIXEL_BYTES);
        let resources = || unsafe {
            (
                GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS),
                GetGuiResources(GetCurrentProcess(), GR_USEROBJECTS),
            )
        };
        let before = resources();
        for _ in 0..40 {
            assert!(extract(&path).is_some());
        }
        let after = resources();
        assert!(
            after.0 <= before.0 + 2 && after.1 <= before.1 + 2,
            "resource growth: {before:?} -> {after:?}"
        );
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/ui-smoke");
        std::fs::create_dir_all(&directory).unwrap();
        image::save_buffer(
            directory.join("native-executable-icon.png"),
            &first.0,
            SIDE as u32,
            SIDE as u32,
            image::ColorType::Rgba8,
        )
        .unwrap();
        println!(
            "Native embedded icon: 32x32, first lookup {elapsed:?}; 40 repeats, GDI/USER {before:?} -> {after:?}; no windows/input"
        );
    }
}
