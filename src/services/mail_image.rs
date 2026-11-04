//! A mail image is decompressed once, outside the critical section, into a shared preview.
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use miniz_oxide::inflate::{
    TINFLStatus,
    core::{DecompressorOxide, decompress, inflate_flags},
};
use serde_json::Value;
pub struct Preview {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u16>,
}
pub fn decode(value: &Value) -> Option<Arc<Preview>> {
    if value["codec"] != "rgb565-zlib" {
        return None;
    }
    let width = value["width"].as_u64()?;
    let height = value["height"].as_u64()?;
    if !(1..=108).contains(&width) || !(1..=80).contains(&height) {
        return None;
    }
    let (width, height) = (width as usize, height as usize);
    let encoded = value["data"].as_str()?;
    if encoded.len() > 24_000 || encoded.len() % 4 != 0 {
        return None;
    }
    let mut compressed = Vec::new();
    compressed.try_reserve_exact(encoded.len() / 4 * 3).ok()?;
    let (mut acc, mut bits, mut padding) = (0u32, 0u8, 0usize);
    for c in encoded.bytes() {
        if c == b'=' {
            padding += 1;
            if padding > 2 {
                return None;
            }
            continue;
        }
        if padding != 0 {
            return None;
        }
        let digit = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = ((acc << 6) | u32::from(digit)) & 0xffffff;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            compressed.push((acc >> bits) as u8);
        }
    }
    // Padding and unused low bits must agree. Otherwise malformed base64 can
    // silently decode to the same bytes as a valid image.
    let expected_padding = match (encoded.len() - padding) % 4 {
        0 => 0,
        2 => 2,
        3 => 1,
        _ => return None,
    };
    if padding != expected_padding || acc & ((1u32 << bits) - 1) != 0 {
        return None;
    }
    let expected = width * height * 2;
    let mut pixels = Vec::<u16>::new();
    pixels.try_reserve_exact(width * height).ok()?;
    pixels.resize(width * height, 0);
    // SAFETY: pixels owns width*height initialized u16 values. A mutable byte
    // view covers exactly that allocation, stays inside this scope and aliases
    // no other reference. Every byte pattern is valid for u16. Ownership and
    // allocation alignment remain with Vec<u16>; no Vec is reinterpreted.
    let output =
        unsafe { core::slice::from_raw_parts_mut(pixels.as_mut_ptr().cast::<u8>(), expected) };
    let mut decoder = Box::<DecompressorOxide>::default();
    let flags = inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
        | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    let (status, consumed, written) = decompress(&mut decoder, &compressed, output, 0, flags);
    if status != TINFLStatus::Done || written != expected || consumed != compressed.len() {
        return None;
    }
    // Wire pixels are little endian, even on the host regression-test target.
    for pixel in &mut pixels {
        *pixel = u16::from_le(*pixel);
    }
    // No intermediate decoded byte Vec or second pixel allocation is needed.
    drop(decoder);
    drop(compressed);
    Some(Arc::new(Preview {
        width,
        height,
        pixels,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maximum_size_preview_keeps_all_pixels() {
        let image = serde_json::json!({
            "codec":"rgb565-zlib", "width":108, "height":80,
            "data":"eJztwzENAAAIA7B9+HeIjJ0IoU2a7kRVVVVVVVVVVVVVVVVVVVVVnz4TrJTy"
        });
        let preview = decode(&image).unwrap();
        assert_eq!(preview.pixels.len(), 108 * 80);
        for pair in preview.pixels.chunks_exact(2) {
            assert_eq!(pair, [0xf800, 0x07e0]);
        }
    }

    #[test]
    fn pixels_decode_in_place_and_reject_bad_padding_checksum_and_trailing_data() {
        let image = |data: &str| {
            serde_json::json!({
                "codec":"rgb565-zlib", "width":1, "height":1, "data":data
            })
        };
        assert_eq!(decode(&image("eJxj+AEAAPoA+Q==")).unwrap().pixels, [0xf800]);
        for bad in [
            "eJxj+AEAAPoA+R==", // Nonzero unused base64 bits.
            "eJxj+AEAAPoA+Q=A", // Data after padding.
            "eJxj+AEAAPoA+Q=",  // Truncated padding.
            "eJxj+AEAAPoA+g==", // Wrong Adler-32.
            "eJxj+AEAAPoA+QAA", // Bytes following a complete zlib stream.
        ] {
            assert!(decode(&image(bad)).is_none(), "accepted {bad}");
        }
    }

    #[test]
    fn preview_rejects_oversized_corrupt_and_bomb_payloads() {
        assert!(
            decode(
                &serde_json::json!({"codec":"rgb565-zlib","width":109,"height":80,"data":"AAAA"})
            )
            .is_none()
        );
        assert!(
            decode(&serde_json::json!({"codec":"rgb565-zlib","width":1,"height":1,"data":"!!!!"}))
                .is_none()
        );
        // zlib containing more than two decoded bytes must fail the exact output bound.
        assert!(decode(&serde_json::json!({"codec":"rgb565-zlib","width":1,"height":1,"data":"eJxjYGBgAAAABAAB"})).is_none());
    }
}
