//! Native public web reader. No Gateway configuration, account or device key.
//! One network exchange at a time; cancellation invalidates late TLS replies.
#[path = "browser/html.rs"]
pub mod html;
#[path = "browser/http.rs"]
pub mod http;
#[path = "browser/url.rs"]
pub mod url;
pub use crate::google_gateway::{MailImage, text};
use alloc::{collections::VecDeque, format, string::String, sync::Arc, vec::Vec};
use core::cell::RefCell;
use critical_section::Mutex;
use serde_json::{Value, json};
use url::Url;
const ROOTS: &str = include_str!("browser/cacert.pem");
const PAGE_BLOCKS: usize = 16;
pub struct Cache {
    pub value: Value,
    pub stamp: u64,
}
#[derive(Clone)]
pub struct Snapshot {
    pub busy: bool,
    pub online: bool,
    pub message: &'static str,
}
#[derive(Clone)]
pub enum Purpose {
    Page,
    Search,
    Image(u16),
}
#[derive(Clone)]
pub(crate) struct Request {
    pub id: u32,
    pub url: Url,
    pub purpose: Purpose,
    pub hops: u8,
    pub deadline: u64,
}
struct Saved {
    key: String,
    doc: html::Document,
}
struct State {
    serial: u32,
    revision: u64,
    online: bool,
    pending: Option<Request>,
    active: Option<Request>,
    message: &'static str,
    docs: VecDeque<Saved>,
    pages: Option<Arc<Cache>>,
    search: Option<Arc<Cache>>,
    image: Option<Arc<MailImage>>,
}
static STATE: Mutex<RefCell<State>> = Mutex::new(RefCell::new(State {
    serial: 0,
    revision: 0,
    online: false,
    pending: None,
    active: None,
    message: "Прямое HTTPS · без Gateway",
    docs: VecDeque::new(),
    pages: None,
    search: None,
    image: None,
}));
fn now() -> u64 {
    esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_millis() as u64
}
pub fn snapshot() -> Snapshot {
    critical_section::with(|cs| {
        let s = STATE.borrow_ref(cs);
        Snapshot {
            busy: s.pending.is_some() || s.active.is_some(),
            online: s.online,
            message: s.message,
        }
    })
}
pub fn cache(kind: u8) -> Arc<Cache> {
    critical_section::with(|cs| {
        let s = STATE.borrow_ref(cs);
        let c = if kind == 0 { &s.search } else { &s.pages };
        c.clone().unwrap_or_else(|| {
            Arc::new(Cache {
                value: Value::Null,
                stamp: 0,
            })
        })
    })
}
pub fn mail_image(_: u8) -> Option<Arc<MailImage>> {
    critical_section::with(|cs| STATE.borrow_ref(cs).image.clone())
}
pub fn clear_preview(_: u8) {
    critical_section::with(|cs| STATE.borrow_ref_mut(cs).image = None);
}
pub fn cancel_data(kind: u8) {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        let belongs = |r: &Request| {
            matches!(
                (&r.purpose, kind),
                (Purpose::Search, 0) | (Purpose::Page, 7) | (Purpose::Image(_), 7)
            )
        };
        if s.pending.as_ref().is_some_and(belongs) || s.active.as_ref().is_some_and(belongs) {
            s.serial = s.serial.wrapping_add(1);
            s.pending = None;
            s.active = None;
            s.message = "Загрузка отменена";
        }
    });
}
pub fn clear() {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        s.serial = s.serial.wrapping_add(1);
        s.pending = None;
        s.active = None;
        s.docs = VecDeque::new();
        s.pages = None;
        s.search = None;
        s.image = None;
    });
}
fn page_value(key: &str, doc: &html::Document, page: u16) -> Option<Value> {
    let count = doc.blocks.len().div_ceil(PAGE_BLOCKS).max(1);
    let page = usize::from(page);
    if page >= count {
        return None;
    }
    let blocks: Vec<_> = doc
        .blocks
        .iter()
        .skip(page * PAGE_BLOCKS)
        .take(PAGE_BLOCKS)
        .cloned()
        .collect();
    Some(
        json!({"kind":"browser-page","document":key,"url":doc.url.as_string(),"title":doc.title,"page":page,"page_count":count,"blocks":blocks,"needs_js":doc.needs_js,"truncated":doc.truncated}),
    )
}
fn set_cache(s: &mut State, kind: u8, value: Value) {
    s.revision = s.revision.wrapping_add(1).max(1);
    let c = Some(Arc::new(Cache {
        value,
        stamp: s.revision,
    }));
    if kind == 0 {
        s.search = c;
    } else {
        s.pages = c;
    }
}
/// Preserve the app's existing bounded fetch ABI while replacing the backend.
pub fn fetch(kind: u8, query: &str, id: &str, page: u16, _: &str, _: &str) -> bool {
    let route = (|| -> Result<(Url, Purpose), &'static str> {
        if !crate::file_store::external_access() {
            return Err("Разблокируйте часы");
        }
        if kind == 0 {
            if query.trim().is_empty() || query.len() > 512 {
                return Err("Запрос: до 512 байт");
            }
            return Ok((
                Url::parse(&format!(
                    "https://html.duckduckgo.com/html/?q={}&kl=ru-ru&kp=1",
                    url::encode(query)
                ))?,
                Purpose::Search,
            ));
        }
        if kind != 7 {
            return Err("Неизвестный запрос браузера");
        }
        if !id.is_empty() {
            return critical_section::with(|cs| {
                let mut s = STATE.borrow_ref_mut(cs);
                let doc = s
                    .docs
                    .iter()
                    .find(|d| d.key == id)
                    .ok_or("Страница устарела · обновите")?;
                if query == "image" {
                    let url = doc
                        .doc
                        .images
                        .get(page as usize)
                        .cloned()
                        .ok_or("Нет изображения")?;
                    Ok((url, Purpose::Image(page)))
                } else {
                    let value = page_value(id, &doc.doc, page).ok_or("Нет такой страницы")?;
                    set_cache(&mut s, 7, value);
                    s.message = "Из памяти часов";
                    Err("@cached")
                }
            });
        }
        let value: Value = serde_json::from_str(query).map_err(|_| "Неверный адрес сайта")?;
        if let Some(raw) = value["url"]
            .as_str()
            .or_else(|| value["result_id"].as_str())
        {
            return Ok((Url::address(raw)?, Purpose::Page));
        }
        let key = value["document"].as_str().ok_or("Нет документа")?;
        let index = value["link"].as_u64().ok_or("Нет ссылки")?;
        critical_section::with(|cs| {
            let s = STATE.borrow_ref(cs);
            let d = s
                .docs
                .iter()
                .find(|d| d.key == key)
                .ok_or("Страница устарела · обновите")?;
            let url = d
                .doc
                .links
                .get(index as usize)
                .cloned()
                .ok_or("Нет ссылки")?;
            Ok((url, Purpose::Page))
        })
    })();
    match route {
        Err("@cached") => true,
        Err(error) => {
            critical_section::with(|cs| STATE.borrow_ref_mut(cs).message = error);
            false
        }
        Ok((url, purpose)) => critical_section::with(|cs| {
            let mut s = STATE.borrow_ref_mut(cs);
            if !s.online {
                s.message = "Нет Wi-Fi";
                return false;
            }
            if s.pending.is_some() || s.active.is_some() {
                s.message = "Дождитесь загрузки";
                return false;
            }
            s.serial = s.serial.wrapping_add(1);
            let id = s.serial;
            s.message = "Загрузка напрямую…";
            s.pending = Some(Request {
                id,
                url,
                purpose,
                hops: 0,
                deadline: now() + 35000,
            });
            true
        }),
    }
}
pub(crate) fn network(online: bool) {
    if !crate::file_store::external_access() {
        clear();
    }
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        s.online = online;
        if !online && (s.pending.is_some() || s.active.is_some()) {
            s.serial = s.serial.wrapping_add(1);
            s.pending = None;
            s.active = None;
            s.message = "Нет Wi-Fi · страница в памяти";
        }
    });
}
pub(crate) fn take_request() -> Option<Request> {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if !s.online || s.active.is_some() {
            return None;
        }
        let r = s.pending.take()?;
        s.active = Some(r.clone());
        Some(r)
    })
}
pub(crate) fn current(id: u32) -> bool {
    critical_section::with(|cs| {
        STATE
            .borrow_ref(cs)
            .active
            .as_ref()
            .is_some_and(|r| r.id == id)
    })
}
pub(crate) fn roots() -> &'static str {
    ROOTS
}
pub(crate) fn fail(id: u32, message: &'static str) {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if s.active.as_ref().is_some_and(|r| r.id == id) {
            s.active = None;
            s.message = message;
        }
    });
}
fn decoded_text(bytes: &[u8], content_type: &str) -> Result<String, &'static str> {
    if content_type.contains("charset=windows-1251") || content_type.contains("charset=cp1251") {
        // Cyrillic pages encountered on older Russian-language sites.
        const EXTRA: [char; 64] = [
            'Ђ', 'Ѓ', '‚', 'ѓ', '„', '…', '†', '‡', '€', '‰', 'Љ', '‹', 'Њ', 'Ќ', 'Ћ', 'Џ', 'ђ',
            '‘', '’', '“', '”', '•', '–', '—', '\u{fffd}', '™', 'љ', '›', 'њ', 'ќ', 'ћ', 'џ', ' ',
            'Ў', 'ў', 'Ј', '¤', 'Ґ', '¦', '§', 'Ё', '©', 'Є', '«', '¬', '\u{ad}', '®', 'Ї', '°',
            '±', 'І', 'і', 'ґ', 'µ', '¶', '·', 'ё', '№', 'є', '»', 'ј', 'Ѕ', 'ѕ', 'ї',
        ];
        return Ok(bytes
            .iter()
            .map(|b| match b {
                0..=127 => *b as char,
                128..=191 => EXTRA[(*b - 128) as usize],
                _ => char::from_u32(0x410 + u32::from(*b) - 192).unwrap(),
            })
            .collect());
    }
    if content_type.contains("charset=")
        && !content_type.contains("utf-8")
        && !content_type.contains("utf8")
    {
        return Err("Кодировка сайта не поддерживается");
    }
    Ok(String::from_utf8_lossy(bytes).into_owned())
}
pub(crate) fn complete(id: u32, response: http::Response) {
    let request = critical_section::with(|cs| {
        STATE
            .borrow_ref(cs)
            .active
            .as_ref()
            .filter(|r| r.id == id)
            .cloned()
    });
    let Some(r) = request else {
        return;
    };
    if (300..=399).contains(&response.status) {
        if r.hops >= 5 {
            fail(id, "Слишком много перенаправлений");
            return;
        }
        match r.url.resolve(&response.location) {
            Ok(url) if !response.location.is_empty() => critical_section::with(|cs| {
                let mut s = STATE.borrow_ref_mut(cs);
                if !s.active.as_ref().is_some_and(|r| r.id == id) {
                    return;
                }
                s.active = None;
                s.serial = s.serial.wrapping_add(1);
                let id = s.serial;
                s.pending = Some(Request {
                    id,
                    url,
                    hops: r.hops + 1,
                    ..r
                });
            }),
            _ => fail(id, "Неподдерживаемое перенаправление"),
        };
        return;
    }
    if !matches!(response.status, 200..=299) {
        fail(
            id,
            match response.status {
                403 | 429 if matches!(r.purpose, Purpose::Search) => "DuckDuckGo требует проверку",
                404 => "Страница не найдена",
                403 => "Сайт запретил доступ",
                429 => "Слишком много запросов",
                _ => "Сайт вернул ошибку",
            },
        );
        return;
    }
    if let Purpose::Image(index) = r.purpose {
        if response.truncated {
            fail(id, "Картинка слишком большая");
            return;
        }
        // The gallery's bounded native JPEG/PNG decoders are also used by web images.
        let Some(pixels) = crate::file_store::network_thumbnail(&response.body) else {
            fail(id, "Картинка не поддерживается или больше 4 Мп");
            return;
        };
        let mut small = alloc::vec![0;108*80];
        for y in 0..80 {
            for x in 0..96 {
                small[y * 108 + x + 6] = pixels[(y * 200 / 80) * 240 + x * 240 / 96];
            }
        }
        critical_section::with(|cs| {
            let mut s = STATE.borrow_ref_mut(cs);
            if s.active.as_ref().is_some_and(|r| r.id == id) {
                s.active = None;
                s.image = Some(Arc::new(MailImage {
                    width: 108,
                    height: 80,
                    pixels: small,
                }));
                set_cache(&mut s, 7, json!({"kind":"browser-image","index":index}));
                s.message = "Изображение";
            }
        });
        return;
    }
    if !response.content_type.is_empty()
        && !response.content_type.starts_with("text/html")
        && !response.content_type.starts_with("application/xhtml+xml")
        && !response.content_type.starts_with("text/plain")
    {
        fail(id, "Поддерживаются HTML и текст");
        return;
    }
    let html = match decoded_text(&response.body, &response.content_type) {
        Ok(v) => v,
        Err(e) => {
            fail(id, e);
            return;
        }
    };
    if matches!(r.purpose, Purpose::Search) {
        if response.status == 202 {
            fail(id, "DuckDuckGo требует проверку");
            return;
        }
        let value = html::search(r.url, &html);
        if value["error"].is_string() {
            fail(id, "DuckDuckGo требует проверку");
            return;
        }
        critical_section::with(|cs| {
            let mut s = STATE.borrow_ref_mut(cs);
            if !s.active.as_ref().is_some_and(|r| r.id == id) {
                return;
            }
            s.active = None;
            s.message = "Поиск напрямую";
            set_cache(&mut s, 0, value);
        });
        return;
    }
    let mut doc = if response.content_type.starts_with("text/plain") {
        let escaped = html.replace('&', "&amp;").replace('<', "&lt;");
        html::parse(r.url, &format!("<p>{escaped}</p>"))
    } else {
        html::parse(r.url, &html)
    };
    doc.truncated |= response.truncated;
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if !s.active.as_ref().is_some_and(|r| r.id == id) {
            return;
        }
        s.active = None;
        let key = format!("local-{id}");
        let value = page_value(&key, &doc, 0).unwrap();
        if s.docs.len() >= 4 {
            s.docs.pop_front();
        }
        s.docs.push_back(Saved { key, doc });
        s.message = "Страница на часах";
        set_cache(&mut s, 7, value);
    });
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn setup() {
        crate::file_store::set_external_access(true);
        clear();
        network(true);
    }
    pub fn reset() {
        clear();
    }
    pub fn html_reply(id: u32, body: &str) {
        complete(
            id,
            http::Response {
                status: 200,
                content_type: "text/html; charset=utf-8".into(),
                location: String::new(),
                encoding: String::new(),
                body: body.as_bytes().to_vec(),
                truncated: false,
            },
        );
    }
    #[test]
    fn transparent_web_icons_remain_visible_on_the_preview_background() {
        let pixels = crate::file_store::network_thumbnail(include_bytes!(
            "../../tools/runtime-check/fixtures/transparent-web.png"
        ))
        .unwrap();
        assert_eq!(pixels[99 * 240 + 119], 0); // Opaque black ink.
        assert_eq!(pixels[99 * 240 + 120], 0xffff); // Transparent black becomes white.
        assert_eq!(pixels[0], 0xffff); // Matching preview margin.
    }
    #[test]
    fn pages_links_cancellation_and_redirects_are_local() {
        setup();
        assert!(fetch(
            7,
            r#"{"url":"https://example.org/start"}"#,
            "",
            0,
            "",
            ""
        ));
        let r = take_request().unwrap();
        html_reply(
            r.id,
            "<h1>Заголовок</h1><a href='/next'>Далее</a><img src='/pic.png'>",
        );
        let v = cache(7);
        let doc = v.value["document"].as_str().unwrap();
        assert!(fetch(
            7,
            &format!(r#"{{"document":"{doc}","link":0}}"#),
            "",
            0,
            "",
            ""
        ));
        let r = take_request().unwrap();
        assert_eq!(r.url.path, "/next");
        cancel_data(7);
        html_reply(r.id, "<p>Late response</p>");
        assert_eq!(cache(7).stamp, v.stamp);
        assert!(fetch(7, "", doc, 0, "", ""));
        assert!(take_request().is_none());
        assert!(!snapshot().busy);
        reset();
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    #[test]
    fn redirects_keep_deadline_and_revalidate_local_destinations() {
        tests::setup();
        assert!(fetch(
            7,
            r#"{"url":"https://example.org/a"}"#,
            "",
            0,
            "",
            ""
        ));
        let r = take_request().unwrap();
        complete(
            r.id,
            http::Response {
                status: 302,
                content_type: String::new(),
                encoding: String::new(),
                location: "/b".into(),
                body: Vec::new(),
                truncated: false,
            },
        );
        let next = take_request().unwrap();
        assert_eq!(next.url.path, "/b");
        assert_eq!(next.deadline, r.deadline);
        assert_ne!(next.id, r.id);
        assert!(!current(r.id));
        complete(
            next.id,
            http::Response {
                status: 302,
                content_type: String::new(),
                encoding: String::new(),
                location: "https://127.0.0.1/private".into(),
                body: Vec::new(),
                truncated: false,
            },
        );
        assert!(!snapshot().busy);
        assert!(take_request().is_none());
        tests::reset();
    }
    #[test]
    fn offline_cancellation_and_lock_do_not_publish_late_documents() {
        tests::setup();
        assert!(fetch(
            7,
            r#"{"url":"https://example.org/a"}"#,
            "",
            0,
            "",
            ""
        ));
        let r = take_request().unwrap();
        network(false);
        assert!(!snapshot().busy);
        tests::html_reply(r.id, "<p>Late</p>");
        assert_eq!(cache(7).stamp, 0);
        network(true);
        assert!(fetch(
            7,
            r#"{"url":"https://example.org/a"}"#,
            "",
            0,
            "",
            ""
        ));
        let r = take_request().unwrap();
        crate::file_store::set_external_access(false);
        network(true);
        tests::html_reply(r.id, "<p>Private</p>");
        assert_eq!(cache(7).stamp, 0);
        crate::file_store::set_external_access(true);
        tests::reset();
    }
    #[test]
    fn native_jpeg_and_png_decode_and_release_once() {
        tests::setup();
        for bytes in [
            include_bytes!("../../tools/runtime-check/fixtures/red-web.jpg").as_slice(),
            include_bytes!("../../tools/runtime-check/fixtures/red-web.png").as_slice(),
        ] {
            assert!(fetch(
                7,
                r#"{"url":"https://example.org/a"}"#,
                "",
                0,
                "",
                ""
            ));
            let r = take_request().unwrap();
            tests::html_reply(r.id, "<img src='/image'>");
            let key = cache(7).value["document"].as_str().unwrap().to_string();
            assert!(fetch(7, "image", &key, 0, "", ""));
            let r = take_request().unwrap();
            complete(
                r.id,
                http::Response {
                    status: 200,
                    content_type: "image/png".into(),
                    encoding: String::new(),
                    location: String::new(),
                    body: bytes.to_vec(),
                    truncated: false,
                },
            );
            let picture = mail_image(7).unwrap();
            assert_eq!((picture.width, picture.height), (108, 80));
            assert!(picture.pixels.iter().any(|p| p & 0xf800 != 0));
            clear_preview(7);
            assert!(mail_image(7).is_none());
        }
        tests::reset();
    }
}
