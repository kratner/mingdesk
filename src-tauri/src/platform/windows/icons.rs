//! Apps' icons for the board, as PNGs, cached per executable.
//!
//! - A Store app's icon is its package's own logo file (the one the Start menu and taskbar use), read from disk.
//! - Anything else: the exe's main icon at 64 px (SHDefExtractIconW), drawn into a 32-bit bitmap and saved as a
//!   PNG. Icons without an alpha channel (old ones) take their transparency from the icon's mask.

use super::rules;
use ::windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HBITMAP, HGDIOBJ,
};
use ::windows::Win32::UI::Shell::SHDefExtractIconW;
use ::windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use ::windows::core::HSTRING;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const SIZE: u32 = 64;

pub fn icon(exe: &str) -> Option<Vec<u8>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Vec<u8>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().unwrap().get(exe) {
        return hit.clone();
    }
    let png = store_logo(exe).or_else(|| exe_icon(exe));
    cache.lock().unwrap().insert(exe.to_string(), png.clone());
    png
}

/// A Store app's logo file, the best of its variants on disk.
fn store_logo(exe: &str) -> Option<Vec<u8>> {
    let (folder, _) = rules::store_package(exe)?;
    let xml = std::fs::read_to_string(Path::new(folder).join("AppxManifest.xml")).ok()?;
    let named = Path::new(folder).join(rules::manifest_logo(&xml)?);
    let dir = named.parent()?;
    let stem = named.file_stem()?.to_string_lossy().into_owned();
    let names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    std::fs::read(dir.join(rules::best_logo(&stem, &refs)?)).ok()
}

fn exe_icon(exe: &str) -> Option<Vec<u8>> {
    let mut large = HICON::default();
    // SAFETY: asks for one icon at SIZE px; the icon is destroyed after it's drawn.
    unsafe {
        SHDefExtractIconW(&HSTRING::from(exe), 0, 0, Some(&mut large), None, SIZE)
            .ok()
            .ok()?;
        if large.is_invalid() {
            return None;
        }
        let png = draw(large);
        let _ = DestroyIcon(large);
        png
    }
}

/// A bitmap's pixels as top-down BGRA.
unsafe fn pixels(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut info = BITMAP::default();
    // SAFETY: fills a BITMAP from a bitmap handle the caller owns.
    if unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            size_of::<BITMAP>() as i32,
            Some(&mut info as *mut _ as *mut _),
        )
    } == 0
    {
        return None;
    }
    let (w, h) = (info.bmWidth as u32, info.bmHeight.unsigned_abs());
    let mut header = BITMAPINFO::default();
    header.bmiHeader = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w as i32,
        biHeight: -(h as i32), // top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    // SAFETY: the buffer holds w×h 32-bit pixels, as the header asks for; the DC is deleted before returning.
    unsafe {
        let dc = CreateCompatibleDC(None);
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            h,
            Some(buf.as_mut_ptr() as *mut _),
            &mut header,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        (lines == h as i32).then_some((w, h, buf))
    }
}

unsafe fn draw(icon: HICON) -> Option<Vec<u8>> {
    let mut parts = ICONINFO::default();
    // SAFETY: GetIconInfo hands over two bitmaps, both deleted below.
    unsafe { GetIconInfo(icon, &mut parts) }.ok()?;
    // SAFETY: the bitmaps came from GetIconInfo just above.
    let colour = unsafe { pixels(parts.hbmColor) };
    let mask = unsafe { pixels(parts.hbmMask) };
    unsafe {
        let _ = DeleteObject(HGDIOBJ(parts.hbmColor.0));
        let _ = DeleteObject(HGDIOBJ(parts.hbmMask.0));
    }
    let (w, h, mut bgra) = colour?;
    if bgra.chunks_exact(4).all(|p| p[3] == 0) {
        // No alpha channel: opaque wherever the mask is black.
        let (_, mh, m) = mask?;
        if mh < h {
            return None;
        }
        for (p, mp) in bgra.chunks_exact_mut(4).zip(m.chunks_exact(4)) {
            p[3] = if mp[0] == 0 { 255 } else { 0 };
        }
    }
    for p in bgra.chunks_exact_mut(4) {
        p.swap(0, 2); // BGRA → RGBA
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().ok()?.write_image_data(&bgra).ok()?;
    }
    Some(out)
}
