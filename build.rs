use std::env;
use std::fs;
use std::io;
use std::path::Path;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    for path in [
        "src",
        "vendor",
        "assets",
        "Cargo.toml",
        "Cargo.lock",
        "LICENSE",
        "NOTICE",
        "THIRD_PARTY_NOTICES.txt",
        ".cargo/config.toml",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let git_output = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
    };
    for path in ["HEAD", "index", "refs"] {
        if let Some(path) = git_output(&["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={}", path.trim());
        }
    }
    let commit = git_output(&["rev-parse", "--verify", "HEAD"])
        .map(|value| value.trim().to_owned())
        .filter(|value| value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()));
    let dirty = git_output(&[
        "status",
        "--porcelain",
        "--untracked-files=normal",
        "--",
        "src",
        "vendor",
        "assets",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "LICENSE",
        "NOTICE",
        "THIRD_PARTY_NOTICES.txt",
        ".cargo/config.toml",
    ]);
    let identity = match (commit, dirty) {
        (Some(commit), Some(status)) => format!(
            "{commit}{}",
            if status.is_empty() { "" } else { "+modified" }
        ),
        _ => "source identity unavailable".into(),
    };
    println!("cargo:rustc-env=TRONTOP_BUILD_ID={identity}");
    println!(
        "cargo:rustc-env=TRONTOP_BUILD_TARGET={}",
        env::var("TARGET").unwrap_or_default()
    );
    let output = env::var_os("OUT_DIR").expect("Cargo did not provide OUT_DIR");
    let logo = image::open("assets/branding/trontop-logo-v2.png")
        .map_err(io::Error::other)?
        .into_rgba8();
    for size in [64, 128] {
        let scaled =
            image::imageops::resize(&logo, size, size, image::imageops::FilterType::Lanczos3);
        let alpha: Vec<u8> = scaled.pixels().map(|pixel| pixel[3]).collect();
        fs::write(
            Path::new(&output).join(format!("logo-alpha-{size}.bin")),
            alpha,
        )?;
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }
    let icon_path = Path::new(&output).join("trontop.ico");
    fs::write(&icon_path, build_icon(&logo))?;

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
        .set("PrivateBuild", &identity)
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

fn build_icon(logo: &image::RgbaImage) -> Vec<u8> {
    const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];
    let images = SIZES.map(|size| render_dib(size, logo));
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

fn render_dib(size: u32, logo: &image::RgbaImage) -> Vec<u8> {
    let scaled = image::imageops::resize(logo, size, size, image::imageops::FilterType::Lanczos3);
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
            let rgba = scaled.get_pixel(x, y).0;
            dib.extend([rgba[2], rgba[1], rgba[0], rgba[3]]);
        }
    }
    dib.resize(40 + pixel_bytes + mask_bytes, 0);
    dib
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
