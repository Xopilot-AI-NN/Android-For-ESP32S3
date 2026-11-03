//! Общие действия/виджеты; аккаунт, TLS и кэш принадлежат системному сервису.
use crate::{
    google_gateway as google,
    runtime::RuntimeEvent,
    text_input::{Action, Keyboard, KeyboardView},
};
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::{cell::RefCell, marker::PhantomData};
use serde_json::Value;
pub trait Endpoint {
    const KIND: u8;
    const INPUT: bool;
    fn offline(_text: &str) -> Option<&'static str> {
        None
    }
}
const LANGUAGES: [(&str, &str); 7] = [
    ("ru", "en"),
    ("en", "ru"),
    ("auto", "en"),
    ("auto", "ru"),
    ("auto", "de"),
    ("auto", "fr"),
    ("auto", "es"),
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct View {
    pub kind: u8,
    pub mode: u8,
    pub cursor: u16,
    pub keyboard: KeyboardView,
    pub text: [u8; 1024],
    pub len: u16,
    pub lines: [[u8; 128]; 7],
    pub lengths: [u8; 7],
    pub message: &'static str,
    pub revision: u32,
    pub qr: [u8; 407],
    pub qr_size: u8,
    pub phase: u8,
    pub email: [u8; 96],
    pub email_len: u8,
    pub services: u8,
    pub busy: bool,
    pub online: bool,
    pub count: u16,
    pub offset: u16,
    pub month: i32,
    pub days: u32,
    pub language: u8,
    pub cached: bool,
    pub month_valid: bool,
    pub error: u8,
    pub more: bool,
    pub total_lines: u16,
    pub page: u16,
    pub provider: [u8; 48],
    pub provider_len: u8,
    pub sync: [u8; 40],
    pub sync_len: u8,
}
pub struct Controller<E: Endpoint> {
    keyboard: Keyboard,
    text: heapless::String<1024>,
    mode: u8,
    cursor: usize,
    scroll: usize,
    page: u16,
    selected: String,
    detail: String,
    header: String,
    seen_epoch: u32,
    language: usize,
    month: i32,
    month_known: bool,
    error: u8,
    qr: RefCell<(String, [u8; 407], u8)>,
    endpoint: PhantomData<E>,
}
impl<E: Endpoint> Default for Controller<E> {
    fn default() -> Self {
        Self {
            keyboard: Keyboard::default(),
            text: heapless::String::new(),
            mode: 0,
            cursor: 0,
            scroll: 0,
            page: 0,
            selected: String::new(),
            detail: String::new(),
            header: String::new(),
            seen_epoch: 0,
            language: 0,
            month: 2000 * 12,
            month_known: false,
            error: 0,
            qr: RefCell::new((String::new(), [0; 407], 0)),
            endpoint: PhantomData,
        }
    }
}
/// UTF-8 копируется целыми символами; размер буфера задаётся самим UI-снимком.
fn copy(out: &mut [u8], text: &str) -> usize {
    let mut at = 0;
    for c in text.chars() {
        if at + c.len_utf8() > out.len() {
            break;
        }
        c.encode_utf8(&mut out[at..at + c.len_utf8()]);
        at += c.len_utf8();
    }
    at
}
fn rows(kind: u8, value: &Value) -> Vec<(String, String)> {
    let key = match kind {
        0 => "results",
        1 => "events",
        _ => "messages",
    };
    value[key].as_array().map_or_else(Vec::new, |items| {
        items
            .iter()
            .take(10)
            .map(|item| {
                let title = google::text(item, if kind == 2 { "subject" } else { "title" }, 160);
                let subtitle = if kind == 1 {
                    item["start"]["dateTime"]
                        .as_str()
                        .or_else(|| item["start"]["date"].as_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    google::text(item, if kind == 2 { "from" } else { "snippet" }, 200)
                };
                (title, subtitle)
            })
            .collect()
    })
}
/// Моноширинный текст разбивается по словам и Unicode, без HTML и исполнения содержимого.
pub fn wrap(text: &str) -> Vec<String> {
    use crate::material::body_character_width as advance;
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut line = String::new();
        let mut width = 0;
        for word in paragraph.split_whitespace() {
            let word_width: i32 = word.chars().map(advance).sum();
            if !line.is_empty() && width + word_width > 216 {
                lines.push(line.trim_end().into());
                line.clear();
                width = 0;
            }
            for c in word.chars() {
                if width + advance(c) > 216 || line.len() + c.len_utf8() > 128 {
                    lines.push(core::mem::take(&mut line));
                    width = 0;
                }
                line.push(c);
                width += advance(c);
            }
            if line.len() < 128 {
                line.push(' ');
                width += advance(' ');
            }
        }
        if !line.is_empty() {
            lines.push(line.trim_end().into());
        }
    }
    lines
}
impl<E: Endpoint> Controller<E> {
    /// Clear private view state as well as the service cache when the account changes.
    pub fn poll(&mut self) {
        let epoch = google::snapshot().account_epoch;
        if self.seen_epoch != epoch {
            self.seen_epoch = epoch;
            self.text.clear();
            self.header.clear();
            self.detail.clear();
            self.selected.clear();
            self.scroll = 0;
            self.page = 0;
            self.qr = RefCell::new((String::new(), [0; 407], 0));
            if matches!(self.mode, 1 | 2 | 4) {
                self.mode = 0;
            }
            self.cursor = 0;
        }
    }
    pub fn open(&mut self) {
        self.mode = 0;
        self.cursor = 0;
        if let Some((year, month, _)) = crate::rtc::calendar_date() {
            self.month = i32::from(year) * 12 + i32::from(month) - 1;
            self.month_known = true;
        }
    }
    pub fn account(&mut self) {
        self.mode = 3;
        self.cursor = 0;
        if google::snapshot().phase == google::Phase::Disconnected {
            let _ = google::request(google::Operation::Renew);
        }
    }
    fn load(&mut self) {
        if E::INPUT && self.text.trim().is_empty() {
            self.mode = 1;
            self.error = 2;
            return;
        }
        if E::KIND == 1 && !self.month_known {
            if let Some((year, month, _)) = crate::rtc::calendar_date() {
                self.month = i32::from(year) * 12 + i32::from(month) - 1;
                self.month_known = true;
            } else {
                self.mode = 5;
                return;
            }
        }
        let (source, target) = LANGUAGES[self.language];
        let query = if E::KIND == 1 {
            format!(
                "{:04}-{:02}",
                self.month.div_euclid(12),
                self.month.rem_euclid(12) + 1
            )
        } else {
            self.text.to_string()
        };
        let accepted = google::fetch(E::KIND, &query, "", 0, source, target);
        if !accepted && (google::snapshot().online || google::cache(E::KIND).stamp == 0) {
            return;
        }
        self.mode = if E::KIND == 3 { 4 } else { 2 };
        self.cursor = 0;
        self.scroll = 0;
        self.detail.clear();
        self.header.clear();
    }
    fn submit(&mut self) {
        if E::KIND == 3 && !google::snapshot().online {
            if let Some(result) = E::offline(&self.text) {
                self.detail = format!("Офлайн-словарь\n{result}");
                self.mode = 4;
                self.scroll = 0;
                return;
            }
        }
        self.load();
    }
    fn account_press(&mut self) {
        use google::{Operation as Op, Phase};
        let s = google::snapshot();
        match s.phase {
            Phase::Waiting | Phase::Creating => google::cancel_pairing(),
            Phase::Confirm => {
                if self.cursor % 2 == 0 {
                    let _ = google::request(Op::Confirm);
                } else {
                    google::cancel_pairing();
                }
            }
            _ => {
                if s.phase == Phase::Connected {
                    match self.cursor % 4 {
                        0 => {
                            let _ = google::request(Op::Pair(1));
                        }
                        1 => {
                            let _ = google::request(Op::Pair(2));
                        }
                        2 => {
                            let _ = google::request(Op::Sync);
                        }
                        _ => {
                            self.mode = 6;
                            self.cursor = 1;
                        }
                    }
                } else {
                    match self.cursor % 4 {
                        0 => {
                            let _ = google::request(Op::Pair(1));
                        }
                        1 => {
                            let _ = google::request(Op::Pair(2));
                        }
                        2 => {
                            let _ = google::request(Op::Pair(3));
                        }
                        _ => {
                            self.mode = 0;
                            self.cursor = 0;
                        }
                    }
                }
            }
        }
        if google::snapshot().phase == google::Phase::Creating {
            self.cursor = 0;
        }
    }
    fn detail_open(&mut self) {
        let cache = google::cache(E::KIND);
        let key = match E::KIND {
            0 => "results",
            1 => "events",
            _ => "messages",
        };
        let Some(item) = cache.value[key].as_array().and_then(|a| a.get(self.cursor)) else {
            return;
        };
        let selected = google::text(item, "id", 128);
        if E::KIND != 1 && !google::fetch(E::KIND, "", &selected, 0, "", "") {
            let offline = !google::snapshot().online
                && google::text(&cache.value, "id", 128) == selected
                && cache.value.get("body").is_some();
            if !offline {
                return;
            }
        }
        self.selected = selected;
        self.header = match E::KIND {
            2 => format!(
                "{}\n{}\n{}\n",
                google::text(item, "subject", 160),
                google::text(item, "from", 200),
                google::text(item, "date", 100)
            ),
            0 => format!(
                "{}\n{}\n",
                google::text(item, "title", 160),
                google::text(item, "url", 200)
            ),
            _ => String::new(),
        };
        self.page = 0;
        self.scroll = 0;
        self.detail = if E::KIND == 1 {
            format!(
                "{}\n{}\n{}\n{}",
                google::text(item, "title", 160),
                item["start"]["dateTime"]
                    .as_str()
                    .or_else(|| item["start"]["date"].as_str())
                    .unwrap_or(""),
                item["end"]["dateTime"]
                    .as_str()
                    .or_else(|| item["end"]["date"].as_str())
                    .unwrap_or(""),
                google::text(item, "location", 160)
            )
        } else {
            String::new()
        };
        if E::KIND == 1 {
            if item["reminders"]["use_default"] == true {
                self.detail.push_str("\nНапоминания Google: по умолчанию");
            } else if let Some(reminders) = item["reminders"]["overrides"].as_array() {
                for reminder in reminders.iter().take(5) {
                    if let Some(minutes) = reminder["minutes"].as_u64() {
                        self.detail.push_str(&format!(
                            "\nGoogle: {} · {minutes} мин",
                            google::text(reminder, "method", 16)
                        ));
                    }
                }
            }
        }
        self.mode = 4;
    }
    pub fn event(&mut self, event: RuntimeEvent) {
        self.poll();
        if !self.month_known {
            if let Some((year, month, _)) = crate::rtc::calendar_date() {
                self.month = i32::from(year) * 12 + i32::from(month) - 1;
                self.month_known = true;
            }
        }
        if self.mode == 1 {
            match event {
                RuntimeEvent::Rotate(d) => self.keyboard.rotate(d),
                RuntimeEvent::Press => match self.keyboard.press() {
                    Action::Character(c) => {
                        self.error = if self.text.push(c).is_err() { 1 } else { 0 };
                    }
                    Action::Backspace => {
                        self.text.pop();
                        self.error = 0;
                    }
                    Action::Suggestion(word) => {
                        let start = self
                            .text
                            .char_indices()
                            .rev()
                            .find(|(_, c)| !c.is_alphabetic())
                            .map_or(0, |(i, c)| i + c.len_utf8());
                        if start + word.len() + 1 <= self.text.capacity() {
                            self.text.truncate(start);
                            let _ = self.text.push_str(word);
                            let _ = self.text.push(' ');
                        }
                    }
                    Action::Submit => self.submit(),
                    Action::Cancel => self.mode = 0,
                    _ => {}
                },
                _ => {}
            }
            self.keyboard.suggest(&self.text);
            return;
        }
        let cache = google::cache(E::KIND);
        let count = if self.mode == 2 {
            rows(E::KIND, &cache.value).len()
                + usize::from(cache.value["next_page"].as_str().is_some())
        } else if self.mode == 6 {
            2
        } else if self.mode == 3 {
            if google::snapshot().phase == google::Phase::Confirm {
                2
            } else {
                4
            }
        } else if E::KIND == 1 {
            4
        } else {
            3
        };
        match event {
            RuntimeEvent::Rotate(d) => {
                if self.mode == 4 {
                    let body = self.body(&cache.value);
                    let max = wrap(&body).len().saturating_sub(7);
                    if d < 0 && self.scroll == 0 && self.page > 0 && !self.selected.is_empty() {
                        let page = self.page - 1;
                        if google::fetch(E::KIND, "", &self.selected, page, "", "") {
                            self.page = page;
                        }
                        return;
                    }
                    self.scroll = (self.scroll as i32 + d).clamp(0, max as i32) as usize;
                } else if self.mode == 5 {
                    if self.month_known {
                        self.month = self.month.saturating_add(d).clamp(1900 * 12, 2100 * 12 - 1);
                    }
                } else {
                    self.cursor = (self.cursor as i32 + d).rem_euclid(count.max(1) as i32) as usize;
                }
            }
            RuntimeEvent::Press => match self.mode {
                3 => self.account_press(),
                6 => {
                    if self.cursor == 0 {
                        google::cancel_pairing();
                        let _ = google::request(google::Operation::Logout);
                    }
                    self.mode = 3;
                    self.cursor = 0;
                }
                5 => self.load(),
                2 => {
                    if self.cursor >= rows(E::KIND, &cache.value).len() {
                        let token = google::text(&cache.value, "next_page", 512);
                        if !token.is_empty() {
                            let query = if E::KIND == 1 {
                                format!("{:04}-{:02}|{token}", self.month / 12, self.month % 12 + 1)
                            } else {
                                token
                            };
                            let _ = google::fetch(E::KIND, &query, "", 0, "", "");
                            self.cursor = 0;
                        }
                    } else {
                        self.detail_open();
                    }
                }
                4 => {
                    if cache.value["has_more"].as_bool().unwrap_or(false)
                        && !self.selected.is_empty()
                    {
                        let page = self.page.saturating_add(1);
                        if google::fetch(E::KIND, "", &self.selected, page, "", "") {
                            self.page = page;
                            self.scroll = 0;
                        }
                    }
                }
                _ => {
                    if self.cursor == if E::KIND == 1 { 3 } else { 2 } {
                        self.account();
                    } else if E::KIND == 1 {
                        if self.cursor == 0 {
                            self.mode = 5;
                        } else {
                            self.load();
                        }
                    } else if E::KIND == 3 && self.cursor == 1 {
                        self.language = (self.language + 1) % LANGUAGES.len();
                    } else if E::INPUT && self.cursor == 0 {
                        self.mode = 1;
                    } else {
                        self.load();
                    }
                }
            },
            _ => {}
        }
    }
    fn body(&self, value: &Value) -> String {
        if !self.detail.is_empty() {
            return self.detail.clone();
        }
        format!(
            "{}{}",
            self.header,
            google::text(value, if E::KIND == 3 { "text" } else { "body" }, 4096)
        )
    }
    pub fn back(&mut self) -> bool {
        if self.mode == 0 {
            return false;
        }
        if self.mode == 3
            && matches!(
                google::snapshot().phase,
                google::Phase::Creating | google::Phase::Waiting | google::Phase::Confirm
            )
        {
            google::cancel_pairing();
        }
        if self.mode == 4 && E::KIND != 3 {
            self.mode = 2;
        } else {
            self.mode = 0;
        }
        self.scroll = 0;
        self.cursor = 0;
        true
    }
    pub fn view(&self) -> View {
        let s = google::snapshot();
        let cache = google::cache(E::KIND);
        let mut v = View {
            kind: E::KIND,
            mode: self.mode,
            cursor: self.cursor as u16,
            keyboard: self.keyboard.view,
            len: self.text.len() as u16,
            message: s.message,
            revision: s.revision,
            phase: s.phase as u8,
            services: s.services,
            busy: s.busy,
            online: s.online,
            month: self.month,
            language: self.language as u8,
            cached: cache.stamp != 0 && !s.online,
            month_valid: self.month_known,
            error: self.error,
            ..View::default()
        };
        v.text[..self.text.len()].copy_from_slice(self.text.as_bytes());
        if !s.busy && s.phase == google::Phase::Connected {
            if E::KIND == 1 && s.services & 1 == 0 {
                v.message = "Разрешите доступ к Calendar";
            }
            if E::KIND == 2 && s.services & 2 == 0 {
                v.message = "Разрешите доступ к Gmail";
            }
        }
        v.email_len = copy(&mut v.email, &s.email) as u8;
        v.sync_len = copy(&mut v.sync, &s.last_sync) as u8;
        v.provider_len = copy(&mut v.provider, &google::text(&cache.value, "provider", 48)) as u8;
        let lines = if self.mode == 4 {
            wrap(&self.body(&cache.value))
        } else if self.mode == 2 {
            let rows = rows(E::KIND, &cache.value);
            v.count = rows.len() as u16;
            let mut out = Vec::new();
            for (title, subtitle) in rows {
                out.push(title);
                out.push(subtitle);
            }
            if cache.value["next_page"].as_str().is_some() {
                out.push("Ещё →".into());
            }
            out
        } else {
            Vec::new()
        };
        v.total_lines = lines.len() as u16;
        v.page = self.page;
        v.more = cache.value["has_more"].as_bool().unwrap_or(false);
        let first = if self.mode == 4 {
            self.scroll
        } else {
            self.cursor.saturating_sub(1) * 2
        };
        v.offset = first as u16;
        for (i, line) in lines.iter().skip(first).take(7).enumerate() {
            v.lengths[i] = copy(&mut v.lines[i], line) as u8;
        }
        if self.mode == 5 {
            let prefix = format!("{:04}-{:02}-", self.month / 12, self.month % 12 + 1);
            if let Some(events) = cache.value["events"].as_array() {
                for e in events {
                    let date = e["start"]["dateTime"]
                        .as_str()
                        .or_else(|| e["start"]["date"].as_str())
                        .unwrap_or("");
                    if let Some(day) = date
                        .strip_prefix(&prefix)
                        .and_then(|tail| tail.get(..2))
                        .and_then(|day| day.parse::<u8>().ok())
                    {
                        if (1..=31).contains(&day) {
                            v.days |= 1 << (day - 1);
                        }
                    }
                }
            }
        }
        if self.mode == 3 && !s.qr.is_empty() {
            let mut saved = self.qr.borrow_mut();
            if saved.0 != s.qr {
                use qrcodegen_no_heap::{QrCode, QrCodeEcc, Version};
                let mut temp = [0; 512];
                let mut output = [0; 512];
                saved.1.fill(0);
                saved.2 = 0;
                if let Ok(qr) = QrCode::encode_text(
                    &s.qr,
                    &mut temp,
                    &mut output,
                    QrCodeEcc::Medium,
                    Version::MIN,
                    Version::new(10),
                    None,
                    true,
                ) {
                    let size = qr.size() as usize;
                    for y in 0..size {
                        for x in 0..size {
                            let bit = y * size + x;
                            if qr.get_module(x as i32, y as i32) {
                                saved.1[bit / 8] |= 1 << (bit % 8);
                            }
                        }
                    }
                    saved.2 = size as u8;
                }
                saved.0 = s.qr;
            }
            v.qr = saved.1;
            v.qr_size = saved.2;
        }
        v
    }
}
impl Default for View {
    fn default() -> Self {
        Self {
            kind: 0,
            mode: 0,
            cursor: 0,
            keyboard: Keyboard::default().view,
            text: [0; 1024],
            len: 0,
            lines: [[0; 128]; 7],
            lengths: [0; 7],
            message: "",
            revision: 0,
            qr: [0; 407],
            qr_size: 0,
            phase: 0,
            email: [0; 96],
            email_len: 0,
            services: 0,
            busy: false,
            online: false,
            count: 0,
            offset: 0,
            month: 2000 * 12,
            days: 0,
            language: 0,
            cached: false,
            month_valid: false,
            error: 0,
            more: false,
            total_lines: 0,
            page: 0,
            provider: [0; 48],
            provider_len: 0,
            sync: [0; 40],
            sync_len: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejecting_a_busy_query_does_not_show_an_old_result_as_a_new_answer() {
        struct Search;
        impl Endpoint for Search {
            const KIND: u8 = 0;
            const INPUT: bool = true;
        }
        crate::google_gateway::tests::setup();
        let mut c = Controller::<Search>::default();
        let _ = c.text.push_str("first");
        c.submit();
        assert_eq!(c.mode, 2);
        c.mode = 1;
        c.text.clear();
        let _ = c.text.push_str("second");
        c.submit();
        assert_eq!(c.mode, 1);
        crate::google_gateway::tests::reset();
    }
    #[test]
    fn body_wrap_uses_real_pixels_and_never_drops_unicode() {
        for text in [
            "Широкие буквы ЩШМ должны переноситься по ширине экрана.",
            "Grüße, français, español: café et œufs.",
            &"😀".repeat(80),
            &"https://example.invalid/".repeat(20),
        ] {
            let lines = wrap(text);
            for line in &lines {
                assert!(
                    line.chars()
                        .map(crate::material::body_character_width)
                        .sum::<i32>()
                        <= 216
                );
                assert!(line.len() <= 128);
            }
            let original: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            let result: String = lines
                .iter()
                .flat_map(|s| s.chars())
                .filter(|c| !c.is_whitespace())
                .collect();
            assert_eq!(result, original);
        }
    }
}
