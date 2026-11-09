//! HTML reader for the watch. Bounded semantic blocks, not a DOM or script VM.
use super::url::{Url, decode};
use alloc::{format, string::String, vec::Vec};
use serde_json::{Value, json};
const MAX_BLOCKS: usize = 256;
const MAX_TEXT: usize = 48 * 1024;
const MAX_LINKS: usize = 64;
const URL_BUDGET: usize = 16 * 1024;
pub struct Document {
    pub url: Url,
    pub title: String,
    pub blocks: Vec<Value>,
    pub links: Vec<Url>,
    pub images: Vec<Url>,
    pub needs_js: bool,
    pub truncated: bool,
}
fn limit(s: &str, n: usize) -> String {
    s.chars()
        .scan(0, |len, c| {
            *len += c.len_utf8();
            (*len <= n).then_some(c)
        })
        .collect()
}
fn entity(s: &str) -> Option<char> {
    match s {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" | "#39" => Some('\''),
        "nbsp" | "thinsp" | "ensp" | "emsp" => Some(' '),
        "bull" => Some('*'),
        "middot" => Some('·'),
        "raquo" | "rarr" => Some('>'),
        "laquo" | "larr" => Some('<'),
        "ndash" => Some('–'),
        "mdash" => Some('—'),
        "hellip" => Some('…'),
        "copy" => Some('©'),
        _ => {
            let code = if let Some(n) = s.strip_prefix("#x").or_else(|| s.strip_prefix("#X")) {
                u32::from_str_radix(n, 16).ok()?
            } else {
                s.strip_prefix('#')?.parse().ok()?
            };
            char::from_u32(code).filter(|c| *c != '\0')
        }
    }
}
pub fn text(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        if let Some(end) = tail.find(';').filter(|e| *e <= 16) {
            if let Some(c) = entity(&tail[..end]) {
                out.push(c);
                rest = &tail[end + 1..];
                continue;
            }
        }
        out.push('&');
        rest = tail;
    }
    out.push_str(rest);
    out
}
fn tag_end(s: &str) -> Option<usize> {
    let mut quote = 0;
    for (i, b) in s.bytes().enumerate() {
        if quote != 0 {
            if b == quote {
                quote = 0;
            }
        } else if b == b'\'' || b == b'"' {
            quote = b;
        } else if b == b'>' {
            return Some(i);
        }
    }
    None
}
fn attrs(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut s = raw.trim_start();
    while !s.is_empty() && out.len() < 32 {
        let n = s
            .find(|c: char| c.is_ascii_whitespace() || c == '=' || c == '/')
            .unwrap_or(s.len());
        if n == 0 {
            s = &s[1..];
            continue;
        }
        let key = s[..n].to_ascii_lowercase();
        s = s[n..].trim_start();
        let mut value = String::new();
        if let Some(rest) = s.strip_prefix('=') {
            s = rest.trim_start();
            if let Some(q) = s.chars().next().filter(|c| *c == '\'' || *c == '"') {
                s = &s[1..];
                let n = s.find(q).unwrap_or(s.len());
                value = text(&s[..n]);
                s = &s[(n + usize::from(n < s.len()))..];
            } else {
                let n = s.find(char::is_whitespace).unwrap_or(s.len());
                value = text(&s[..n]);
                s = &s[n..];
            }
        }
        out.push((key, limit(&value, 2048)));
        s = s.trim_start();
    }
    out
}
fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}
fn visible(attrs: &[(String, String)]) -> bool {
    !attrs.iter().any(|(k, _)| k == "hidden")
        && attr(attrs, "aria-hidden") != Some("true")
        && !attr(attrs, "style").is_some_and(|s| {
            let s = s.replace(' ', "").to_ascii_lowercase();
            s.contains("display:none") || s.contains("visibility:hidden")
        })
}
fn heading_level(tag: &str) -> Option<u8> {
    match tag.as_bytes() {
        [b'h', level @ b'1'..=b'6'] => Some(level - b'0'),
        _ => None,
    }
}
fn boundary(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "div"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "nav"
            | "main"
            | "ul"
            | "ol"
            | "li"
            | "br"
            | "tr"
            | "pre"
            | "blockquote"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
    )
}
fn void(tag: &str) -> bool {
    matches!(
        tag,
        "br" | "hr"
            | "img"
            | "meta"
            | "link"
            | "input"
            | "source"
            | "wbr"
            | "base"
            | "area"
            | "embed"
            | "param"
            | "col"
            | "track"
    )
}
fn flush(
    doc: &mut Document,
    buf: &mut String,
    heading: u8,
    link: Option<usize>,
    bold: bool,
    total: &mut usize,
    flow: u32,
) {
    let source = core::mem::take(buf);
    let mut normal = String::new();
    for word in source.split_whitespace() {
        if !normal.is_empty() {
            normal.push(' ');
        }
        normal.push_str(word);
    }
    if normal.is_empty()
        || normal
            .chars()
            .all(|c| c.is_whitespace() || "|[]•".contains(c))
    {
        return;
    }
    let mut rest = normal.as_str();
    while !rest.is_empty() {
        if doc.blocks.len() >= MAX_BLOCKS || *total >= MAX_TEXT {
            doc.truncated = true;
            return;
        }
        let part = limit(rest, (MAX_TEXT - *total).min(512));
        if part.is_empty() {
            doc.truncated = true;
            return;
        }
        *total += part.len();
        let index = link.unwrap_or(doc.blocks.len());
        doc.blocks.push(json!({"index":index,"type":if link.is_some(){"link"}else if heading>0{"heading"}else{"paragraph"},"text":part,"level":heading,"bold":bold,"flow":flow}));
        rest = &rest[part.len()..];
    }
}
pub fn parse(url: Url, html: &str) -> Document {
    let mut doc = Document {
        title: url.host.clone(),
        url: url.clone(),
        blocks: Vec::new(),
        links: Vec::new(),
        images: Vec::new(),
        needs_js: false,
        truncated: false,
    };
    let (mut base, mut base_seen, mut buf, mut heading, mut bold, mut link, mut total) =
        (url, false, String::new(), 0u8, false, None, 0usize);
    let (mut stack, mut hidden_depth, mut head, mut title, mut scripts) =
        (Vec::<(String, bool)>::new(), 0usize, false, false, 0usize);
    let mut flow = 0u32;
    let mut list_starts = Vec::<usize>::new();
    let (mut at, mut operations, mut url_bytes) = (0usize, 0usize, 0usize);
    while at < html.len() && operations < 16000 && doc.blocks.len() < MAX_BLOCKS && total < MAX_TEXT
    {
        operations += 1;
        if html[at..].starts_with("<!--") {
            at = html[at + 4..]
                .find("-->")
                .map_or(html.len(), |i| at + 4 + i + 3);
            continue;
        }
        if html.as_bytes()[at] != b'<' {
            let end = html[at..].find('<').map_or(html.len(), |i| at + i);
            if hidden_depth == 0 && (!head || title) {
                if title {
                    let value = text(&limit(&html[at..end], 256));
                    if !value.trim().is_empty() {
                        doc.title = limit(value.trim(), 120);
                    }
                } else {
                    let mut rest = &html[at..end];
                    while !rest.is_empty() && total < MAX_TEXT && doc.blocks.len() < MAX_BLOCKS {
                        let mut n = rest.len().min(1024);
                        while !rest.is_char_boundary(n) {
                            n -= 1;
                        }
                        if n < rest.len() {
                            if let Some(amp) = rest[..n]
                                .rfind('&')
                                .filter(|a| n - a <= 16 && !rest[*a..n].contains(';'))
                            {
                                if amp > 0 {
                                    n = amp;
                                }
                            }
                        }
                        buf.push_str(&text(&rest[..n]));
                        rest = &rest[n..];
                        if buf.len() > 2048 {
                            flush(&mut doc, &mut buf, heading, link, bold, &mut total, flow);
                        }
                    }
                    if !rest.is_empty() {
                        doc.truncated = true;
                    }
                }
            }
            at = end;
            continue;
        }
        let Some(n) = tag_end(&html[at + 1..]) else {
            break;
        };
        let raw = html[at + 1..at + 1 + n].trim();
        at += n + 2;
        if raw.starts_with(['!', '?']) {
            continue;
        }
        let closing = raw.starts_with('/');
        let raw = raw.trim_start_matches('/').trim_start();
        let n = raw
            .find(|c: char| c.is_ascii_whitespace() || c == '/')
            .unwrap_or(raw.len());
        let tag = raw[..n].to_ascii_lowercase();
        let attributes = attrs(&raw[n..]);
        if closing {
            if tag == "head" {
                head = false;
            }
            if tag == "title" {
                title = false;
            }
            if hidden_depth == 0 && boundary(&tag) {
                flush(&mut doc, &mut buf, heading, link, bold, &mut total, flow);
                flow = flow.wrapping_add(1);
            }
            if tag == "li" && hidden_depth == 0 {
                if let Some(start) = list_starts.pop() {
                    if let Some(block) = doc.blocks.get_mut(start) {
                        block["list"] = json!(true);
                    }
                }
            }
            if let Some(i) = stack.iter().rposition(|(t, _)| *t == tag) {
                for (_, hidden) in stack.drain(i..) {
                    if hidden {
                        hidden_depth = hidden_depth.saturating_sub(1);
                    }
                }
            }
            if tag == "a" {
                flush(&mut doc, &mut buf, heading, link, bold, &mut total, flow);
                link = None;
            }
            if heading_level(&tag).is_some() {
                heading = 0;
            }
            if matches!(tag.as_str(), "b" | "strong") {
                bold = false;
            }
            continue;
        }
        if matches!(
            tag.as_str(),
            "script" | "style" | "noscript" | "svg" | "template"
        ) {
            if tag == "script" {
                scripts += 1;
            }
            // Raw text can contain '<' and '>' which are not HTML tags.
            let needle = format!("</{tag}");
            let tail = &html[at..];
            at = tail
                .as_bytes()
                .windows(needle.len())
                .position(|part| part.eq_ignore_ascii_case(needle.as_bytes()))
                .map_or(html.len(), |i| at + i);
            continue;
        }
        if tag == "head" {
            head = true;
        }
        if tag == "title" {
            title = true;
        }
        if !void(&tag) && !raw.ends_with('/') {
            if stack.len() >= 32 {
                doc.truncated = true;
                break;
            }
            let hidden = !visible(&attributes);
            if hidden {
                hidden_depth += 1;
            }
            stack.push((tag.clone(), hidden));
        }
        if hidden_depth > 0 || !visible(&attributes) {
            continue;
        }
        if tag == "base" && !base_seen {
            if let Some(url) = attr(&attributes, "href").and_then(|v| base.resolve(v).ok()) {
                base = url;
                base_seen = true;
            }
        }
        if head {
            continue;
        }
        if boundary(&tag) || matches!(tag.as_str(), "td" | "th") {
            flush(&mut doc, &mut buf, heading, link, bold, &mut total, flow);
            flow = flow.wrapping_add(1);
        }
        if let Some(level) = heading_level(&tag) {
            heading = level;
        }
        if tag == "li" {
            list_starts.push(doc.blocks.len());
        }
        if matches!(tag.as_str(), "b" | "strong") {
            bold = true;
        }
        if tag == "a" {
            flush(&mut doc, &mut buf, heading, link, bold, &mut total, flow);
            link = None;
            if doc.links.len() < MAX_LINKS {
                if let Some(url) = attr(&attributes, "href").and_then(|v| base.resolve(v).ok()) {
                    let size = url.as_string().len();
                    if url_bytes + size <= URL_BUDGET {
                        url_bytes += size;
                        link = Some(doc.links.len());
                        doc.links.push(url);
                    } else {
                        doc.truncated = true;
                    }
                }
            }
        }
        if tag == "img" && doc.images.len() < 32 {
            flush(&mut doc, &mut buf, heading, link, bold, &mut total, flow);
            if let Some(url) = attr(&attributes, "data-src")
                .or_else(|| attr(&attributes, "src"))
                .and_then(|v| base.resolve(v).ok())
            {
                let size = url.as_string().len();
                if url_bytes + size > URL_BUDGET {
                    doc.truncated = true;
                    continue;
                }
                url_bytes += size;
                let index = doc.images.len();
                doc.images.push(url);
                doc.blocks.push(json!({"type":"image","index":index,"text":limit(attr(&attributes,"alt").unwrap_or("Изображение"),96)}));
            }
        }
        if tag == "hr" {
            flush(&mut doc, &mut buf, heading, link, bold, &mut total, flow);
            doc.blocks
                .push(json!({"type":"rule","index":doc.blocks.len(),"text":""}));
        }
    }
    flush(&mut doc, &mut buf, heading, link, bold, &mut total, flow);
    doc.needs_js = scripts > 0 && total < 80;
    doc.truncated |= at < html.len();
    doc
}
pub fn search(url: Url, html: &str) -> Value {
    if html.contains("challenge-form") || html.contains("anomaly.js") {
        return json!({"error":"SEARCH_VERIFICATION_REQUIRED"});
    }
    let doc = parse(url, html);
    let mut results = Vec::<Value>::new();
    for block in &doc.blocks {
        if block["type"] == "link" {
            let Some(index) = block["index"].as_u64() else {
                continue;
            };
            let Some(url) = doc.links.get(index as usize) else {
                continue;
            };
            let mut target = url.clone();
            if target.host.ends_with("duckduckgo.com") {
                let encoded = target
                    .path
                    .split('?')
                    .nth(1)
                    .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("uddg=")));
                let Some(url) = encoded.and_then(decode).and_then(|v| Url::parse(&v).ok()) else {
                    continue;
                };
                target = url;
            }
            if results.iter().any(|v| v["url"] == target.as_string()) {
                continue;
            }
            if results.len() >= 10 {
                break;
            }
            results.push(json!({"id":target.as_string(),"url":target.as_string(),"title":block["text"],"snippet":""}));
        } else if let Some(last) = results.last_mut() {
            if last["snippet"] == "" && block["type"] == "paragraph" {
                last["snippet"] = block["text"].clone();
            }
        }
    }
    json!({"provider":"DuckDuckGo","results":results,"truncated":doc.truncated})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn horizontal_rule_does_not_turn_following_body_text_into_a_heading() {
        let doc = parse(
            Url::parse("https://example.org/").unwrap(),
            "<h2>Title</h2><hr><p>Body &bull; text</p>",
        );
        assert_eq!(doc.blocks[0]["type"], "heading");
        assert_eq!(doc.blocks[1]["type"], "rule");
        assert_eq!(doc.blocks[2]["type"], "paragraph");
        assert_eq!(doc.blocks[2]["text"], "Body * text");
    }
    #[test]
    fn inline_sentences_lists_and_table_cells_have_real_boundaries() {
        let doc = parse(
            Url::parse("https://example.org/").unwrap(),
            "<p>Hello <a href='/next'>world</a>!</p><ul><li><a href='/a'>Item</a></li></ul><table><tr><td>Name</td><td><a href='/b'>Value</a></td></tr></table>",
        );
        assert_eq!(doc.blocks[0]["flow"], doc.blocks[1]["flow"]);
        assert_eq!(doc.blocks[1]["flow"], doc.blocks[2]["flow"]);
        assert_ne!(doc.blocks[2]["flow"], doc.blocks[3]["flow"]);
        assert_eq!(doc.blocks[3]["text"], "Item");
        assert_eq!(doc.blocks[3]["list"], true);
        assert_ne!(doc.blocks[4]["flow"], doc.blocks[5]["flow"]);
        assert!(
            doc.blocks
                .iter()
                .all(|b| !matches!(b["text"].as_str(), Some("|" | "•" | "][")))
        );
    }
    #[test]
    fn semantics_links_base_entities_hidden_and_scripts() {
        let d = parse(
            Url::parse("https://example.org/start").unwrap(),
            "<head><title>A &amp; Б</title><base href='/docs/'></head><h1>Hi</h1><p>Текст <a href='../next?x=1&amp;y=2'>далее</a></p><div hidden><p>secret</p></div><script>if(a<b){}</script><img src='photo.png' alt='Фото'>",
        );
        assert_eq!(d.title, "A & Б");
        assert_eq!(d.links[0].as_string(), "https://example.org/next?x=1&y=2");
        assert_eq!(
            d.images[0].as_string(),
            "https://example.org/docs/photo.png"
        );
        assert!(
            !d.blocks
                .iter()
                .any(|b| b["text"].as_str().unwrap_or("").contains("secret"))
        );
        assert_eq!(text("&#x1f600; &lt; &#1055;"), "😀 < П");
    }
    #[test]
    fn scripts_angles_and_text_limits() {
        let h = format!(
            "<script>const s='<p>bad</p>';</script><p>{}</p>",
            "д".repeat(100000)
        );
        let d = parse(Url::parse("https://example.org/").unwrap(), &h);
        assert!(d.truncated);
        assert!(d.blocks.len() <= MAX_BLOCKS);
        assert!(
            d.blocks
                .iter()
                .map(|b| b["text"].as_str().unwrap().len())
                .sum::<usize>()
                <= MAX_TEXT
        );
    }
}
