//! Порционное чтение JPEG непосредственно из WFS; выход всегда 240×200 RGB565.
use super::*;
use core::ffi::c_void;
unsafe extern "C" {
    fn watch_jpeg(
        read: unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> usize,
        ctx: *mut c_void,
        pixels: *mut u16,
        work: *mut c_void,
        size: usize,
    ) -> i32;
}
struct Input<'a, D: BlockDevice> {
    fs: &'a Fs,
    dev: &'a D,
    path: &'a str,
    offset: usize,
    size: usize,
}
/// C decoder may request skipping APP/EXIF bytes with a null output pointer.
unsafe extern "C" fn read<D: BlockDevice>(
    ctx: *mut c_void,
    buffer: *mut u8,
    length: usize,
) -> usize {
    let input = unsafe { &mut *(ctx as *mut Input<'_, D>) };
    let len = length.min(input.size.saturating_sub(input.offset));
    if buffer.is_null() {
        input.offset += len;
        return len;
    }
    let Ok(bytes) = input
        .fs
        .read_range(input.dev, input.path, input.offset, len.min(4096))
    else {
        return 0;
    };
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
    }
    input.offset += bytes.len();
    bytes.len()
}
impl Fs {
    /// Возвращает ограниченный RGB565 preview, не выделяя RAM под исходное фото.
    pub fn thumbnail<D: BlockDevice>(&self, dev: &D, path: &str) -> Result<Vec<u8>, Error> {
        let entry = &self.entries[self.index(path)?];
        let header = self.read_range(dev, path, 0, 8)?;
        let pixels = if header.starts_with(&[0xff, 0xd8]) {
            let mut input = Input {
                fs: self,
                dev,
                path,
                offset: 0,
                size: entry.size as usize,
            };
            let mut pixels = vec![0u16; 240 * 200];
            let mut work = vec![0u32; 2048];
            // Context and buffers outlive C decoding; callback is synchronous and never unwinds.
            let result = unsafe {
                watch_jpeg(
                    read::<D>,
                    &mut input as *mut _ as *mut c_void,
                    pixels.as_mut_ptr(),
                    work.as_mut_ptr() as *mut c_void,
                    work.len() * 4,
                )
            };
            if result != 0 {
                return Err(Error::Corrupt);
            }
            pixels
        } else if header == b"\x89PNG\r\n\x1a\n" {
            self.png_thumbnail(dev, path)?
        } else {
            return Err(Error::InvalidPath);
        };
        let mut bytes = Vec::with_capacity(pixels.len() * 2);
        for p in pixels {
            bytes.extend_from_slice(&p.to_le_bytes());
        }
        Ok(bytes)
    }
}

struct SliceInput<'a> {
    bytes: &'a [u8],
    at: usize,
}
unsafe extern "C" fn read_slice(ctx: *mut c_void, buffer: *mut u8, length: usize) -> usize {
    let input = unsafe { &mut *(ctx as *mut SliceInput<'_>) };
    let count = length.min(input.bytes.len().saturating_sub(input.at));
    if !buffer.is_null() {
        unsafe {
            core::ptr::copy_nonoverlapping(input.bytes.as_ptr().add(input.at), buffer, count);
        }
    }
    input.at += count;
    count
}
pub(super) fn from_slice(bytes: &[u8]) -> Result<Vec<u16>, Error> {
    // Public pages can point at decompression bombs. Keep network work small
    // even though local gallery files permit larger camera photos.
    let (width, height) = network_dimensions(bytes).ok_or(Error::Corrupt)?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 4_000_000 {
        return Err(Error::TooLarge);
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return super::png::decode(
            |at, count| {
                let end = at.checked_add(count).ok_or(Error::Corrupt)?;
                bytes.get(at..end).map(|v| v.to_vec()).ok_or(Error::Corrupt)
            },
            255,
        );
    }
    if !bytes.starts_with(&[255, 216]) {
        return Err(Error::InvalidPath);
    }
    let mut input = SliceInput { bytes, at: 0 };
    let mut pixels = vec![0u16; 240 * 200];
    let mut work = vec![0u32; 2048];
    let result = unsafe {
        watch_jpeg(
            read_slice,
            &mut input as *mut _ as *mut c_void,
            pixels.as_mut_ptr(),
            work.as_mut_ptr() as *mut c_void,
            work.len() * 4,
        )
    };
    if result != 0 {
        return Err(Error::Corrupt);
    }
    Ok(pixels)
}

fn network_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if bytes.get(12..16)? != b"IHDR" {
            return None;
        }
        return Some((
            u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?),
            u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?),
        ));
    }
    if !bytes.starts_with(&[255, 216]) {
        return None;
    }
    let mut at = 2;
    while at < bytes.len() {
        if *bytes.get(at)? != 255 {
            return None;
        }
        while bytes.get(at) == Some(&255) {
            at += 1;
        }
        let marker = *bytes.get(at)?;
        at += 1;
        if marker == 0xd9 || marker == 0xda {
            return None;
        }
        let size = u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?) as usize;
        if size < 2 {
            return None;
        }
        let part = bytes.get(at..at.checked_add(size)?)?;
        if marker == 0xc0 {
            let height = u16::from_be_bytes(part.get(3..5)?.try_into().ok()?) as u32;
            let width = u16::from_be_bytes(part.get(5..7)?.try_into().ok()?) as u32;
            return Some((width, height));
        }
        at += size;
    }
    None
}
