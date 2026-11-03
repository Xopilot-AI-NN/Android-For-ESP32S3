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
