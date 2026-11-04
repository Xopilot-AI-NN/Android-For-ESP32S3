//! Bounded HTTP/1.1 response framing. Redirects are never followed and compressed bodies are rejected.
use alloc::vec::Vec;
pub const LIMIT: usize = 24 * 1024;
/// None means more TLS plaintext is needed; malformed framing fails closed.
pub fn response(bytes: &[u8]) -> Result<Option<(u16, Vec<u8>)>, &'static str> {
    if bytes.len() > LIMIT + 4096 {
        return Err("Ответ Gateway слишком большой");
    }
    let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
        if bytes.len() > 4096 {
            return Err("Заголовки Gateway слишком большие");
        }
        return Ok(None);
    };
    if end > 4096 {
        return Err("Заголовки Gateway слишком большие");
    }
    let headers = core::str::from_utf8(&bytes[..end]).map_err(|_| "Неверные заголовки Gateway")?;
    let mut lines = headers.split("\r\n");
    let mut status = lines.next().ok_or("Нет HTTP статуса")?.split_whitespace();
    if status.next() != Some("HTTP/1.1") {
        return Err("Требуется HTTP/1.1");
    }
    let status = status
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("Неверный HTTP статус")?;
    let (mut length, mut chunked) = (None, false);
    for line in lines {
        let (k, v) = line.split_once(':').ok_or("Неверный HTTP заголовок")?;
        let v = v.trim();
        if k.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err("Повторный Content-Length");
            }
            let n = v.parse::<usize>().map_err(|_| "Неверная длина ответа")?;
            if n > LIMIT {
                return Err("Ответ слишком большой");
            }
            length = Some(n);
        }
        if k.eq_ignore_ascii_case("transfer-encoding") {
            if chunked || !v.eq_ignore_ascii_case("chunked") {
                return Err("Неверное кодирование ответа");
            }
            chunked = true;
        }
        if k.eq_ignore_ascii_case("content-encoding") && !v.eq_ignore_ascii_case("identity") {
            return Err("Сжатый ответ не поддерживается");
        }
    }
    let data = &bytes[end + 4..];
    if chunked {
        if length.is_some() {
            return Err("Неоднозначная длина ответа");
        }
        // TLS can deliver one HTTP chunk across many reads. Validate the complete
        // framing first so an incomplete response does not allocate and copy
        // the same body again on every read.
        let mut decoded_len = 0usize;
        let mut at = 0;
        loop {
            let Some(line) = data[at..].windows(2).position(|w| w == b"\r\n") else {
                return Ok(None);
            };
            if line > 32 {
                return Err("Неверный chunk");
            }
            let size = core::str::from_utf8(&data[at..at + line])
                .ok()
                .and_then(|s| usize::from_str_radix(s.split(';').next().unwrap_or(""), 16).ok())
                .ok_or("Неверный chunk")?;
            at += line + 2;
            if size == 0 {
                if data.len() < at + 2 {
                    return Ok(None);
                }
                if &data[at..at + 2] != b"\r\n" {
                    return Err("HTTP trailers не поддерживаются");
                }
                if data.len() != at + 2 {
                    return Err("Лишние данные после ответа");
                }
                break;
            }
            if size > LIMIT - decoded_len {
                return Err("Ответ слишком большой");
            }
            if data.len() < at + size + 2 {
                return Ok(None);
            }
            decoded_len += size;
            at += size;
            if &data[at..at + 2] != b"\r\n" {
                return Err("Неверная граница chunk");
            }
            at += 2;
        }
        let mut out = Vec::new();
        out.try_reserve_exact(decoded_len)
            .map_err(|_| "Недостаточно памяти для ответа Gateway")?;
        at = 0;
        loop {
            // These boundaries were checked above against the same immutable
            // input; only copy after the terminating chunk has arrived.
            let line = data[at..]
                .windows(2)
                .position(|w| w == b"\r\n")
                .ok_or("Неверный chunk")?;
            let size = core::str::from_utf8(&data[at..at + line])
                .ok()
                .and_then(|s| usize::from_str_radix(s.split(';').next().unwrap_or(""), 16).ok())
                .ok_or("Неверный chunk")?;
            at += line + 2;
            if size == 0 {
                return Ok(Some((status, out)));
            }
            out.extend_from_slice(&data[at..at + size]);
            at += size + 2;
        }
    }
    let length = length.ok_or("Gateway не указал длину ответа")?;
    if data.len() < length {
        Ok(None)
    } else if data.len() == length {
        let mut body = Vec::new();
        body.try_reserve_exact(length)
            .map_err(|_| "Недостаточно памяти для ответа Gateway")?;
        body.extend_from_slice(data);
        Ok(Some((status, body)))
    } else {
        Err("Лишние данные после ответа")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragments_and_chunked_utf8_are_complete_before_parsing_json() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n";
        for n in 0..raw.len() {
            assert!(response(&raw[..n]).unwrap().is_none());
        }
        assert_eq!(response(raw).unwrap().unwrap(), (200, b"{}".to_vec()));
    }
    #[test]
    fn multiple_chunks_extensions_and_every_fragment_boundary() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3;part=1\r\nabc\r\n2\r\nde\r\n0\r\n\r\n";
        for end in 0..raw.len() {
            assert!(response(&raw[..end]).unwrap().is_none(), "fragment {end}");
        }
        assert_eq!(response(raw).unwrap(), Some((200, b"abcde".to_vec())));
        let mut extra = raw.to_vec();
        extra.push(b'x');
        assert!(response(&extra).is_err());
    }

    #[test]
    fn chunked_body_limit_is_cumulative() {
        let mut raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n6000\r\n".to_vec();
        raw.extend(core::iter::repeat_n(b'a', LIMIT));
        raw.extend_from_slice(b"\r\n0\r\n\r\n");
        assert_eq!(response(&raw).unwrap().unwrap().1.len(), LIMIT);
        raw.truncate(raw.len() - 5);
        raw.extend_from_slice(b"1\r\nb\r\n0\r\n\r\n");
        assert!(response(&raw).is_err());
    }

    #[test]
    fn ambiguous_truncated_and_oversized_framing_is_rejected() {
        assert!(
            response(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n{}")
                .unwrap()
                .is_none()
        );
        assert!(
            response(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nTransfer-Encoding: chunked\r\n\r\n")
                .is_err()
        );
        assert!(response(b"HTTP/1.1 200 OK\r\nContent-Length: 999999\r\n\r\n").is_err());
    }
}
