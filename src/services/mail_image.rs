//! A mail image is decompressed once, outside the critical section, into a shared preview.
use alloc::{sync::Arc, vec::Vec};
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
    let mut compressed = Vec::with_capacity(encoded.len() / 4 * 3);
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
    let expected = width * height * 2;
    let bytes =
        miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&compressed, expected).ok()?;
    if bytes.len() != expected {
        return None;
    }
    let pixels = bytes
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect();
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
