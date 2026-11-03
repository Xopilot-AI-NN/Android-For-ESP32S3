//! PNG декодируется по строкам: память зависит от ширины, а не от числа мегапикселей.
//! Поддерживаются 8-bit RGB/RGBA/Gray/GrayAlpha/Palette без Adam7.
use super::*;
use miniz_oxide::{
    DataFormat, MZFlush, MZStatus,
    inflate::stream::{InflateState, inflate},
};
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i32::from(a) + i32::from(b) - i32::from(c);
    let da = (p - i32::from(a)).abs();
    let db = (p - i32::from(b)).abs();
    let dc = (p - i32::from(c)).abs();
    if da <= db && da <= dc {
        a
    } else if db <= dc {
        b
    } else {
        c
    }
}
impl Fs {
    pub(super) fn png_thumbnail<D: BlockDevice>(
        &self,
        dev: &D,
        path: &str,
    ) -> Result<Vec<u16>, Error> {
        let mut at = 8;
        let mut width = 0usize;
        let mut height = 0usize;
        let mut bpp = 0;
        let mut color = 0;
        let mut palette = vec![0u8; 768];
        let mut alpha = vec![255u8; 256];
        let mut palette_len = 0;
        let mut previous = Vec::new();
        let mut row = Vec::new();
        let mut filled = 0;
        let mut y = 0;
        let mut out = vec![0u16; 240 * 200];
        let (mut ow, mut oh) = (0, 0);
        let mut ended = false;
        let mut state = InflateState::new_boxed(DataFormat::Zlib);
        loop {
            let header = self.read_range(dev, path, at, 8)?;
            if header.len() != 8 {
                return Err(Error::Corrupt);
            }
            let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
            let kind = &header[4..8];
            at += 8;
            if length > MAX_BLOB {
                return Err(Error::TooLarge);
            }
            let mut crc = gpt::crc32_update(!0, kind);
            let mut remaining = length;
            if kind == b"IHDR" {
                if width != 0 || length != 13 {
                    return Err(Error::Corrupt);
                }
                let data = self.read_range(dev, path, at, length)?;
                if data.len() != 13 {
                    return Err(Error::Corrupt);
                }
                crc = gpt::crc32_update(crc, &data);
                at += length;
                remaining = 0;
                width = u32::from_be_bytes(data[..4].try_into().unwrap()) as usize;
                height = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
                if width == 0
                    || height == 0
                    || width > 8192
                    || height > 8192
                    || data[8] != 8
                    || data[10..13] != [0, 0, 0]
                {
                    return Err(Error::InvalidPath);
                }
                color = data[9];
                bpp = match color {
                    0 | 3 => 1,
                    2 => 3,
                    4 => 2,
                    6 => 4,
                    _ => return Err(Error::InvalidPath),
                };
                previous = vec![0; width * bpp];
                row = vec![0; width * bpp + 1];
                ow = width;
                oh = height;
                if ow > 240 {
                    oh = oh * 240 / ow;
                    ow = 240;
                }
                if oh > 200 {
                    ow = ow * 200 / oh;
                    oh = 200;
                }
                if ow == 0 || oh == 0 {
                    return Err(Error::InvalidPath);
                }
            }
            while remaining > 0 {
                let take = remaining.min(4096);
                let data = self.read_range(dev, path, at, take)?;
                if data.len() != take {
                    return Err(Error::Corrupt);
                }
                crc = gpt::crc32_update(crc, &data);
                let processed = length - remaining;
                if kind == b"PLTE" {
                    if length > 768 || length % 3 != 0 {
                        return Err(Error::Corrupt);
                    }
                    palette[processed..processed + take].copy_from_slice(&data);
                    palette_len = length / 3;
                } else if kind == b"tRNS" && color == 3 {
                    if length > 256 {
                        return Err(Error::Corrupt);
                    }
                    alpha[processed..processed + take].copy_from_slice(&data);
                } else if kind == b"IDAT" {
                    if width == 0 || ended {
                        return Err(Error::Corrupt);
                    }
                    let mut consumed = 0;
                    loop {
                        let result = inflate(
                            &mut state,
                            &data[consumed..],
                            &mut row[filled..],
                            MZFlush::None,
                        );
                        consumed += result.bytes_consumed;
                        filled += result.bytes_written;
                        let status = match result.status {
                            Ok(s) => s,
                            Err(miniz_oxide::MZError::Buf)
                                if consumed == data.len() && result.bytes_written == 0 =>
                            {
                                break;
                            }
                            Err(_) => return Err(Error::Corrupt),
                        };
                        if filled == row.len() {
                            if y >= height {
                                return Err(Error::Corrupt);
                            }
                            let filter = row[0];
                            for x in 0..previous.len() {
                                let left = if x >= bpp { row[x + 1 - bpp] } else { 0 };
                                let up = previous[x];
                                let ul = if x >= bpp { previous[x - bpp] } else { 0 };
                                let predictor = match filter {
                                    0 => 0,
                                    1 => left,
                                    2 => up,
                                    3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                                    4 => paeth(left, up, ul),
                                    _ => return Err(Error::Corrupt),
                                };
                                row[x + 1] = row[x + 1].wrapping_add(predictor);
                            }
                            for x in 0..width {
                                let i = 1 + x * bpp;
                                let (r, g, b, a) = match color {
                                    0 => (row[i], row[i], row[i], 255),
                                    2 => (row[i], row[i + 1], row[i + 2], 255),
                                    4 => (row[i], row[i], row[i], row[i + 1]),
                                    6 => (row[i], row[i + 1], row[i + 2], row[i + 3]),
                                    3 => {
                                        let index = row[i] as usize;
                                        if index >= palette_len {
                                            return Err(Error::Corrupt);
                                        }
                                        (
                                            palette[index * 3],
                                            palette[index * 3 + 1],
                                            palette[index * 3 + 2],
                                            alpha[index],
                                        )
                                    }
                                    _ => unreachable!(),
                                };
                                let r = (u16::from(r) * u16::from(a) / 255) as u8;
                                let g = (u16::from(g) * u16::from(a) / 255) as u8;
                                let b = (u16::from(b) * u16::from(a) / 255) as u8;
                                let pixel = ((u16::from(r) & 248) << 8)
                                    | ((u16::from(g) & 252) << 3)
                                    | (u16::from(b) >> 3);
                                for dy in y * oh / height..(y + 1) * oh / height {
                                    for dx in x * ow / width..(x + 1) * ow / width {
                                        out[(dy + (200 - oh) / 2) * 240 + dx + (240 - ow) / 2] =
                                            pixel;
                                    }
                                }
                            }
                            previous.copy_from_slice(&row[1..]);
                            filled = 0;
                            y += 1;
                        }
                        if status == MZStatus::StreamEnd {
                            ended = true;
                            if consumed != data.len() {
                                return Err(Error::Corrupt);
                            }
                            break;
                        }
                        if result.bytes_consumed == 0 && result.bytes_written == 0 {
                            if consumed == data.len() {
                                break;
                            }
                            return Err(Error::Corrupt);
                        }
                    }
                } else if kind[0].is_ascii_uppercase() && !matches!(kind, b"PLTE" | b"IEND") {
                    return Err(Error::InvalidPath);
                }
                at += take;
                remaining -= take;
            }
            let check = self.read_range(dev, path, at, 4)?;
            if check.len() != 4 || !crc != u32::from_be_bytes(check[..].try_into().unwrap()) {
                return Err(Error::Corrupt);
            }
            at += 4;
            if kind == b"IEND" {
                if length != 0 || !ended || y != height || filled != 0 {
                    return Err(Error::Corrupt);
                }
                return Ok(out);
            }
        }
    }
}
