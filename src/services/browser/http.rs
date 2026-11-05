//! Bounded HTTP framing for public pages. No cookies, credentials or scripting.
use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use miniz_oxide::inflate::TINFLStatus;
use miniz_oxide::inflate::core::{DecompressorOxide, decompress, inflate_flags};
pub const BODY_LIMIT: usize = 192 * 1024;
pub const TEXT_LIMIT: usize = 192 * 1024;
const HEADER_LIMIT: usize = 8192;
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub location: String,
    pub encoding: String,
    pub body: Vec<u8>,
    pub truncated: bool,
}
/// Return a complete or deliberately capped response. A capped image is rejected
/// by the decoder; text pages may show their bounded prefix with a visible note.
pub fn response(bytes: &[u8], eof: bool) -> Result<Option<Response>, &'static str> {
    let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
        if bytes.len() > HEADER_LIMIT || eof {
            return Err("Неполные HTTP-заголовки");
        }
        return Ok(None);
    };
    if end > HEADER_LIMIT {
        return Err("Заголовки сайта слишком большие");
    }
    let header = core::str::from_utf8(&bytes[..end]).map_err(|_| "Неверный HTTP-ответ")?;
    let mut lines = header.split("\r\n");
    let mut status = lines.next().ok_or("Нет HTTP-статуса")?.split_whitespace();
    if !matches!(status.next(), Some("HTTP/1.1" | "HTTP/1.0")) {
        return Err("Неверный HTTP-протокол");
    }
    let status = status
        .next()
        .and_then(|v| v.parse().ok())
        .filter(|s| (200..=599).contains(s))
        .ok_or("Неверный HTTP-статус")?;
    let (mut length, mut chunked, mut content_type, mut location, mut encoding) =
        (None, false, String::new(), String::new(), String::new());
    for line in lines {
        let (k, v) = line.split_once(':').ok_or("Неверный HTTP-заголовок")?;
        let v = v.trim();
        if k.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err("Повторная длина ответа");
            }
            length = Some(v.parse::<usize>().map_err(|_| "Неверная длина ответа")?);
        } else if k.eq_ignore_ascii_case("transfer-encoding") {
            if chunked || !v.eq_ignore_ascii_case("chunked") {
                return Err("Неподдерживаемая передача HTTP");
            }
            chunked = true;
        } else if k.eq_ignore_ascii_case("content-type") {
            content_type = v.to_ascii_lowercase();
        } else if k.eq_ignore_ascii_case("location") {
            if !location.is_empty() || v.len() > 2048 {
                return Err("Неверный redirect");
            }
            location = v.to_string();
        } else if k.eq_ignore_ascii_case("content-encoding") {
            if !encoding.is_empty() {
                return Err("Повторное сжатие HTTP");
            }
            encoding = v.to_ascii_lowercase();
        }
    }
    if chunked && length.is_some() {
        return Err("Неоднозначная длина HTTP");
    }
    let data = &bytes[end + 4..];
    let (mut truncated, body) = if matches!(status, 204 | 304) || (300..=399).contains(&status) {
        (false, Vec::new())
    } else if chunked {
        let (mut at, mut size, mut done, mut cap) = (0usize, 0usize, false, false);
        // First validate the chunk stream without allocating a body on every TLS read.
        loop {
            let Some(n) = data
                .get(at..)
                .and_then(|v| v.windows(2).position(|p| p == b"\r\n"))
            else {
                if eof {
                    return Err("Неполный chunk");
                }
                return Ok(None);
            };
            if n > 128 {
                return Err("Неверный chunk");
            }
            let count = core::str::from_utf8(&data[at..at + n])
                .ok()
                .and_then(|v| usize::from_str_radix(v.split(';').next().unwrap_or(""), 16).ok())
                .ok_or("Неверный chunk")?;
            at += n + 2;
            if count == 0 {
                let tail = &data[at..];
                done = tail.starts_with(b"\r\n") || tail.windows(4).any(|v| v == b"\r\n\r\n");
                break;
            }
            let take = count.min(BODY_LIMIT - size);
            if data.len().saturating_sub(at) < take {
                if eof {
                    return Err("Неполный chunk");
                }
                return Ok(None);
            }
            size += take;
            if size == BODY_LIMIT {
                cap = true;
                break;
            }
            if data.len().saturating_sub(at) < count + 2 {
                if eof {
                    return Err("Неполный chunk");
                }
                return Ok(None);
            }
            if &data[at + count..at + count + 2] != b"\r\n" {
                return Err("Неверная граница chunk");
            }
            at += count + 2;
        }
        if !done && !cap {
            if eof {
                return Err("Неполный HTTP-ответ");
            }
            return Ok(None);
        }
        let mut body = Vec::with_capacity(size);
        let mut at = 0;
        while body.len() < size {
            let n = data[at..].windows(2).position(|p| p == b"\r\n").unwrap();
            let count = usize::from_str_radix(
                core::str::from_utf8(&data[at..at + n])
                    .unwrap()
                    .split(';')
                    .next()
                    .unwrap(),
                16,
            )
            .unwrap();
            at += n + 2;
            let take = count.min(size - body.len());
            body.extend_from_slice(&data[at..at + take]);
            at += take + 2;
        }
        (cap, body)
    } else if let Some(length) = length {
        let take = length.min(BODY_LIMIT);
        if data.len() < take {
            if eof {
                return Err("Неполная страница");
            }
            return Ok(None);
        }
        (length > BODY_LIMIT, data[..take].to_vec())
    } else {
        if !eof && data.len() < BODY_LIMIT {
            return Ok(None);
        }
        (
            !eof || data.len() > BODY_LIMIT,
            data[..data.len().min(BODY_LIMIT)].to_vec(),
        )
    };
    let body = if encoding.is_empty()
        || encoding == "identity"
        || (300..=399).contains(&status)
        || status == 204
    {
        body
    } else if encoding == "gzip" {
        // The server may send gzip despite Accept-Encoding: identity. Bound inflated
        // output before touching untrusted compressed bytes; no allocation by ISIZE.
        if body.len() < 18 || body[..3] != [31, 139, 8] {
            return Err("Неверный gzip");
        }
        let flags = body[3];
        if flags & 0xe0 != 0 {
            return Err("Неверный gzip");
        }
        let mut at = 10;
        if flags & 4 != 0 {
            let n = body.get(at..at + 2).ok_or("Неполный gzip")?;
            at += 2 + u16::from_le_bytes([n[0], n[1]]) as usize;
        }
        for bit in [8, 16] {
            if flags & bit != 0 {
                let n = body
                    .get(at..)
                    .and_then(|v| v.iter().position(|b| *b == 0))
                    .ok_or("Неполный gzip")?;
                at += n + 1;
            }
        }
        if flags & 2 != 0 {
            at += 2;
        }
        let input = body.get(at..).ok_or("Неполный gzip")?;
        let mut out = alloc::vec![0;TEXT_LIMIT];
        let mut decoder = Box::<DecompressorOxide>::default();
        let (status, consumed, written) = decompress(
            &mut decoder,
            input,
            &mut out,
            0,
            inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
        );
        out.truncate(written);
        if status == TINFLStatus::HasMoreOutput && written == TEXT_LIMIT {
            truncated = true;
        } else if status == TINFLStatus::Done {
            let footer = input.get(consumed..consumed + 8).ok_or("Неполный gzip")?;
            if input.len() != consumed + 8 {
                return Err("Составной gzip не поддерживается");
            }
            let crc = !crate::gpt::crc32_update(!0, &out);
            if crc != u32::from_le_bytes(footer[..4].try_into().unwrap())
                || written as u32 != u32::from_le_bytes(footer[4..].try_into().unwrap())
            {
                return Err("Повреждённый gzip");
            }
        } else {
            return Err("Неподдерживаемый gzip");
        }
        out
    } else {
        return Err("Сжатие сайта не поддерживается");
    };
    Ok(Some(Response {
        status,
        content_type,
        location,
        encoding,
        body,
        truncated,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunked_eof_redirect_and_bad_framing() {
        assert!(
            response(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n",
                false
            )
            .unwrap()
            .unwrap()
            .body
                == b"abc"
        );
        assert!(
            response(b"HTTP/1.1 200 OK\r\n\r\nbody", false)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            response(b"HTTP/1.1 200 OK\r\n\r\nbody", true)
                .unwrap()
                .unwrap()
                .body,
            b"body"
        );
        assert!(
            response(
                b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nTransfer-Encoding: chunked\r\n\r\n",
                false
            )
            .is_err()
        );
        assert!(
            response(b"HTTP/1.1 302 Found\r\nLocation: /next\r\n\r\n", false)
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn caps_large_pages_and_never_trusts_announced_size() {
        let mut raw = b"HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\n\r\n".to_vec();
        raw.resize(raw.len() + BODY_LIMIT, b'a');
        let r = response(&raw, false).unwrap().unwrap();
        assert!(r.truncated);
        assert_eq!(r.body.len(), BODY_LIMIT);
    }
}

#[cfg(test)]
mod more_tests {
    use super::*;
    #[test]
    fn fragment_boundaries_and_gzip_integrity_are_checked() {
        let raw=b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: text/html\r\n\r\n5;ext=yes\r\nhello\r\n6\r\n world\r\n0\r\nX-Trailer: yes\r\n\r\n";
        for end in 0..raw.len() {
            assert!(response(&raw[..end], false).unwrap().is_none(), "end {end}");
        }
        assert_eq!(response(raw, false).unwrap().unwrap().body, b"hello world");
        let input = b"<h1>Compressed</h1>";
        let compressed = miniz_oxide::deflate::compress_to_vec(input, 6);
        let mut gzip = alloc::vec![31, 139, 8, 0, 0, 0, 0, 0, 0, 255];
        gzip.extend_from_slice(&compressed);
        gzip.extend_from_slice(&(!crate::gpt::crc32_update(!0, input)).to_le_bytes());
        gzip.extend_from_slice(&(input.len() as u32).to_le_bytes());
        let mut raw = alloc::format!(
            "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\n\r\n",
            gzip.len()
        )
        .into_bytes();
        raw.extend_from_slice(&gzip);
        assert_eq!(response(&raw, false).unwrap().unwrap().body, input);
        let end = raw.len();
        raw[end - 8] ^= 1;
        assert!(response(&raw, false).is_err());
    }
}
