//! Bounded RGB565 assets in reserved userdata ranges. Never stores executable code.
use crate::gpt;
use alloc::{vec, vec::Vec};
use embedded_sdmmc::BlockDevice;
pub const SCREENSHOT_OFFSET: u64 = 1024 * 1024;
const PIXELS: usize = 240 * 280;
#[path = "wallpapers/builtins.rs"]
mod builtins;
#[path = "wallpapers/png.rs"]
mod png;
/// Read-only virtual files exported over ADB; no image decoding or heap copy.
pub fn builtin_png(index: u8) -> Option<&'static [u8]> {
    if index == 0 {
        return None;
    }
    builtins::IMAGES.get(usize::from(index - 1)).map(|v| v.1)
}
pub fn builtin_file(path: &str) -> Option<&'static [u8]> {
    let number = path
        .strip_prefix("/sdcard/Wallpapers/")?
        .strip_suffix(".png")?;
    builtin_png(number.parse::<u8>().ok()?)
}
pub fn builtin_count() -> u8 {
    builtins::IMAGES.len() as u8
}
pub fn name(index: u8) -> &'static str {
    if index == 0 {
        "Загруженные"
    } else {
        builtins::IMAGES
            .get(usize::from(index - 1))
            .map(|v| v.0)
            .unwrap_or("Обои")
    }
}
pub fn builtin(index: u8) -> Option<Vec<u16>> {
    let (_, bytes) = builtins::IMAGES.get(usize::from(index.saturating_sub(1)))?;
    png::decode(bytes).ok()
}
pub fn load<D: BlockDevice>(dev: &D) -> Option<Vec<u16>> {
    let part = gpt::find_partition(dev, "userdata").ok()?;
    let mut header = [0; 512];
    gpt::read_partition(dev, part, 0, &mut header).ok()?;
    if &header[..8] != b"WALLv1\0\0"
        || u16::from_le_bytes([header[8], header[9]]) != 240
        || u16::from_le_bytes([header[10], header[11]]) != 280
    {
        return None;
    }
    let len = u32::from_le_bytes(header[12..16].try_into().ok()?) as usize;
    if len != PIXELS * 2 {
        return None;
    }
    let crc = u32::from_le_bytes(header[16..20].try_into().ok()?);
    let mut bytes = vec![0; len];
    gpt::read_partition(dev, part, 512, &mut bytes).ok()?;
    if gpt::crc32(&bytes) != crc {
        return None;
    }
    Some(
        bytes
            .chunks_exact(2)
            .map(|p| u16::from_be_bytes([p[0], p[1]]))
            .collect(),
    )
}
pub fn save_screenshot<D: BlockDevice>(dev: &D, pixels: &[u16]) -> bool {
    if pixels.len() != PIXELS {
        return false;
    }
    let Ok(part) = gpt::find_partition(dev, "userdata") else {
        return false;
    };
    // Existing filesystem data is never a valid scratch area for a capture.
    let mut previous = [0u8; 512];
    if gpt::read_partition(dev, part, 0, &mut previous).is_err()
        || (previous.iter().any(|&b| b != 0) && &previous[..8] != b"WALLv1\0\0")
    {
        return false;
    }
    let mut bytes = Vec::with_capacity(PIXELS * 2);
    for p in pixels {
        bytes.extend_from_slice(&p.to_be_bytes());
    }
    let mut header = [0; 512];
    header[..8].copy_from_slice(b"SHOTv1\0\0");
    header[8..10].copy_from_slice(&240u16.to_le_bytes());
    header[10..12].copy_from_slice(&280u16.to_le_bytes());
    header[12..16].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
    header[16..20].copy_from_slice(&gpt::crc32(&bytes).to_le_bytes());
    gpt::write_partition(dev, part, SCREENSHOT_OFFSET + 512, &bytes).is_ok()
        && gpt::write_partition(dev, part, SCREENSHOT_OFFSET, &header).is_ok()
}

// ADB receives bytes in the network service; the boot loop owns disk writes.
// Do not acknowledge SYNC completion until the image has actually been saved.
use core::cell::RefCell;
use critical_section::Mutex;
enum Upload {
    Idle,
    Pending(Vec<u16>),
    Applying,
    Finished(bool),
}
static UPLOAD: Mutex<RefCell<Upload>> = Mutex::new(RefCell::new(Upload::Idle));
pub fn queue_png(bytes: &[u8]) -> Result<(), &'static str> {
    let pixels = png::decode(bytes)?;
    critical_section::with(|cs| {
        let mut upload = UPLOAD.borrow(cs).borrow_mut();
        if !matches!(*upload, Upload::Idle) {
            return Err("another wallpaper upload is pending");
        }
        *upload = Upload::Pending(pixels);
        Ok(())
    })
}
pub fn take_upload() -> Option<Vec<u16>> {
    critical_section::with(|cs| {
        let mut upload = UPLOAD.borrow(cs).borrow_mut();
        match core::mem::replace(&mut *upload, Upload::Applying) {
            Upload::Pending(pixels) => Some(pixels),
            other => {
                *upload = other;
                None
            }
        }
    })
}
pub fn complete_upload(ok: bool) {
    critical_section::with(|cs| *UPLOAD.borrow(cs).borrow_mut() = Upload::Finished(ok));
}
pub fn cancel_upload() {
    critical_section::with(|cs| *UPLOAD.borrow(cs).borrow_mut() = Upload::Idle);
}
pub fn upload_result() -> Option<bool> {
    critical_section::with(|cs| {
        let mut upload = UPLOAD.borrow(cs).borrow_mut();
        if let Upload::Finished(ok) = *upload {
            *upload = Upload::Idle;
            Some(ok)
        } else {
            None
        }
    })
}
pub fn save<D: BlockDevice>(dev: &D, pixels: &[u16]) -> bool {
    if pixels.len() != PIXELS {
        return false;
    }
    let Ok(part) = gpt::find_partition(dev, "userdata") else {
        return false;
    };
    let mut old = [0; 512];
    if gpt::read_partition(dev, part, 0, &mut old).is_err()
        || (old.iter().any(|&v| v != 0) && &old[..8] != b"WALLv1\0\0")
    {
        return false;
    }
    let mut bytes = Vec::with_capacity(PIXELS * 2);
    for pixel in pixels {
        bytes.extend_from_slice(&pixel.to_be_bytes());
    }
    let mut header = [0; 512];
    header[..8].copy_from_slice(b"WALLv1\0\0");
    header[8..10].copy_from_slice(&240u16.to_le_bytes());
    header[10..12].copy_from_slice(&280u16.to_le_bytes());
    header[12..16].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
    header[16..20].copy_from_slice(&gpt::crc32(&bytes).to_le_bytes());
    gpt::write_partition(dev, part, 512, &bytes).is_ok()
        && gpt::write_partition(dev, part, 0, &header).is_ok()
}
