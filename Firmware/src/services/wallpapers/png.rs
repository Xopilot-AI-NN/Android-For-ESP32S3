// Модуль декодирования PNG.
//! Bounded PNG decoder for 240x280, 8-bit, non-interlaced wallpaper images.
use alloc::{vec, vec::Vec};
/// Читает целое число PNG в порядке big-endian.
fn be(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
/// Вычисляет предиктор фильтра Paeth для восстановления строки PNG.
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (a, b, c) = (i32::from(a), i32::from(b), i32::from(c));
    let p = a + b - c;
    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if pa <= pb && pa <= pc {
        a as u8
    } else if pb <= pc {
        b as u8
    } else {
        c as u8
    }
}
/// Проверяет двоичное представление и восстанавливает типизированные данные.
pub fn decode(bytes: &[u8]) -> Result<Vec<u16>, &'static str> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("send a 240x280 PNG");
    }
    let mut pos = 8usize;
    let mut channels = 0usize;
    let mut kind = 0u8;
    let mut compressed = Vec::new();
    let mut palette = Vec::new();
    let mut alpha = [255u8; 256];
    let mut ended = false;
    while pos
        .checked_add(12)
        .filter(|&end| end <= bytes.len())
        .is_some()
    {
        let n = be(&bytes[pos..pos + 4]) as usize;
        let end = pos
            .checked_add(12)
            .and_then(|v| v.checked_add(n))
            .filter(|&v| v <= bytes.len())
            .ok_or("bad PNG length")?;
        let tag = &bytes[pos + 4..pos + 8];
        let data = &bytes[pos + 8..end - 4];
        if crate::gpt::crc32(&bytes[pos + 4..end - 4]) != be(&bytes[end - 4..end]) {
            return Err("PNG CRC mismatch");
        }
        match tag {
            b"IHDR" => {
                if n != 13
                    || be(&data[..4]) != 240
                    || be(&data[4..8]) != 280
                    || data[8] != 8
                    || data[10..13] != [0, 0, 0]
                {
                    return Err("PNG must be 240x280, 8-bit, non-interlaced");
                }
                kind = data[9];
                channels = match kind {
                    0 | 3 => 1,
                    2 => 3,
                    4 => 2,
                    6 => 4,
                    _ => return Err("unsupported PNG color"),
                };
            }
            b"PLTE" => {
                if n > 768 || n % 3 != 0 {
                    return Err("invalid palette");
                }
                palette.extend_from_slice(data);
            }
            b"tRNS" if kind == 3 => {
                if n > 256 {
                    return Err("invalid alpha palette");
                }
                alpha[..n].copy_from_slice(data);
            }
            b"IDAT" => {
                if compressed.len() + n > 512 * 1024 {
                    return Err("PNG too large");
                }
                compressed.extend_from_slice(data);
            }
            b"IEND" => {
                ended = true;
                break;
            }
            _ => {}
        }
        pos = end;
    }
    if channels == 0 || !ended {
        return Err("incomplete PNG");
    }
    let stride = 240 * channels;
    let expected = (stride + 1) * 280;
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&compressed, expected)
        .map_err(|_| "invalid PNG data")?;
    if raw.len() != expected {
        return Err("invalid PNG rows");
    }
    drop(compressed);
    let mut previous = vec![0u8; stride];
    let mut row = vec![0u8; stride];
    let mut pixels = Vec::with_capacity(240 * 280);
    for y in 0..280 {
        let input = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        for x in 0..stride {
            let a = if x >= channels { row[x - channels] } else { 0 };
            let b = previous[x];
            let c = if x >= channels {
                previous[x - channels]
            } else {
                0
            };
            let predictor = match input[0] {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
                4 => paeth(a, b, c),
                _ => return Err("invalid PNG filter"),
            };
            row[x] = input[x + 1].wrapping_add(predictor);
        }
        for x in 0..240 {
            let i = x * channels;
            let (r, g, b, a) = match kind {
                2 => (row[i], row[i + 1], row[i + 2], 255),
                6 => (row[i], row[i + 1], row[i + 2], row[i + 3]),
                0 => (row[i], row[i], row[i], 255),
                4 => (row[i], row[i], row[i], row[i + 1]),
                3 => {
                    let p = usize::from(row[i]) * 3;
                    if p + 2 >= palette.len() {
                        return Err("invalid palette index");
                    }
                    (
                        palette[p],
                        palette[p + 1],
                        palette[p + 2],
                        alpha[usize::from(row[i])],
                    )
                }
                _ => return Err("unsupported PNG"),
            };
            let r = u16::from(r) * u16::from(a) / 255;
            let g = u16::from(g) * u16::from(a) / 255;
            let b = u16::from(b) * u16::from(a) / 255;
            pixels.push(((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3));
        }
        core::mem::swap(&mut row, &mut previous);
    }
    Ok(pixels)
}
