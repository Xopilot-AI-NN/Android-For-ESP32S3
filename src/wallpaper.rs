//! Bounded RGB565 assets in reserved userdata ranges. Never stores executable code.
use crate::gpt;
use alloc::{vec, vec::Vec};
use embedded_sdmmc::BlockDevice;
pub const SCREENSHOT_OFFSET: u64 = 1024 * 1024;
const PIXELS: usize = 240 * 280;
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
