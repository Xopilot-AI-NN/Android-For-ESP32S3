//! Общий менеджер Google: UI ставит запросы, сетевой сервис выполняет HTTPS.
//! OAuth secret/access/refresh tokens сюда не передаются. В ZPager — только ключ устройства.
use alloc::{collections::VecDeque, format, string::String, sync::Arc, vec::Vec};
use core::cell::RefCell;
use critical_section::Mutex;
use serde_json::{Value, json};
#[path = "mail_image.rs"]
mod image;
pub use image::Preview as MailImage;
pub const CONFIG_TAG: u32 = 0x4753_5731;
pub const CONFIG_SLOTS: [u32; 2] = [76, 77];
const LOGOUT_SLOTS: [u32; 2] = [78, 79];
const LOGOUT_TAG: u32 = 0x4753_4f31;
pub const RESPONSE_LIMIT: usize = 24 * 1024;
/// Намеренно без Debug: диагностический вывод не должен раскрывать credential.
pub(crate) struct Configuration {
    pub base: String,
    pub host: String,
    pub port: u16,
    pub device: String,
    pub credential: String,
    pub ca: String,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Unconfigured,
    Disconnected,
    Creating,
    Waiting,
    Confirm,
    Connected,
    Expired,
    Denied,
    Reauth,
    Error,
}
#[derive(Clone)]
pub struct Snapshot {
    pub phase: Phase,
    pub email: String,
    pub services: u8,
    pub qr: String,
    pub message: &'static str,
    pub busy: bool,
    pub revision: u32,
    pub online: bool,
    pub last_sync: String,
    pub account_epoch: u32,
}
#[derive(Clone)]
pub struct Cache {
    pub value: Value,
    pub stamp: u64,
}
#[derive(Clone)]
pub enum Operation {
    Pair(u8),
    Poll,
    Confirm,
    Renew,
    Me,
    Sync,
    Data(u8, String, Vec<u8>),
    Logout,
    Cancel,
}
pub(crate) struct Request {
    pub id: u32,
    pub config: Arc<Configuration>,
    pub method: &'static str,
    pub path: String,
    pub body: Vec<u8>,
    pub authorization: String,
}
struct State {
    config: Option<Arc<Configuration>>,
    loaded: bool,
    phase: Phase,
    email: String,
    services: u8,
    identity: String,
    pending_email: String,
    pending_services: u8,
    qr: String,
    pair: String,
    expires: u64,
    next_poll: u64,
    session: String,
    session_until: u64,
    queue: VecDeque<Operation>,
    active: Option<(u32, Operation)>,
    serial: u32,
    revision: u32,
    message: &'static str,
    caches: [Option<Arc<Cache>>; 8],
    image: Option<Arc<MailImage>>,
    image_kind: u8,
    online: bool,
    last_sync: String,
    account_epoch: u32,
    logout_marker: u8,
    marker_dirty: bool,
    next_unlink: u64,
}
impl State {
    const fn new() -> Self {
        Self {
            config: None,
            loaded: false,
            phase: Phase::Unconfigured,
            email: String::new(),
            services: 0,
            identity: String::new(),
            pending_email: String::new(),
            pending_services: 0,
            qr: String::new(),
            pair: String::new(),
            expires: 0,
            next_poll: 0,
            session: String::new(),
            session_until: 0,
            queue: VecDeque::new(),
            active: None,
            serial: 1,
            revision: 0,
            message: "Настройте Gateway",
            // Readers clone an Arc instead of the entire JSON tree each frame.
            caches: [const { None }; 8],
            image: None,
            image_kind: 0,
            online: false,
            last_sync: String::new(),
            account_epoch: 0,
            logout_marker: 0,
            marker_dirty: false,
            next_unlink: 0,
        }
    }
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    fn forget(&mut self) {
        self.image = None;
        self.account_epoch = self.account_epoch.wrapping_add(1);
        self.session.clear();
        self.session_until = 0;
        self.email.clear();
        self.identity.clear();
        self.pending_email.clear();
        self.pending_services = 0;
        self.services = 0;
        self.caches = core::array::from_fn(|_| None);
        self.last_sync.clear();
    }
}
static STATE: Mutex<RefCell<State>> = Mutex::new(RefCell::new(State::new()));
fn now() -> u64 {
    esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_millis()
}
pub fn text(v: &Value, key: &str, limit: usize) -> String {
    v[key].as_str().unwrap_or("").chars().take(limit).collect()
}
fn opaque(v: &str) -> bool {
    (16..=128).contains(&v.len())
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
fn encode(v: &str) -> String {
    use core::fmt::Write;
    let mut s = String::new();
    for b in v.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            s.push(b as char);
        } else {
            let _ = write!(s, "%{b:02X}");
        }
    }
    s
}
/// Только HTTPS origin: путь, userinfo, fragment и переключатель insecure отсутствуют.
fn origin(base: &str) -> Option<(String, u16)> {
    if base.len() > 150 {
        return None;
    }
    let a = base.strip_prefix("https://")?;
    if a.is_empty() || a.contains(['/', '@', '?', '#']) {
        return None;
    }
    let (h, p) = match a.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().ok()?),
        None => (a, 443),
    };
    if p == 0
        || h.is_empty()
        || h.len() > 120
        || h.starts_with('.')
        || h.ends_with('.')
        || !h
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
    {
        return None;
    }
    Some((h.to_ascii_lowercase(), p))
}
pub(crate) fn parse_config(bytes: &[u8]) -> Option<Configuration> {
    if bytes.len() > 4080 {
        return None;
    }
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if v["version"] != 1 || v.get("client_secret").is_some() || v.get("refresh_token").is_some() {
        return None;
    }
    let base = text(&v, "base_url", 151);
    let (host, port) = origin(&base)?;
    let device = text(&v, "device_id", 129);
    let credential = text(&v, "device_credential", 129);
    let ca = text(&v, "ca_pem", 3001);
    if !opaque(&device)
        || !opaque(&credential)
        || ca.len() > 3000
        || ca.contains('\0')
        || !ca.contains("-----BEGIN CERTIFICATE-----")
        || !ca.contains("-----END CERTIFICATE-----")
    {
        return None;
    }
    Some(Configuration {
        base,
        host,
        port,
        device,
        credential,
        ca,
        generation: v["generation"].as_u64()?,
    })
}
/// Из двух резервных страниц выбирается последняя целая конфигурация. Через Files она недоступна.
#[inline(never)]
pub fn service_storage<D: embedded_sdmmc::BlockDevice>(dev: &D) {
    let (loaded, dirty, marker) = critical_section::with(|cs| {
        let s = STATE.borrow_ref(cs);
        (s.loaded, s.marker_dirty, s.logout_marker)
    });
    if loaded {
        if dirty {
            if let Ok(p) = crate::page_store::PageStore::open(dev) {
                let mut saved = true;
                for slot in LOGOUT_SLOTS {
                    saved &= p.write(dev, slot, LOGOUT_TAG, &[marker]).is_ok();
                }
                if saved {
                    critical_section::with(|cs| {
                        let mut s = STATE.borrow_ref_mut(cs);
                        if s.logout_marker == marker {
                            s.marker_dirty = false;
                        }
                    });
                }
            }
        }
        return;
    }
    let mut best: Option<Configuration> = None;
    let mut logout_marker = 0;
    if let Ok(p) = crate::page_store::PageStore::open(dev) {
        // Either copy requesting sign-out wins over an active copy. Interrupted re-login
        // may require confirmation again, but cannot revive a signed-out account.
        for slot in LOGOUT_SLOTS {
            let mut b = [0; 1];
            let candidate = match p.read(dev, slot, LOGOUT_TAG, &mut b) {
                Ok(1) if b[0] <= 2 => b[0],
                Err(crate::page_store::PageStoreError::Empty) => 0,
                _ => 2,
            };
            if candidate == 1 || candidate == 2 && logout_marker != 1 {
                logout_marker = candidate;
            }
        }
        for slot in CONFIG_SLOTS {
            let mut b = alloc::vec![0; 4080];
            if let Ok(n) = p.read(dev, slot, CONFIG_TAG, &mut b) {
                if let Some(c) = parse_config(&b[..n]) {
                    if best
                        .as_ref()
                        .is_none_or(|old| c.generation > old.generation)
                    {
                        best = Some(c);
                    }
                }
            }
        }
    }
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        s.loaded = true;
        s.logout_marker = logout_marker;
        if let Some(c) = best {
            s.config = Some(Arc::new(c));
            s.phase = Phase::Disconnected;
            s.message = "Войдите в Google";
        }
        s.changed();
    });
}
pub fn snapshot() -> Snapshot {
    critical_section::with(|cs| {
        let s = STATE.borrow_ref(cs);
        Snapshot {
            phase: s.phase,
            email: if s.phase == Phase::Confirm {
                s.pending_email.clone()
            } else {
                s.email.clone()
            },
            services: if s.phase == Phase::Confirm {
                s.pending_services
            } else {
                s.services
            },
            qr: s.qr.clone(),
            message: s.message,
            busy: s.active.is_some() || !s.queue.is_empty(),
            revision: s.revision,
            online: s.online,
            last_sync: s.last_sync.clone(),
            account_epoch: s.account_epoch,
        }
    })
}
/// Кэш только RAM; содержимое писем не записывается в общедоступные документы.
/// Invalidate only this app's request; a late network response must not reopen a closed page.
pub fn cancel_data(kind: u8) {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        s.queue
            .retain(|op| !matches!(op, Operation::Data(k, _, _) if *k == kind));
        if matches!(s.active.as_ref(), Some((_, Operation::Data(k, _, _))) if *k == kind) {
            s.active = None;
            s.serial = s.serial.wrapping_add(1);
        }
        s.changed();
    });
}
pub fn clear_preview(kind: u8) {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if s.image_kind == kind {
            s.image = None;
        }
    });
}
pub fn mail_image(kind: u8) -> Option<Arc<MailImage>> {
    critical_section::with(|cs| {
        let s = STATE.borrow_ref(cs);
        if s.image_kind == kind {
            s.image.clone()
        } else {
            None
        }
    })
}
/// A snapshot shares immutable JSON with the service. Replacing or clearing a
/// cache never changes a view that is already being painted.
pub fn cache(kind: u8) -> Arc<Cache> {
    let cached = if crate::file_store::external_access() {
        critical_section::with(|cs| STATE.borrow_ref(cs).caches[usize::from(kind.min(7))].clone())
    } else {
        None
    };
    cached.unwrap_or_else(|| {
        Arc::new(Cache {
            value: Value::Null,
            stamp: 0,
        })
    })
}

pub fn request(op: Operation) -> bool {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if !crate::file_store::external_access() {
            s.message = "Часы заблокированы";
            return false;
        }
        if s.config.is_none() {
            s.message = "Импортируйте настройки Gateway";
            return false;
        }
        if matches!(op, Operation::Logout) {
            s.active = None;
            s.queue.clear();
            s.serial = s.serial.wrapping_add(1);
            s.forget();
            s.qr.clear();
            s.pair.clear();
            s.phase = Phase::Disconnected;
            s.logout_marker = 1;
            s.marker_dirty = true;
            s.next_unlink = 0;
            s.message = "Выход: ожидается связь с Gateway";
            s.changed();
            return true;
        }
        if !s.online {
            s.message = "Нет Wi-Fi · доступен кэш";
            s.changed();
            return false;
        }
        if s.logout_marker == 1 {
            s.message = "Сначала завершится выход";
            return false;
        }
        if matches!(
            op,
            Operation::Renew
                | Operation::Me
                | Operation::Sync
                | Operation::Data(1 | 2 | 4 | 5, _, _)
        ) && s.logout_marker != 0
        {
            s.message = "Войдите в Google";
            return false;
        }
        if matches!(op, Operation::Confirm) && (s.phase != Phase::Confirm || now() >= s.expires) {
            return false;
        }
        if s.active.is_some() || !s.queue.is_empty() {
            s.message = "Дождитесь завершения";
            return false;
        }
        if let Operation::Pair(mask) = op {
            let mask = (mask | s.services) & 15;
            s.phase = Phase::Creating;
            s.pending_email.clear();
            s.pending_services = 0;
            s.qr.clear();
            s.pair.clear();
            s.queue.push_back(Operation::Pair(mask));
        } else {
            if matches!(op, Operation::Logout) {
                s.forget();
                s.phase = Phase::Disconnected;
            }
            if let Operation::Data(kind, ref path, _) = op {
                if kind == 7 {
                    s.caches[7] = None;
                    s.image = None;
                }
                if path.contains("/messages/") || path.contains("/search/pages/") {
                    if let Some(cache) = s.caches[kind.min(7) as usize].as_mut() {
                        // Usually unique; copy only when an older snapshot is
                        // still held by a reader while a request is started.
                        if let Some(value) = Arc::make_mut(cache).value.as_object_mut() {
                            value.remove("body");
                            value.remove("has_more");
                        }
                    }
                }
            }
            s.queue.push_back(op);
        }
        s.message = "Запрос к Gateway…";
        s.changed();
        true
    })
}
/// Сериализация URL/JSON централизована: приложения не задают адрес удалённого сервера.
pub fn fetch(kind: u8, query: &str, id: &str, page: u16, source: &str, target: &str) -> bool {
    let (path, body) = match kind {
        7 if id.is_empty() => ("/api/v1/browser/open".into(), query.as_bytes().to_vec()),
        7 if query == "image" => (
            format!("/api/v1/browser/documents/{}/images/{page}", encode(id)),
            Vec::new(),
        ),
        7 => (
            format!("/api/v1/browser/documents/{}?page={page}", encode(id)),
            Vec::new(),
        ),
        0 if id.is_empty() => (format!("/api/v1/search?q={}", encode(query)), Vec::new()),
        0 => (
            format!("/api/v1/search/pages/{}?page={page}", encode(id)),
            Vec::new(),
        ),
        1 => {
            let (month, token) = query.split_once('|').unwrap_or((query, ""));
            (
                format!(
                    "/api/v1/google/calendar/events?limit=10&month={}&page={}",
                    encode(month),
                    encode(token)
                ),
                Vec::new(),
            )
        }
        2 if id.is_empty() => (
            format!(
                "/api/v1/google/gmail/messages?limit=10&page={}",
                encode(query)
            ),
            Vec::new(),
        ),
        2 => (
            format!("/api/v1/google/gmail/messages/{}?page={page}", encode(id)),
            Vec::new(),
        ),
        4 => (
            format!("/api/v1/google/contacts?page={}", encode(query)),
            Vec::new(),
        ),
        5 if id.is_empty() && query == "choose" => {
            ("/api/v1/google/photos/picker".into(), b"{}".to_vec())
        }
        5 if id.is_empty() => (
            format!("/api/v1/google/photos?page={}", encode(query)),
            Vec::new(),
        ),
        5 => (
            format!("/api/v1/google/photos/items/{}", encode(id)),
            Vec::new(),
        ),
        3 => (
            "/api/v1/translate".into(),
            serde_json::to_vec(&json!({"text":query,"source":source,"target":target}))
                .unwrap_or_default(),
        ),
        _ => return false,
    };
    request(Operation::Data(kind, path, body))
}
/// Поздний ответ отменённой привязки не может создать локальную сессию.
pub fn cancel_pairing() {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        s.serial = s.serial.wrapping_add(1);
        s.active = None;
        s.queue.clear();
        s.qr.clear();
        s.pending_email.clear();
        s.pending_services = 0;
        if !s.pair.is_empty() {
            s.queue.push_back(Operation::Cancel);
        }
        s.phase = if s.session.is_empty() {
            Phase::Disconnected
        } else {
            Phase::Connected
        };
        s.message = "Вход отменён";
        s.changed();
    });
}
pub(crate) fn current(id: u32) -> bool {
    critical_section::with(|cs| {
        STATE
            .borrow_ref(cs)
            .active
            .as_ref()
            .is_some_and(|v| v.0 == id)
    }) && crate::file_store::external_access()
}
pub(crate) fn network(online: bool) {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if s.online != online {
            s.online = online;
            s.changed();
        }
        if !crate::file_store::external_access() {
            if s.active.is_some() || !s.qr.is_empty() || !s.queue.is_empty() {
                s.active = None;
                s.queue.clear();
                s.qr.clear();
                s.pair.clear();
                s.phase = if s.session.is_empty() {
                    Phase::Disconnected
                } else {
                    Phase::Connected
                };
                s.changed();
            }
            return;
        }
        if matches!(s.phase, Phase::Waiting | Phase::Confirm) && now() >= s.expires {
            s.active = None;
            s.phase = if s.session.is_empty() {
                Phase::Expired
            } else {
                Phase::Connected
            };
            s.pending_email.clear();
            s.pending_services = 0;
            s.qr.clear();
            s.queue.clear();
            s.pair.clear();
            s.message = "QR истёк. Повторите вход";
            s.changed();
        }
        if !online && !s.queue.is_empty() {
            s.queue.clear();
            s.message = "Нет Wi-Fi · доступен кэш";
            if s.phase == Phase::Creating {
                s.phase = Phase::Error;
            }
            s.changed();
        }
        if online
            && s.logout_marker == 1
            && s.active.is_none()
            && s.queue.is_empty()
            && now() >= s.next_unlink
        {
            s.queue.push_back(Operation::Logout);
            s.next_unlink = now() + 30_000;
        }
        if online
            && s.phase == Phase::Waiting
            && s.active.is_none()
            && s.queue.is_empty()
            && now() >= s.next_poll
        {
            s.queue.push_back(Operation::Poll);
            s.next_poll = now() + 3000;
        }
    });
}
/// Сессия обновляется ключом устройства после перезагрузки, без хранения Google-токенов на часах.
pub(crate) fn take_request() -> Option<Request> {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if s.active.is_some() || !s.online || !crate::file_store::external_access() {
            return None;
        }
        let config = s.config.clone()?;
        let op = s.queue.pop_front()?;
        let session = matches!(
            op,
            Operation::Me | Operation::Sync | Operation::Data(1 | 2 | 4 | 5, _, _)
        );
        let op = if session && (s.session.is_empty() || now() + 30_000 >= s.session_until) {
            s.queue.push_front(op);
            Operation::Renew
        } else {
            op
        };
        let mut method = "GET";
        let mut body = Vec::new();
        let path = match &op {
            Operation::Pair(mask) => {
                method = "POST";
                let mut services = Vec::new();
                if mask & 1 != 0 {
                    services.push("calendar");
                }
                if mask & 2 != 0 {
                    services.push("gmail");
                }
                if mask & 4 != 0 {
                    services.push("contacts");
                }
                if mask & 8 != 0 {
                    services.push("photos");
                }
                body = serde_json::to_vec(&json!({"services":services})).ok()?;
                "/api/v1/google/pairings".into()
            }
            Operation::Poll => format!("/api/v1/google/pairings/{}", s.pair),
            Operation::Confirm => {
                method = "POST";
                body = b"{}".to_vec();
                format!("/api/v1/google/pairings/{}/confirm", s.pair)
            }
            Operation::Cancel => {
                method = "DELETE";
                let p = format!("/api/v1/google/pairings/{}", s.pair);
                s.pair.clear();
                p
            }
            Operation::Renew => {
                method = "POST";
                body = b"{}".to_vec();
                "/api/v1/google/session".into()
            }
            Operation::Me => "/api/v1/google/me".into(),
            Operation::Sync => {
                method = "POST";
                body = b"{}".to_vec();
                "/api/v1/google/sync".into()
            }
            Operation::Data(_, path, data) => {
                body = data.clone();
                if !body.is_empty() {
                    method = "POST";
                }
                path.clone()
            }
            Operation::Logout => {
                method = "DELETE";
                "/api/v1/google/account".into()
            }
        };
        let device = !matches!(
            op,
            Operation::Me | Operation::Sync | Operation::Data(1 | 2 | 4 | 5, _, _)
        );
        let authorization = if device {
            format!("Device {}", config.credential)
        } else {
            format!("Bearer {}", s.session)
        };
        s.serial = s.serial.wrapping_add(1);
        let id = s.serial;
        s.active = Some((id, op));
        Some(Request {
            id,
            config,
            method,
            path,
            body,
            authorization,
        })
    })
}
fn mask(v: &Value) -> u8 {
    v.as_array().map_or(0, |a| {
        a.iter().fold(0, |n, s| {
            n | match s.as_str() {
                Some("calendar") => 1,
                Some("gmail") => 2,
                Some("contacts") => 4,
                Some("photos") => 8,
                _ => 0,
            }
        })
    })
}
fn masked_email(v: &str) -> String {
    let Some((name, domain)) = v.rsplit_once('@') else {
        return "Google".into();
    };
    format!(
        "{}***@{}",
        name.chars().next().unwrap_or('*'),
        domain.chars().take(60).collect::<String>()
    )
}
/// Ответ ограничен до разбора; ID защищает от ответов старой привязки после отмены/выхода.
pub(crate) fn complete(id: u32, status: u16, bytes: &[u8]) {
    let mut value = if bytes.len() <= RESPONSE_LIMIT {
        serde_json::from_slice::<Value>(bytes).ok()
    } else {
        None
    };
    let preview = value.as_ref().and_then(|v| image::decode(&v["image"]));
    if let Some(v) = value.as_mut() {
        if v.get("image").is_some() && preview.is_none() {
            if let Some(object) = v.as_object_mut() {
                object.remove("image");
            }
            v["image_unavailable"] = Value::Bool(true);
        }
        if let Some(image) = v.get_mut("image").and_then(Value::as_object_mut) {
            image.remove("data"); // JSON snapshots share only metadata; pixels have one Arc owner.
        }
    }
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if !s.active.as_ref().is_some_and(|a| a.0 == id) {
            return;
        }
        let (_, op) = s.active.take().unwrap();
        let Some(v) = value else {
            s.message = "Некорректный ответ Gateway";
            s.phase = Phase::Error;
            s.queue.clear();
            s.changed();
            return;
        };
        if !(200..300).contains(&status) {
            s.message = match v["error"].as_str() {
                Some("BROWSER_PAGE_EXPIRED") => "Страница устарела · обновите",
                Some("BROWSER_URL_INVALID") => "Нужен адрес HTTPS-сайта",
                Some("PAGE_NOT_ALLOWED") => "Адрес недоступен браузеру",
                Some("PAGE_NOT_READABLE") => "Формат сайта не поддерживается",
                Some("SEARCH_VERIFICATION_REQUIRED") => "DuckDuckGo просит проверку",
                Some("SCOPE_MISSING") => "Разрешите доступ к сервису",
                Some("MAIL_RENDER_UNAVAILABLE") => "Проверьте Python на Gateway",
                Some("GOOGLE_API_DISABLED") => "Включите API в Google Cloud",
                Some("PROVIDER_NOT_CONFIGURED") => "Настройте ключ API на Gateway",
                Some("RATE_LIMITED" | "BUDGET_EXCEEDED") => "Лимит API. Повторите позже",
                Some("REAUTH_REQUIRED" | "UNAUTHORIZED") => "Повторите вход в Google",
                Some("PAIRING_EXPIRED") => "QR истёк",
                Some("PAGE_EXPIRED") => "Обновите результаты поиска",
                _ => "Ошибка Gateway",
            };
            if status == 401 {
                s.forget();
                s.phase = Phase::Reauth;
            } else if status == 410
                && matches!(
                    op,
                    Operation::Pair(_) | Operation::Poll | Operation::Confirm
                )
            {
                s.phase = Phase::Expired;
                s.qr.clear();
            } else if matches!(
                op,
                Operation::Pair(_) | Operation::Poll | Operation::Confirm
            ) {
                s.phase = Phase::Error;
            }
            s.queue.clear();
            if v["error"] == "SCOPE_MISSING"
                && !s.session.is_empty()
                && matches!(op, Operation::Data(_, _, _) | Operation::Sync)
            {
                s.image = None;
                s.account_epoch = s.account_epoch.wrapping_add(1);
                s.caches = core::array::from_fn(|_| None);
                s.queue.push_back(Operation::Me);
            }
            s.changed();
            return;
        }
        match op {
            Operation::Pair(_) => {
                let pair = text(&v, "pairing_id", 129);
                let qr = text(&v, "qr_url", 220);
                if opaque(&pair)
                    && s.config.as_ref().is_some_and(|c| {
                        qr.strip_suffix(&format!("/p/{pair}"))
                            .and_then(origin)
                            .is_some_and(|(host, port)| host == c.host && port == c.port)
                    })
                {
                    s.pair = pair;
                    s.qr = qr;
                    s.expires = now() + v["expires_in"].as_u64().unwrap_or(300).min(300) * 1000;
                    s.next_poll = now() + 3000;
                    s.phase = Phase::Waiting;
                    s.message = "Сканируйте QR телефоном";
                } else {
                    s.phase = Phase::Error;
                    s.message = "Неверная ссылка входа";
                }
            }
            Operation::Poll => match v["status"].as_str() {
                Some("awaiting_watch_confirmation") => {
                    s.pending_email = masked_email(v["account"]["email"].as_str().unwrap_or(""));
                    s.pending_services = mask(&v["granted_services"]);
                    s.phase = Phase::Confirm;
                    s.message = "Проверьте аккаунт и подтвердите";
                }
                Some("denied" | "cancelled") => {
                    s.phase = if s.session.is_empty() {
                        Phase::Denied
                    } else {
                        Phase::Connected
                    };
                    s.pending_email.clear();
                    s.pending_services = 0;
                    s.qr.clear();
                    s.message = "Вход отменён";
                }
                _ => {}
            },
            Operation::Confirm | Operation::Renew => {
                let token = text(&v, "session", 129);
                if opaque(&token) {
                    if matches!(op, Operation::Confirm) {
                        let email = s.pending_email.clone();
                        let services = s.pending_services;
                        s.forget();
                        s.email = email;
                        s.services = services;
                    }
                    if matches!(op, Operation::Confirm) {
                        s.logout_marker = 0;
                        s.marker_dirty = true;
                    }
                    s.session = token;
                    s.session_until =
                        now() + v["expires_in"].as_u64().unwrap_or(3600).min(3600) * 1000;
                    s.qr.clear();
                    s.pair.clear();
                    s.phase = Phase::Connected;
                    s.message = "Google подключён";
                    s.queue.push_front(Operation::Me);
                } else {
                    s.phase = Phase::Error;
                    s.message = "Неверная сессия";
                    s.queue.clear();
                }
            }
            Operation::Me => {
                let identity = text(&v["account"], "sub", 255);
                if !s.identity.is_empty() && s.identity != identity {
                    s.image = None;
                    s.account_epoch = s.account_epoch.wrapping_add(1);
                    s.caches = core::array::from_fn(|_| None);
                }
                s.identity = identity;
                s.email = masked_email(v["account"]["email"].as_str().unwrap_or(""));
                s.services = mask(&v["granted_services"]);
                s.last_sync = text(&v, "last_sync", 40);
                s.message = "Google подключён";
            }
            Operation::Sync => {
                for (kind, key) in [(1, "calendar"), (2, "gmail")] {
                    if v.get(key).is_some() {
                        s.caches[kind] = Some(Arc::new(Cache {
                            value: v[key].clone(),
                            stamp: now(),
                        }));
                    }
                }
                s.last_sync = text(&v, "synced_at", 40);
                s.message = "Синхронизация завершена";
            }
            Operation::Data(kind, _, _) => {
                if matches!(kind, 2 | 5 | 7) {
                    s.image = preview;
                    s.image_kind = kind;
                }
                let mut v = v;
                if kind == 1 || kind == 2 {
                    s.last_sync = text(&v, "synced_at", 40);
                }
                if v.get("body").is_some() {
                    if let Some(existing) = s.caches[kind.min(7) as usize]
                        .as_ref()
                        .and_then(|cache| cache.value.as_object())
                    {
                        for key in [
                            "results",
                            "messages",
                            "events",
                            "contacts",
                            "photos",
                            "next_page",
                            "page_token",
                            "provider",
                        ] {
                            if let Some(value) = existing.get(key) {
                                v[key] = value.clone();
                            }
                        }
                    }
                }
                s.caches[kind.min(7) as usize] = Some(Arc::new(Cache {
                    value: v,
                    stamp: now(),
                }));
                s.message = "Данные обновлены";
            }
            Operation::Logout => {
                s.forget();
                s.logout_marker = 2;
                s.marker_dirty = true;
                s.phase = Phase::Disconnected;
                s.message = "Аккаунт отключён";
            }
            Operation::Cancel => {}
        }
        s.changed();
    });
}
pub(crate) fn fail(id: u32, message: &'static str) {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if s.active.as_ref().is_some_and(|a| a.0 == id) {
            s.active = None;
            s.queue.clear();
            s.message = message;
            if s.phase == Phase::Creating {
                s.phase = Phase::Error;
            }
            s.changed();
        }
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn reset() {
        critical_section::with(|cs| *STATE.borrow_ref_mut(cs) = State::new());
        crate::file_store::set_external_access(false);
    }
    pub(crate) fn setup() {
        reset();
        crate::time::set_ms(1000);
        crate::file_store::set_external_access(true);
        let config=parse_config(&serde_json::to_vec(&json!({"version":1,"generation":1,"base_url":"https://gateway.example.org","device_id":"d".repeat(22),"device_credential":"k".repeat(43),"ca_pem":"-----BEGIN CERTIFICATE-----\nfixture\n-----END CERTIFICATE-----"})).unwrap()).unwrap();
        critical_section::with(|cs| {
            let mut s = STATE.borrow_ref_mut(cs);
            s.config = Some(Arc::new(config));
            s.loaded = true;
            s.phase = Phase::Disconnected;
        });
        network(true);
    }
    fn reply(id: u32, value: Value) {
        complete(id, 200, &serde_json::to_vec(&value).unwrap());
    }
    fn pairing() -> String {
        assert!(request(Operation::Pair(3)));
        let r = take_request().unwrap();
        assert!(r.authorization.starts_with("Device "));
        let pair = "p".repeat(22);
        reply(
            r.id,
            json!({"pairing_id":pair,"qr_url":format!("https://gateway.example.org/p/{pair}"),"expires_in":300}),
        );
        pair
    }
    pub(crate) fn connected() {
        setup();
        pairing();
        confirm();
    }
    fn confirm() {
        crate::time::set_ms(4001);
        network(true);
        let r = take_request().unwrap();
        reply(
            r.id,
            json!({"status":"awaiting_watch_confirmation","account":{"email":"fixture@example.invalid"},"granted_services":["calendar","gmail"]}),
        );
        assert_eq!(snapshot().phase, Phase::Confirm);
        assert!(take_request().is_none());
        assert!(request(Operation::Confirm));
        let r = take_request().unwrap();
        reply(r.id, json!({"session":"s".repeat(43),"expires_in":3600}));
        let r = take_request().unwrap();
        assert!(r.authorization.starts_with("Bearer "));
        reply(
            r.id,
            json!({"account":{"email":"fixture@example.invalid"},"granted_services":["calendar","gmail"]}),
        );
    }
    #[test]
    fn cache_snapshots_share_json_and_survive_service_replacement() {
        connected();
        assert!(fetch(2, "", "", 0, "", ""));
        let r = take_request().unwrap();
        reply(
            r.id,
            json!({"messages":[],"body":"old preview","page_token":""}),
        );
        let old = cache(2);
        assert!(Arc::ptr_eq(&old, &cache(2)));
        // Starting a detail request removes the service's old body, while a
        // renderer holding a snapshot must keep seeing a consistent document.
        assert!(fetch(2, "", "another-message", 0, "", ""));
        assert_eq!(old.value["body"], "old preview");
        assert!(cache(2).value.get("body").is_none());
        let r = take_request().unwrap();
        reply(r.id, json!({"body":"new preview"}));
        assert_eq!(cache(2).value["body"], "new preview");
        assert_eq!(old.value["body"], "old preview");
        reset();
    }

    #[test]
    fn compressed_mail_preview_renders_real_pixels_and_is_cleared_on_logout() {
        connected();
        assert!(fetch(2, "", "mail-preview", 0, "", ""));
        let r = take_request().unwrap();
        reply(
            r.id,
            json!({"id":"mail-preview","body":"Fixture image","image":{
            "codec":"rgb565-zlib","width":2,"height":1,"data":"eJxj+PGAHQAEswHg"}}),
        );
        let preview = mail_image(2).unwrap();
        assert_eq!(preview.pixels, [0xf800, 0x07e0]);
        assert!(mail_image(5).is_none());
        assert!(cache(2).value["image"].get("data").is_none());
        let mut frame =
            crate::material::MaterialFrame::parse("M3|1|0|70|0|0|0|0|1|1|0|0|600|0|0|0|1|1|3")
                .unwrap();
        frame.app.id = crate::apps::GMAIL;
        frame.app.remote.kind = 2;
        frame.app.remote.mode = 4;
        frame.app.remote.image = true;
        let mut surface = crate::material::Surface::new();
        surface.render(frame);
        assert_eq!(surface.pixels()[163 * 240 + 118], 0xf800);
        assert_eq!(surface.pixels()[163 * 240 + 120], 0x07e0);
        assert!(request(Operation::Logout));
        assert!(mail_image(2).is_none());
        reset();
    }
    #[test]
    fn real_contract_requires_confirmation_preserves_mail_list_and_logs_out_offline() {
        setup();
        pairing();
        assert_eq!(snapshot().phase, Phase::Waiting);
        confirm();
        assert_eq!(snapshot().phase, Phase::Connected);
        assert_eq!(snapshot().services, 3);
        assert!(fetch(2, "", "", 0, "", ""));
        let r = take_request().unwrap();
        reply(
            r.id,
            json!({"messages":[{"id":"mail-one","subject":"Fixture"}],"next_page":"next"}),
        );
        assert!(fetch(2, "", "mail-one", 0, "", ""));
        let r = take_request().unwrap();
        reply(r.id, json!({"body":"actual text","has_more":false}));
        assert_eq!(cache(2).value["messages"][0]["subject"], "Fixture");
        assert_eq!(cache(2).value["body"], "actual text");
        network(false);
        assert!(request(Operation::Logout));
        assert!(cache(2).value.is_null());
        assert_eq!(snapshot().phase, Phase::Disconnected);
        assert!(!request(Operation::Renew));
        network(true);
        let r = take_request().unwrap();
        assert_eq!(r.method, "DELETE");
        assert!(r.authorization.starts_with("Device "));
        reply(r.id, json!({"status":"not_connected"}));
        assert!(!request(Operation::Renew));
        reset();
    }
    #[test]
    fn cancelled_expired_and_locked_pairings_cannot_be_resurrected_by_late_replies() {
        setup();
        pairing();
        crate::time::set_ms(4001);
        network(true);
        let r = take_request().unwrap();
        cancel_pairing();
        reply(
            r.id,
            json!({"status":"awaiting_watch_confirmation","account":{"email":"other@example.invalid"}}),
        );
        assert_eq!(snapshot().phase, Phase::Disconnected);
        reset();
        setup();
        pairing();
        crate::time::set_ms(301001);
        network(true);
        assert_eq!(snapshot().phase, Phase::Expired);
        assert!(snapshot().qr.is_empty());
        reset();
        setup();
        pairing();
        crate::file_store::set_external_access(false);
        network(true);
        assert!(snapshot().qr.is_empty());
        assert!(cache(2).value.is_null());
        reset();
    }
    #[test]
    fn bad_origins_and_google_oauth_material_are_rejected() {
        assert!(origin("http://gateway.example.org").is_none());
        assert!(origin("https://name:password@example.org").is_none());
        assert!(origin("https://example.org/path").is_none());
        assert!(origin("https://example.org:8443").is_some());
        assert!(parse_config(br#"{"version":1,"client_secret":"fixture"}"#).is_none());
    }
    #[test]
    fn a_cold_watch_renews_by_device_credential_before_loading_personal_data() {
        setup();
        assert!(fetch(1, "2026-11", "", 0, "", ""));
        let r = take_request().unwrap();
        assert_eq!(r.path, "/api/v1/google/session");
        assert!(r.authorization.starts_with("Device "));
        reply(r.id, json!({"session":"s".repeat(43),"expires_in":3600}));
        let me = take_request().unwrap();
        assert_eq!(me.path, "/api/v1/google/me");
        reply(
            me.id,
            json!({"account":{"email":"fixture@example.invalid"},"granted_services":["calendar"]}),
        );
        let r = take_request().unwrap();
        assert!(r.path.contains("month=2026-11"));
        assert!(r.authorization.starts_with("Bearer "));
        reset();
    }
    #[test]
    fn cancelling_a_different_pending_identity_keeps_the_original_account() {
        setup();
        pairing();
        confirm();
        let old = snapshot().email;
        pairing();
        crate::time::set_ms(8000);
        network(true);
        let r = take_request().unwrap();
        reply(
            r.id,
            json!({"status":"awaiting_watch_confirmation","account":{"email":"other@different.invalid"},"granted_services":["calendar"]}),
        );
        assert_ne!(snapshot().email, old);
        cancel_pairing();
        assert_eq!(snapshot().email, old);
        assert_eq!(snapshot().services, 3);
        reset();
    }
    #[test]
    #[ignore = "requires WATCH_TEST_IMAGE; all writes are an in-memory overlay"]
    fn configuration_fallback_and_offline_logout_survive_a_cold_restore() {
        use embedded_sdmmc::{Block, BlockCount, BlockIdx};
        use std::{
            collections::BTreeMap,
            fs::File,
            io::{Read as _, Seek as _, SeekFrom},
        };
        struct Image {
            file: RefCell<File>,
            writes: RefCell<BTreeMap<u32, [u8; 512]>>,
        }
        impl embedded_sdmmc::BlockDevice for Image {
            type Error = std::io::Error;
            fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
                Ok(BlockCount(
                    (self.file.borrow().metadata()?.len() / 512) as u32,
                ))
            }
            fn read(&self, blocks: &mut [Block], index: BlockIdx) -> Result<(), Self::Error> {
                let mut f = self.file.borrow_mut();
                for (i, b) in blocks.iter_mut().enumerate() {
                    let at = index.0 + i as u32;
                    if let Some(data) = self.writes.borrow().get(&at) {
                        b.contents = *data;
                    } else {
                        f.seek(SeekFrom::Start(u64::from(at) * 512))?;
                        f.read_exact(&mut b.contents)?;
                    }
                }
                Ok(())
            }
            fn write(&self, blocks: &[Block], index: BlockIdx) -> Result<(), Self::Error> {
                for (i, b) in blocks.iter().enumerate() {
                    self.writes
                        .borrow_mut()
                        .insert(index.0 + i as u32, b.contents);
                }
                Ok(())
            }
        }
        let path = std::env::var("WATCH_TEST_IMAGE").expect("provide the test image path");
        let disk = Image {
            file: RefCell::new(File::open(path).unwrap()),
            writes: RefCell::new(BTreeMap::new()),
        };
        let pager = crate::page_store::PageStore::open(&disk).unwrap();
        for (i, slot) in CONFIG_SLOTS.into_iter().enumerate() {
            let data=serde_json::to_vec(&json!({"version":1,"generation":i+1,"base_url":"https://gateway.example.org","device_id":"d".repeat(22),"device_credential":"k".repeat(43),"ca_pem":"-----BEGIN CERTIFICATE-----\nfixture\n-----END CERTIFICATE-----"})).unwrap();
            pager.write(&disk, slot, CONFIG_TAG, &data).unwrap();
        }
        reset();
        service_storage(&disk);
        assert_eq!(
            critical_section::with(|cs| STATE.borrow_ref(cs).config.as_ref().unwrap().generation),
            2
        );
        let part = crate::gpt::find_partition(&disk, "swap").unwrap();
        crate::gpt::write_partition(
            &disk,
            part,
            (u64::from(CONFIG_SLOTS[1]) + 1) * 4096 + 12,
            &[0; 4],
        )
        .unwrap();
        reset();
        service_storage(&disk);
        assert_eq!(
            critical_section::with(|cs| STATE.borrow_ref(cs).config.as_ref().unwrap().generation),
            1
        );
        crate::file_store::set_external_access(true);
        assert!(request(Operation::Logout));
        service_storage(&disk);
        reset();
        service_storage(&disk);
        crate::file_store::set_external_access(true);
        network(true);
        let r = take_request().unwrap();
        assert_eq!(r.method, "DELETE");
        reply(r.id, json!({"status":"not_connected"}));
        service_storage(&disk);
        reset();
        service_storage(&disk);
        crate::file_store::set_external_access(true);
        network(true);
        assert!(!request(Operation::Renew));
        reset();
    }
}
