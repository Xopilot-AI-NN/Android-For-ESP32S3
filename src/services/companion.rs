// Модуль подтверждённого моста KDE Connect.
//! Explicitly approved KDE desktop bridge. This is not a native KDE TLS client.
use crate::runtime::RuntimeEvent;
use alloc::string::{String, ToString};
use core::{
    cell::RefCell,
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering},
};
use critical_section::Mutex;
/// Разрешение команд моста от текущего состояния блокировки.
static ALLOWED: AtomicBool = AtomicBool::new(false);
/// Рабочее состояние компонента компонента подтверждённого моста KDE Connect.
struct State {
    /// Идентификатор подтверждённой сессии моста.
    session: String,
    /// Кандидат на подключение до подтверждения на часах.
    candidate: String,
    /// Имя компонента, устройства или записи.
    name: String,
    /// Монотонное начало ожидания подтверждения моста.
    pending_since: u64,
    /// Время последнего успешного обновления данных.
    updated: u64,
    /// Признак доступности сетевого соединения.
    online: bool,
    /// Индекс текущей цели навигации.
    cursor: u8,
    /// Последовательный идентификатор операции/записи.
    sequence: u32,
    /// Ожидающее действие пользователя.
    action: u8,
    /// Ещё не выполненный запрос ресурса.
    requested: u64,
    /// Текстовое содержимое в ограниченном буфере.
    text: String,
    /// Сообщение или его код для вывода пользователю.
    message: &'static str,
}
/// Общее состояние сервиса, доступ к которому защищён critical_section.
static STATE: Mutex<RefCell<State>> = Mutex::new(RefCell::new(State {
    session: String::new(),
    candidate: String::new(),
    name: String::new(),
    pending_since: 0,
    updated: 0,
    online: false,
    cursor: 0,
    sequence: 0,
    action: 0,
    requested: 0,
    text: String::new(),
    message: "Мост не подключён",
}));
/// Возвращает текущий отсчёт монотонных часов.
fn now() -> u64 {
    esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_secs()
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Снимок данных для отображения компонента подтверждённого моста KDE Connect.
pub struct View {
    /// Признак подтверждённого сопряжения с мостом.
    pub paired: bool,
    /// Необработанный запрос или ожидающий результат.
    pub pending: bool,
    /// Признак доступности сетевого соединения.
    pub online: bool,
    /// Индекс текущей цели навигации.
    pub cursor: u8,
    /// Имя компонента, устройства или записи.
    pub name: [u8; 24],
    /// Число значащих байтов буфера name.
    pub name_len: u8,
    /// Код погоды или подтверждения текущего протокола.
    pub code: [u8; 4],
    /// Сообщение или его код для вывода пользователю.
    pub message: &'static str,
}
impl Default for View {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            paired: false,
            pending: false,
            online: false,
            cursor: 0,
            name: [0; 24],
            name_len: 0,
            code: [b'-'; 4],
            message: "Мост не подключён",
        }
    }
}
/// Обновляет разрешение доступа моста; блокировка отменяет ожидающие команды.
pub fn set_allowed(allowed: bool) {
    ALLOWED.store(allowed, Ordering::Relaxed);
    if !allowed {
        critical_section::with(|cs| {
            let mut s = STATE.borrow_ref_mut(cs);
            s.action = 0;
            s.text.clear();
        });
    }
}
/// Собирает снимок состояния для интерфейса.
pub fn view() -> View {
    critical_section::with(|cs| {
        let s = STATE.borrow_ref(cs);
        let mut v = View {
            paired: !s.session.is_empty(),
            pending: !s.candidate.is_empty() && now().saturating_sub(s.pending_since) < 30,
            online: s.online && now().saturating_sub(s.updated) < 15,
            cursor: s.cursor,
            message: s.message,
            ..View::default()
        };
        v.name_len = crate::apps::files::copy_text(&mut v.name, &s.name);
        if s.candidate.len() == 16 {
            v.code.copy_from_slice(&s.candidate.as_bytes()[12..]);
        }
        v
    })
}
/// Обрабатывает событие интерфейса и выполняет соответствующий переход состояния.
pub fn event(event: RuntimeEvent, text: Option<&str>) {
    if !ALLOWED.load(Ordering::Relaxed) {
        return;
    }
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        let pending = !s.candidate.is_empty() && now().saturating_sub(s.pending_since) < 30;
        let count = if pending { 2 } else { 4 };
        match event {
            RuntimeEvent::Rotate(d) => {
                s.cursor = (i32::from(s.cursor) + d.rem_euclid(count)).rem_euclid(count) as u8
            }
            RuntimeEvent::Press if pending => {
                if s.cursor == 0 {
                    s.session = s.candidate.clone();
                    s.message = "Мост разрешён";
                } else {
                    s.message = "Подключение отклонено";
                }
                s.candidate.clear();
                s.cursor = 0;
            }
            RuntimeEvent::Press if s.cursor == 3 => {
                s.session.clear();
                s.candidate.clear();
                s.online = false;
                s.action = 0;
                s.text.clear();
                s.message = "Мост отключён";
            }
            RuntimeEvent::Press
                if !s.session.is_empty() && s.online && now().saturating_sub(s.updated) < 15 =>
            {
                if s.action != 0 && now().saturating_sub(s.requested) < 30 {
                    s.message = "Дождитесь ответа моста";
                    return;
                }
                if s.cursor == 2 {
                    let Some(text) =
                        text.filter(|t| !t.is_empty() && t.len() <= crate::file_store::MAX_FILE)
                    else {
                        s.message = "Откройте сохранённый текст";
                        return;
                    };
                    s.text = text.to_string();
                } else {
                    s.text.clear();
                }
                s.sequence = s.sequence.wrapping_add(1);
                s.action = s.cursor + 1;
                s.requested = now();
                s.message = "Запрос передан мосту";
            }
            RuntimeEvent::Press => s.message = "Нужен запущенный мост KDE",
            _ => {}
        }
    });
}
/// Декодирует hex-байты с проверкой символов и длины.
fn decode_hex(raw: &str) -> Option<alloc::vec::Vec<u8>> {
    if raw.len() > 48 || raw.len() % 2 != 0 {
        return None;
    }
    raw.as_bytes()
        .chunks_exact(2)
        .map(|b| {
            let hi = (b[0] as char).to_digit(16)?;
            let lo = (b[1] as char).to_digit(16)?;
            Some((hi * 16 + lo) as u8)
        })
        .collect()
}
/// Проверяет PIN-доступ и сессию моста перед обработкой его команды.
pub fn shell<const N: usize>(command: &str, out: &mut heapless::String<N>) {
    if !ALLOWED.load(Ordering::Relaxed) {
        let _ = out.push_str("locked");
        return;
    }
    let mut words = command.split_whitespace();
    let op = words.next().unwrap_or("");
    let token = words.next().unwrap_or("");
    if token.len() != 16 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        let _ = out.push_str("invalid session");
        return;
    }
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if op == "hello" {
            if !s.session.is_empty() && s.session != token {
                let _ = out.push_str("another bridge is paired; disconnect on watch");
                return;
            }
            if s.session == token {
                let _ = out.push_str("approved");
                return;
            }
            if s.candidate != token || now().saturating_sub(s.pending_since) >= 30 {
                let name = words
                    .next()
                    .and_then(decode_hex)
                    .and_then(|b| String::from_utf8(b).ok())
                    .unwrap_or_else(|| "KDE Connect".to_string());
                s.name = name.chars().filter(|c| !c.is_control()).take(12).collect();
                s.candidate = token.to_string();
                s.pending_since = now();
                s.cursor = 1;
            }
            let _ = out.push_str("confirm on watch");
            return;
        }
        if s.session != token || s.session.is_empty() {
            let _ = out.push_str("unpaired");
            return;
        }
        match op {
            "update" => {
                s.online = words.next() == Some("1");
                s.updated = now();
                let _ = out.push_str("OK");
            }
            "poll" => {
                let action = if now().saturating_sub(s.requested) < 30 {
                    s.action
                } else {
                    0
                };
                let label = match action {
                    1 => "ring",
                    2 => "ping",
                    3 => "share",
                    _ => "none",
                };
                let _ = write!(
                    out,
                    "{{\"id\":{},\"action\":\"{}\",\"bytes\":{}}}",
                    s.sequence,
                    label,
                    if action == 3 { s.text.len() } else { 0 }
                );
            }
            "text" => {
                let offset = words
                    .next()
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(usize::MAX);
                if s.action == 3 && now().saturating_sub(s.requested) < 30 && offset <= s.text.len()
                {
                    for byte in &s.text.as_bytes()[offset..(offset + 64).min(s.text.len())] {
                        let _ = write!(out, "{:02x}", byte);
                    }
                } else {
                    let _ = out.push_str("expired");
                }
            }
            "ack" => {
                let id = words.next().and_then(|s| s.parse::<u32>().ok());
                if id == Some(s.sequence) {
                    s.action = 0;
                    s.text.clear();
                    s.message = match words.next() {
                        Some("1") => "Команда выполнена мостом",
                        Some("2") => "Запрос принят мостом",
                        _ => "Мост сообщил об ошибке",
                    };
                }
                let _ = out.push_str("OK");
            }
            _ => {
                let _ = out.push_str("unknown bridge command");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Выполняет команду моста в ограниченный тестовый буфер.
    fn call(s: &str) -> heapless::String<512> {
        let mut out = heapless::String::new();
        shell(s, &mut out);
        out
    }
    #[test]
    /// Проверяет обязательное подтверждение моста и запрет его команд при PIN-блокировке.
    fn bridge_needs_watch_approval_and_cannot_act_through_pin_lock() {
        crate::time::set(100);
        set_allowed(true);
        assert_eq!(call("hello 1234567890abcdef 4b4445"), "confirm on watch");
        assert_eq!(call("poll 1234567890abcdef"), "unpaired");
        event(RuntimeEvent::Rotate(-1), None);
        event(RuntimeEvent::Press, None);
        assert_eq!(call("hello 1234567890abcdef"), "approved");
        assert_eq!(call("update 1234567890abcdef 1"), "OK");
        event(RuntimeEvent::Press, None);
        assert!(call("poll 1234567890abcdef").contains("ring"));
        set_allowed(false);
        assert_eq!(call("poll 1234567890abcdef"), "locked");
        set_allowed(true);
        assert!(call("poll 1234567890abcdef").contains("none"));
        event(RuntimeEvent::Rotate(3), None);
        event(RuntimeEvent::Press, None);
        assert!(!view().paired);
        set_allowed(false);
    }
}
