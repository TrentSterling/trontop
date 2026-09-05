use std::env;
use std::fs;
use std::io;
use std::path::Path;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }

    let output = env::var_os("OUT_DIR").expect("Cargo did not provide OUT_DIR");
    let icon_path = Path::new(&output).join("trontop.ico");
    fs::write(&icon_path, build_icon())?;

    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let numeric_version = numeric_version(&version);
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon(&icon_path.to_string_lossy())
        .set("CompanyName", "Tront")
        .set("ProductName", "Trontop")
        .set("FileDescription", "Trontop System Control Deck")
        .set("InternalName", "trontop")
        .set("OriginalFilename", "trontop.exe")
        .set("LegalCopyright", "Copyright Trent Sterling")
        .set("Comments", "Native Windows process and performance manager")
        .set("FileVersion", &version)
        .set("ProductVersion", &version)
        .set_version_info(winresource::VersionInfo::FILEVERSION, numeric_version)
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, numeric_version);
    if version.contains('-') {
        resource.set_version_info(
            winresource::VersionInfo::FILEFLAGS,
            winresource::VersionInfo::VS_FF_PRERELEASE,
        );
    }
    resource.compile()
}

fn numeric_version(version: &str) -> u64 {
    let mut words = version
        .split(['.', '-'])
        .filter_map(|word| word.parse::<u16>().ok());
    let major = words.next().unwrap_or(0) as u64;
    let minor = words.next().unwrap_or(0) as u64;
    let patch = words.next().unwrap_or(0) as u64;
    let build = words.next().unwrap_or(0) as u64;
    (major << 48) | (minor << 32) | (patch << 16) | build
}

fn build_icon() -> Vec<u8> {
    const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];
    let images = SIZES.map(render_dib);
    let directory_size = 6 + SIZES.len() * 16;
    let total_size = directory_size + images.iter().map(Vec::len).sum::<usize>();
    let mut icon = Vec::with_capacity(total_size);
    push_u16(&mut icon, 0);
    push_u16(&mut icon, 1);
    push_u16(&mut icon, SIZES.len() as u16);

    let mut offset = directory_size as u32;
    for (size, image) in SIZES.into_iter().zip(&images) {
        icon.push(if size == 256 { 0 } else { size as u8 });
        icon.push(if size == 256 { 0 } else { size as u8 });
        icon.push(0);
        icon.push(0);
        push_u16(&mut icon, 1);
        push_u16(&mut icon, 32);
        push_u32(&mut icon, image.len() as u32);
        push_u32(&mut icon, offset);
        offset += image.len() as u32;
    }
    for image in images {
        icon.extend(image);
    }
    icon
}

fn render_dib(size: u32) -> Vec<u8> {
    let pixel_bytes = (size * size * 4) as usize;
    let mask_stride = size.div_ceil(32) * 4;
    let mask_bytes = (mask_stride * size) as usize;
    let mut dib = Vec::with_capacity(40 + pixel_bytes + mask_bytes);
    push_u32(&mut dib, 40);
    push_i32(&mut dib, size as i32);
    push_i32(&mut dib, (size * 2) as i32);
    push_u16(&mut dib, 1);
    push_u16(&mut dib, 32);
    push_u32(&mut dib, 0);
    push_u32(&mut dib, pixel_bytes as u32);
    push_i32(&mut dib, 0);
    push_i32(&mut dib, 0);
    push_u32(&mut dib, 0);
    push_u32(&mut dib, 0);

    for stored_y in 0..size {
        let y = size - 1 - stored_y;
        for x in 0..size {
            let rgba = icon_pixel(x, y, size);
            dib.extend([rgba[2], rgba[1], rgba[0], rgba[3]]);
        }
    }
    dib.resize(40 + pixel_bytes + mask_bytes, 0);
    dib
}

fn icon_pixel(x: u32, y: u32, size: u32) -> [u8; 4] {
    let nx = (x as f32 + 0.5) / size as f32;
    let ny = (y as f32 + 0.5) / size as f32;
    let radius = 0.19;
    let qx = (f32::abs(nx - 0.5) - (0.5 - radius)).max(0.0);
    let qy = (f32::abs(ny - 0.5) - (0.5 - radius)).max(0.0);
    let signed_distance = f32::sqrt(qx * qx + qy * qy) - radius;
    if signed_distance > 0.0 {
        return [0, 0, 0, 0];
    }

    let amount = ((nx + ny) * 0.5).clamp(0.0, 1.0);
    let mut color = mix([91, 43, 153], [15, 115, 111], amount);
    if signed_distance > -0.035 {
        color = mix([188, 97, 255], [57, 238, 218], ny);
    }
    let is_top = (0.20..=0.80).contains(&nx) && (0.20..=0.33).contains(&ny);
    let is_stem = (0.445..=0.555).contains(&nx) && (0.25..=0.72).contains(&ny);
    if is_top || is_stem {
        color = mix([255, 255, 255], [207, 255, 251], ny);
    }
    if (0.20..=0.80).contains(&nx) && (0.79..=0.87).contains(&ny) {
        color = mix([190, 91, 255], [46, 230, 215], (nx - 0.20) / 0.60);
    }
    [color[0], color[1], color[2], 255]
}

fn mix(a: [u8; 3], b: [u8; 3], amount: f32) -> [u8; 3] {
    let amount = amount.clamp(0.0, 1.0);
    let channel =
        |from: u8, to: u8| (from as f32 + (to as f32 - from as f32) * amount).round() as u8;
    [
        channel(a[0], b[0]),
        channel(a[1], b[1]),
        channel(a[2], b[2]),
    ]
}

fn push_u16(buffer: &mut Vec<u8>, value: u16) {
    buffer.extend(value.to_le_bytes());
}

fn push_u32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend(value.to_le_bytes());
}

fn push_i32(buffer: &mut Vec<u8>, value: i32) {
    buffer.extend(value.to_le_bytes());
}
