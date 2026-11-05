//! Small HTTPS URL resolver. Credentials, nonstandard ports and local networks
//! never become browser destinations; DNS answers get the same check in net.
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Url {
    pub host: String,
    pub path: String,
}
impl Url {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        if value.len() > 2048 || value.bytes().any(|b| b <= 32 || b == 127 || b == b'\\') {
            return Err("Неверный адрес сайта");
        }
        let raw = value
            .strip_prefix("https://")
            .ok_or("Поддерживается только HTTPS")?;
        let end = raw.find(['/', '?', '#']).unwrap_or(raw.len());
        let authority = &raw[..end];
        if authority.contains('@') || authority.contains('[') {
            return Err("Неверный адрес сайта");
        }
        let host = authority
            .strip_suffix(":443")
            .unwrap_or(authority)
            .to_ascii_lowercase();
        if host.len() > 253
            || host.contains(':')
            || !host.contains('.')
            || host.ends_with('.')
            || host.split('.').any(|s| {
                s.is_empty()
                    || s.len() > 63
                    || s.starts_with('-')
                    || s.ends_with('-')
                    || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
            || host.ends_with(".local")
            || host.ends_with(".localhost")
        {
            return Err("Неверное имя сайта");
        }
        if let Some(ip) = ipv4(&host) {
            if !public_ipv4(ip) {
                return Err("Локальные адреса запрещены");
            }
        }
        // Reject numeric hostname tricks rather than relying on DNS interpretation.
        else if host.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            return Err("Неверный IPv4 адрес");
        }
        let rest = raw[end..].split('#').next().unwrap_or("");
        let path = if rest.is_empty() {
            "/".into()
        } else if rest.starts_with('?') {
            format!("/{rest}")
        } else {
            rest.into()
        };
        Ok(Self { host, path })
    }
    pub fn address(value: &str) -> Result<Self, &'static str> {
        let value = value.trim();
        if value.contains("://") {
            Self::parse(value)
        } else {
            Self::parse(&format!("https://{value}"))
        }
    }
    pub fn as_string(&self) -> String {
        format!("https://{}{}", self.host, self.path)
    }
    pub fn resolve(&self, href: &str) -> Result<Self, &'static str> {
        let href = href.trim();
        if href.starts_with("https://") {
            return Self::parse(href);
        }
        if href.starts_with("//") {
            return Self::parse(&format!("https:{href}"));
        }
        if href
            .find(':')
            .is_some_and(|colon| href.find('/').is_none_or(|slash| colon < slash))
        {
            return Err("Неподдерживаемая ссылка");
        }
        if href.starts_with('#') || href.is_empty() {
            return Ok(self.clone());
        }
        let path = if href.starts_with('/') {
            href.to_string()
        } else if href.starts_with('?') {
            format!("{}{}", self.path.split('?').next().unwrap_or("/"), href)
        } else {
            let base = self.path.split('?').next().unwrap_or("/");
            let dir = &base[..base.rfind('/').unwrap_or(0) + 1];
            format!("{dir}{href}")
        };
        let path = path.split('#').next().unwrap_or("");
        let (path, query) = path
            .split_once('?')
            .map_or((path, None), |(p, q)| (p, Some(q)));
        let mut pieces = Vec::new();
        for piece in path.split('/') {
            match piece {
                "" | "." => {}
                ".." => {
                    pieces.pop();
                }
                _ => pieces.push(piece),
            }
        }
        let mut normalized = format!("/{}", pieces.join("/"));
        if path.ends_with('/') && !normalized.ends_with('/') {
            normalized.push('/');
        }
        if let Some(q) = query {
            normalized.push('?');
            normalized.push_str(q);
        }
        // Encode non-ASCII path bytes and spaces, keeping existing escapes intact.
        let mut encoded = String::new();
        for b in normalized.bytes() {
            if b <= 32 || b >= 127 {
                encoded.push_str(&format!("%{b:02X}"));
            } else {
                encoded.push(b as char);
            }
        }
        Self::parse(&format!("https://{}{encoded}", self.host))
    }
}
pub fn ipv4(host: &str) -> Option<[u8; 4]> {
    let mut out = [0; 4];
    let mut parts = host.split('.');
    for b in &mut out {
        *b = parts.next()?.parse().ok()?;
    }
    if parts.next().is_some() {
        None
    } else {
        Some(out)
    }
}
pub fn public_ipv4([a, b, c, _]: [u8; 4]) -> bool {
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && (b == 168 || b == 0 || b == 2))
        || (a == 100 && (64..=127).contains(&b))
        || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && c == 113))
}
pub fn encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char)
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
pub fn decode(value: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let s = value.as_bytes();
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'%' {
            let h = core::str::from_utf8(s.get(i + 1..i + 3)?).ok()?;
            bytes.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            bytes.push(if s[i] == b'+' { b' ' } else { s[i] });
            i += 1;
        }
    }
    String::from_utf8(bytes).ok()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_queries_and_private_destinations() {
        let u = Url::parse("https://example.org/a/b?q=1").unwrap();
        assert_eq!(
            u.resolve("../c?x=2&amp=3#anchor").unwrap().as_string(),
            "https://example.org/c?x=2&amp=3"
        );
        assert_eq!(u.resolve("?a=1").unwrap().path, "/a/b?a=1");
        for s in [
            "http://example.org/",
            "https://user@example.org/",
            "https://127.0.0.1/",
            "https://192.168.1.1/",
            "https://example.org:8443/",
            "https://a.local/",
        ] {
            assert!(Url::parse(s).is_err(), "{s}");
        }
        assert!(u.resolve("javascript:alert(1)").is_err());
        assert!(u.resolve("mailto:a@b.com").is_err());
        assert!(u.resolve("//10.0.0.1/").is_err());
        assert_eq!(
            u.resolve("../поиск").unwrap().path,
            "/%D0%BF%D0%BE%D0%B8%D1%81%D0%BA"
        );
    }
}

/// A DoH response is still untrusted input. Match the question and accept only
/// public A records; the resolved IP is pinned while TLS validates the hostname.
pub fn dns_json(host: &str, bytes: &[u8]) -> Result<[u8; 4], &'static str> {
    if bytes.len() > 16384 {
        return Err("Ответ DNS слишком большой");
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| "Неверный ответ DNS")?;
    if value["Status"] != 0
        || value["TC"] == true
        || !value["Question"][0]["name"]
            .as_str()
            .is_some_and(|q| q.trim_end_matches('.').eq_ignore_ascii_case(host))
    {
        return Err("DNS не нашёл сайт");
    }
    value["Answer"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .filter(|r| r["type"] == 1)
                .filter_map(|r| r["data"].as_str().and_then(ipv4))
                .find(|ip| public_ipv4(*ip))
        })
        .ok_or("DNS не вернул публичный IPv4")
}
#[cfg(test)]
mod dns_tests {
    use super::*;
    #[test]
    fn doh_matches_question_and_rejects_lan_and_truncated_answers() {
        let good=br#"{"Status":0,"Question":[{"name":"example.org."}],"Answer":[{"type":1,"data":"10.0.0.1"},{"type":1,"data":"93.184.215.14"}]}"#;
        assert_eq!(dns_json("example.org", good).unwrap(), [93, 184, 215, 14]);
        assert!(dns_json("wrong.org", good).is_err());
        assert!(dns_json("example.org",br#"{"Status":0,"Question":[{"name":"example.org"}],"Answer":[{"type":1,"data":"127.0.0.1"}]}"#).is_err());
    }
}
