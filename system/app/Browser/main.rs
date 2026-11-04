//! Anonymous HTTPS browser. History/layout live in bounded RAM; Gateway owns HTML/resources.
mod engine;
use crate::{
    google_gateway as web,
    runtime::RuntimeEvent,
    text_input::{Action as KeyAction, Keyboard, KeyboardView},
};
use alloc::{string::String, vec::Vec};
use core::cell::RefCell;
use critical_section::Mutex;
use engine::{Action, Layout};
static OPEN: Mutex<RefCell<Option<String>>> = Mutex::new(RefCell::new(None));
pub fn request_search_result(id: String) {
    critical_section::with(|cs| *OPEN.borrow_ref_mut(cs) = Some(id));
}
pub fn take_open() -> Option<String> {
    critical_section::with(|cs| OPEN.borrow_ref_mut(cs).take())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Row {
    pub text: [u8; 112],
    pub len: u8,
    pub kind: u8,
    pub indent: u8,
    pub selected: bool,
    pub height: i32,
}
impl Default for Row {
    fn default() -> Self {
        Self {
            text: [0; 112],
            len: 0,
            kind: 0,
            indent: 0,
            selected: false,
            height: 18,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct View {
    pub mode: u8,
    pub keyboard: KeyboardView,
    pub error: u8,
    pub address: [u8; 512],
    pub address_len: u16,
    pub title: [u8; 96],
    pub title_len: u8,
    pub rows: [Row; 8],
    pub rows_len: u8,
    pub cursor: u16,
    pub total: u16,
    pub page: u16,
    pub pages: u16,
    pub light: bool,
    pub busy: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub message: &'static str,
}
impl Default for View {
    fn default() -> Self {
        Self {
            mode: 0,
            keyboard: Keyboard::default().view,
            error: 0,
            address: [0; 512],
            address_len: 0,
            title: [0; 96],
            title_len: 0,
            rows: [Row::default(); 8],
            rows_len: 0,
            cursor: 0,
            total: 0,
            page: 0,
            pages: 0,
            light: false,
            busy: false,
            can_back: false,
            can_forward: false,
            message: "HTTPS · HTML",
        }
    }
}
struct Visit {
    id: String,
    url: String,
    title: String,
    page: u16,
    line: usize,
}
#[derive(Clone, Copy)]
enum Pending {
    New,
    Reload,
    History(usize),
    Page(u16, bool),
    Image(u16),
}
pub struct Controller {
    active: bool,
    mode: u8,
    cursor: usize,
    menu: usize,
    light: bool,
    keyboard: Keyboard,
    address: heapless::String<512>,
    error: u8,
    history: Vec<Visit>,
    current: usize,
    layout: Layout,
    pages: u16,
    pending: Option<Pending>,
    message: &'static str,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            active: false,
            mode: 0,
            cursor: 0,
            menu: 0,
            light: false,
            keyboard: Keyboard::default(),
            address: heapless::String::new(),
            error: 0,
            history: Vec::new(),
            current: 0,
            layout: Layout::default(),
            pages: 0,
            pending: None,
            message: "HTTPS · HTML",
        }
    }
}
impl Controller {
    pub fn open(&mut self) {
        self.active = true;
        self.mode = 0;
        self.menu = 0;
        self.error = 0;
    }
    pub fn open_result(&mut self, id: String) {
        self.close();
        self.open();
        self.navigate(serde_json::json!({"result_id":id}), Pending::New);
    }
    pub fn close(&mut self) {
        web::cancel_data(7);
        web::clear_preview(7);
        self.active = false;
        self.pending = None;
        // Closing the app must return its layout/history capacity to the heap.
        self.layout = Layout::default();
        self.history = Vec::new();
        self.pages = 0;
        self.cursor = 0;
        self.current = 0;
    }
    fn navigate(&mut self, value: serde_json::Value, action: Pending) {
        if let Ok(query) = serde_json::to_string(&value) {
            if web::fetch(7, &query, "", 0, "", "") {
                self.pending = Some(action);
                self.message = "Загрузка сайта…";
                self.mode = 2;
            } else {
                self.message = web::snapshot().message;
            }
        }
    }
    fn page(&mut self, page: u16, tail: bool) {
        if let Some(visit) = self.history.get(self.current) {
            if web::fetch(7, "", &visit.id, page, "", "") {
                self.pending = Some(Pending::Page(page, tail));
                self.message = "Загрузка страницы…";
            }
        }
    }
    fn history(&mut self, index: usize) {
        if let Some(visit) = self.history.get(index) {
            if web::fetch(7, "", &visit.id, visit.page, "", "") {
                self.pending = Some(Pending::History(index));
                self.message = "История…";
                self.mode = 2;
            }
        }
    }
    pub fn poll(&mut self) {
        if !self.active || self.pending.is_none() {
            return;
        }
        let status = web::snapshot();
        if status.busy {
            return;
        }
        let pending = self.pending.take().unwrap();
        let cache = web::cache(7);
        let value = &cache.value;
        if let Pending::Image(index) = pending {
            if value["kind"] == "browser-image"
                && value["index"].as_u64() == Some(u64::from(index))
                && web::mail_image(7).is_some()
            {
                self.mode = 4;
                self.message = "Назад: к странице";
            } else {
                self.message = status.message;
                self.mode = 2;
            }
            return;
        }
        if value["kind"] != "browser-page" {
            self.message = status.message;
            return;
        }
        let id = web::text(value, "document", 64);
        let page = value["page"].as_u64().unwrap_or(0) as u16;
        if let Pending::Page(expected, _) = pending {
            if expected != page {
                self.message = status.message;
                return;
            }
        }
        let url = web::text(value, "url", 2048);
        let title = web::text(value, "title", 120);
        match pending {
            Pending::New => {
                if !self.history.is_empty() {
                    self.history.truncate(self.current + 1);
                }
                if self.history.len() >= 16 {
                    self.history.remove(0);
                }
                self.history.push(Visit {
                    id: id.clone(),
                    url: url.clone(),
                    title: title.clone(),
                    page,
                    line: 0,
                });
                self.current = self.history.len() - 1;
                self.cursor = 0;
            }
            Pending::Reload => {
                if let Some(visit) = self.history.get_mut(self.current) {
                    *visit = Visit {
                        id: id.clone(),
                        url: url.clone(),
                        title: title.clone(),
                        page,
                        line: 0,
                    };
                }
                self.cursor = 0;
            }
            Pending::History(index) => {
                self.current = index;
                self.cursor = self.history.get(index).map_or(0, |v| v.line);
            }
            Pending::Page(_, _) => {
                self.cursor = 0;
            }
            Pending::Image(_) => {}
        }
        self.layout = Layout::build(value);
        if matches!(pending, Pending::Page(_, true)) {
            self.cursor = self.layout.lines.len().saturating_sub(1);
        }
        self.cursor = self.cursor.min(self.layout.lines.len().saturating_sub(1));
        if let Some(visit) = self.history.get_mut(self.current) {
            visit.page = page;
            visit.line = self.cursor;
        }
        self.pages = value["page_count"].as_u64().unwrap_or(1).min(65535) as u16;
        self.mode = 2;
        self.message = if value["needs_js"] == true {
            "Сайту нужен JavaScript"
        } else {
            "Колесо: читать · Нажатие: открыть"
        };
    }
    fn address(&mut self) {
        self.address.clear();
        if let Some(visit) = self.history.get(self.current) {
            for c in visit.url.chars() {
                if self.address.push(c).is_err() {
                    break;
                }
            }
        }
        self.keyboard = Keyboard::default();
        self.mode = 1;
        self.error = 0;
    }
    pub fn back(&mut self) -> bool {
        if self.pending.is_some() {
            web::cancel_data(7);
            self.pending = None;
            self.message = "Загрузка отменена";
            self.mode = if self.history.is_empty() { 0 } else { 2 };
            return true;
        }
        match self.mode {
            1 | 3 | 5 => {
                self.mode = if self.history.is_empty() { 0 } else { 2 };
                true
            }
            4 => {
                web::clear_preview(7);
                self.mode = 2;
                true
            }
            2 if self.current > 0 => {
                self.history(self.current - 1);
                true
            }
            _ => false,
        }
    }
    /// true asks the native app router to close the browser.
    pub fn event(&mut self, event: RuntimeEvent) -> bool {
        self.poll();
        if matches!(event, RuntimeEvent::Click(2)) && self.mode != 1 {
            self.mode = 3;
            self.menu = 0;
            return false;
        }
        let event = match event {
            RuntimeEvent::Swipe(d) => RuntimeEvent::Rotate(d.signum()),
            RuntimeEvent::Click(_) => RuntimeEvent::Press,
            _ => event,
        };
        if self.mode == 1 {
            match event {
                RuntimeEvent::Rotate(d) => self.keyboard.rotate(d),
                RuntimeEvent::Press => match self.keyboard.press() {
                    KeyAction::Character(c) => {
                        self.error = if self.address.push(c).is_err() { 1 } else { 0 }
                    }
                    KeyAction::Backspace => {
                        self.address.pop();
                        self.error = 0;
                    }
                    KeyAction::Submit => {
                        if self.address.trim().is_empty() {
                            self.error = 2;
                        } else {
                            self.navigate(
                                serde_json::json!({"url":self.address.as_str().trim()}),
                                Pending::New,
                            );
                        }
                    }
                    KeyAction::Cancel => {
                        self.mode = if self.history.is_empty() { 0 } else { 2 };
                    }
                    _ => {}
                },
                _ => {}
            }
            return false;
        }
        if self.pending.is_some() {
            return false;
        }
        match event {
            RuntimeEvent::Rotate(d) if self.mode == 2 => {
                let next = self.cursor as i64 + i64::from(d);
                let page = self.history.get(self.current).map_or(0, |v| v.page);
                if next < 0 && page > 0 {
                    self.page(page - 1, true);
                } else if next >= self.layout.lines.len() as i64 && page + 1 < self.pages {
                    self.page(page + 1, false);
                } else {
                    self.cursor =
                        next.clamp(0, self.layout.lines.len().saturating_sub(1) as i64) as usize;
                    if let Some(v) = self.history.get_mut(self.current) {
                        v.line = self.cursor;
                    }
                }
            }
            RuntimeEvent::Rotate(d) => {
                let count = match self.mode {
                    0 => 3,
                    3 => 7,
                    5 => self.history.len().max(1),
                    _ => 1,
                };
                self.menu = (self.menu as i32 + d).rem_euclid(count as i32) as usize;
            }
            RuntimeEvent::Press if self.mode == 2 => {
                match self
                    .layout
                    .lines
                    .get(self.cursor)
                    .and_then(|line| line.action)
                {
                    Some(Action::Link(index)) => {
                        if let Some(v) = self.history.get(self.current) {
                            self.navigate(
                                serde_json::json!({"document":v.id,"link":index}),
                                Pending::New,
                            );
                        }
                    }
                    Some(Action::Image(index)) => {
                        if let Some(v) = self.history.get(self.current) {
                            if web::fetch(7, "image", &v.id, index, "", "") {
                                self.pending = Some(Pending::Image(index));
                                self.message = "Загрузка картинки…";
                            }
                        }
                    }
                    None => {
                        self.mode = 3;
                        self.menu = 0;
                    }
                }
            }
            RuntimeEvent::Press if self.mode == 0 => match self.menu {
                0 => self.address(),
                1 => {
                    self.mode = 5;
                    self.menu = self.current;
                }
                _ => self.light = !self.light,
            },
            RuntimeEvent::Press if self.mode == 3 => match self.menu {
                0 => self.address(),
                1 if self.current > 0 => self.history(self.current - 1),
                2 if self.current + 1 < self.history.len() => self.history(self.current + 1),
                1 => self.message = "В истории нет предыдущей страницы",
                2 => self.message = "В истории нет следующей страницы",
                3 => {
                    if let Some(v) = self.history.get(self.current) {
                        self.navigate(serde_json::json!({"url":v.url}), Pending::Reload);
                    }
                }
                4 => {
                    self.light = !self.light;
                    self.mode = if self.history.is_empty() { 0 } else { 2 };
                }
                5 => {
                    self.mode = 0;
                    self.menu = 0;
                }
                6 => return true,
                _ => {}
            },
            RuntimeEvent::Press if self.mode == 5 => self.history(self.menu),
            RuntimeEvent::Press if self.mode == 4 => {
                self.mode = 2;
                web::clear_preview(7);
            }
            _ => {}
        }
        false
    }
    pub fn view(&self) -> View {
        let mut view = View {
            mode: self.mode,
            keyboard: self.keyboard.view,
            error: self.error,
            light: self.light,
            busy: self.pending.is_some(),
            cursor: self.cursor as u16,
            total: self.layout.lines.len() as u16,
            pages: self.pages,
            message: self.message,
            can_back: self.current > 0,
            can_forward: self.current + 1 < self.history.len(),
            ..View::default()
        };
        let (url, title, page) = self
            .history
            .get(self.current)
            .map_or(("", "Браузер", 0), |v| {
                (v.url.as_str(), v.title.as_str(), v.page)
            });
        let address = if self.mode == 1 {
            self.address.as_str()
        } else {
            url
        };
        view.address_len = copy(&mut view.address, address) as u16;
        view.title_len = copy(&mut view.title, title) as u8;
        view.page = page;
        if self.mode == 2 {
            let first = self.cursor.saturating_sub(2);
            let mut height = 0;
            for line in self.layout.lines.iter().skip(first) {
                if view.rows_len >= 8 || height + line.height() > 160 {
                    break;
                }
                let index = view.rows_len as usize;
                view.rows[index] = Row {
                    kind: line.kind,
                    indent: line.indent,
                    selected: first + index == self.cursor,
                    height: line.height(),
                    ..Row::default()
                };
                view.rows[index].len = copy(&mut view.rows[index].text, &line.text) as u8;
                view.rows_len += 1;
                height += line.height();
            }
        } else if self.mode == 5 {
            view.cursor = self.menu as u16;
            view.total = self.history.len() as u16;
            for (i, visit) in self
                .history
                .iter()
                .skip(self.menu.saturating_sub(1))
                .take(4)
                .enumerate()
            {
                view.rows[i].len = copy(&mut view.rows[i].text, &visit.title) as u8;
                view.rows[i].selected = self.menu.saturating_sub(1) + i == self.menu;
                view.rows_len += 1;
            }
        } else {
            view.cursor = self.menu as u16;
        }
        view
    }
}
fn copy(out: &mut [u8], text: &str) -> usize {
    let mut size = 0;
    for c in text.chars() {
        if size + c.len_utf8() > out.len() {
            break;
        }
        size += c.encode_utf8(&mut out[size..]).len();
    }
    size
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reply(request: crate::google_gateway::Request, page: u16, blocks: serde_json::Value) {
        web::complete(request.id,200,&serde_json::to_vec(&serde_json::json!({"kind":"browser-page","document":"fixture-document-123456","url":"https://example.org/","title":"Fixture","page":page,"page_count":2,"blocks":blocks})).unwrap());
    }
    #[test]
    fn cancelling_first_navigation_returns_home_and_close_releases_buffers() {
        web::tests::setup();
        let mut browser = Controller::default();
        browser.open_result("fixture".into());
        let request = web::take_request().unwrap();
        assert!(browser.back());
        assert_eq!(browser.view().mode, 0);
        reply(request, 0, serde_json::json!([]));
        browser.poll();
        assert!(browser.history.is_empty());
        browser.open_result("fixture".into());
        let request = web::take_request().unwrap();
        reply(
            request,
            0,
            serde_json::json!([{"index":0,"type":"paragraph","text":"Content"}]),
        );
        browser.poll();
        assert!(browser.history.capacity() > 0);
        assert!(browser.layout.lines.capacity() > 0);
        browser.close();
        assert_eq!(browser.history.capacity(), 0);
        assert_eq!(browser.layout.lines.capacity(), 0);
        assert_eq!(browser.view().pages, 0);
        web::tests::reset();
    }

    #[test]
    fn browser_preview_decodes_once_paints_pixels_and_is_released_on_back() {
        web::tests::setup();
        let mut browser = Controller::default();
        browser.open_result("fixture".into());
        let request = web::take_request().unwrap();
        reply(
            request,
            0,
            serde_json::json!([{"index":4,"type":"image","text":"Photo"}]),
        );
        browser.poll();
        browser.event(RuntimeEvent::Press);
        let request = web::take_request().unwrap();
        assert!(request.path.ends_with("/images/4"));
        web::complete(request.id,200,&serde_json::to_vec(&serde_json::json!({
            "kind":"browser-image","index":4,"image":{"codec":"rgb565-zlib","width":2,"height":1,"data":"eJxj+PGAHQAEswHg"}
        })).unwrap());
        browser.poll();
        assert_eq!(browser.mode, 4);
        assert!(web::cache(7).value["image"].get("data").is_none());
        let mut frame =
            crate::material::MaterialFrame::parse("M3|1|0|70|0|0|0|0|1|1|0|0|600|0|0|0|1|1|3")
                .unwrap();
        frame.app.id = crate::apps::BROWSER;
        frame.app.browser = browser.view();
        let mut surface = crate::material::Surface::new();
        surface.render(frame);
        assert_eq!(surface.pixels()[159 * 240 + 118], 0xf800);
        assert_eq!(surface.pixels()[159 * 240 + 120], 0x07e0);
        assert!(browser.back());
        assert_eq!(browser.mode, 2);
        assert!(web::mail_image(7).is_none());
        browser.close();
        web::tests::reset();
    }
    #[test]
    fn navigation_pages_history_errors_and_cancel_do_not_reuse_old_responses() {
        web::tests::setup(); // Browser is independent of a Google account/session.
        let mut browser = Controller::default();
        browser.open_result("result-capability".into());
        let request = web::take_request().unwrap();
        assert_eq!(request.path, "/api/v1/browser/open");
        assert_eq!(request.method, "POST");
        assert!(request.authorization.starts_with("Device "));
        reply(
            request,
            0,
            serde_json::json!([{"index":2,"type":"link","text":"Next"}]),
        );
        browser.poll();
        assert_eq!(browser.history.len(), 1);
        browser.event(RuntimeEvent::Press);
        let request = web::take_request().unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&request.body).unwrap()["link"],
            2
        );
        reply(
            request,
            0,
            serde_json::json!([{"index":0,"type":"paragraph","text":"Other"}]),
        );
        browser.poll();
        assert_eq!(browser.history.len(), 2);
        assert!(browser.back());
        let request = web::take_request().unwrap();
        assert!(request.path.ends_with("?page=0"));
        reply(
            request,
            0,
            serde_json::json!([{"index":0,"type":"paragraph","text":"First"}]),
        );
        browser.poll();
        assert_eq!(browser.current, 0);
        browser.event(RuntimeEvent::Rotate(1));
        let request = web::take_request().unwrap();
        assert!(request.path.ends_with("?page=1"));
        reply(
            request,
            1,
            serde_json::json!([{"index":1,"type":"paragraph","text":"Second"}]),
        );
        browser.poll();
        assert_eq!(browser.history[0].page, 1);
        browser.page(0, false);
        let request = web::take_request().unwrap();
        web::complete(request.id, 410, br#"{"error":"BROWSER_PAGE_EXPIRED"}"#);
        browser.poll();
        assert_eq!(browser.history[0].page, 1);
        assert_eq!(browser.message, "Страница устарела · обновите");
        browser.page(0, false);
        let request = web::take_request().unwrap();
        browser.close();
        reply(request, 0, serde_json::json!([]));
        browser.poll();
        assert!(browser.history.is_empty());
        assert!(!browser.active);
        assert!(web::cache(7).value.is_null());
        web::tests::reset();
    }
}
