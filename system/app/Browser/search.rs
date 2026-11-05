//! Search is native too: the browser does not borrow a Google account session.
use crate::apps::remote::View;
use crate::{
    browser_service as web,
    runtime::RuntimeEvent,
    text_input::{Action, Keyboard, complete},
};
use alloc::string::String;
pub struct Controller {
    mode: u8,
    text: heapless::String<512>,
    keyboard: Keyboard,
    cursor: usize,
    pending: bool,
    seen: u64,
    error: u8,
    failed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_failure_can_be_retried_without_showing_stale_results() {
        web::tests::setup();
        let mut search = Controller::default();
        search.mode = 2;
        search.pending = true;
        search.text.push_str("rust").unwrap();
        assert!(web::fetch(0, "rust", "", 0, "", ""));
        let request = web::take_request().unwrap();
        web::fail(request.id, "DuckDuckGo требует проверку");
        search.poll();
        assert_eq!(search.view().error, 3);
        assert_eq!(search.view().count, 0);
        search.event(RuntimeEvent::Press);
        assert_eq!(search.view().mode, 1);
        assert_eq!(search.view().error, 0);
        assert_eq!(search.text.as_str(), "rust");
        web::tests::reset();
    }
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            mode: 0,
            text: heapless::String::new(),
            keyboard: Keyboard::default(),
            cursor: 0,
            pending: false,
            seen: 0,
            error: 0,
            failed: false,
        }
    }
}
impl Controller {
    pub fn begin_input(&mut self) {
        self.mode = 1;
        self.keyboard.suggest(&self.text);
        self.error = 0;
        self.failed = false;
    }
    pub fn back(&mut self) {
        if self.pending {
            web::cancel_data(0);
            self.pending = false;
        }
        self.mode = 0;
    }
    pub fn poll(&mut self) {
        if self.pending && !web::snapshot().busy {
            self.pending = false;
            let cache = web::cache(0);
            if cache.stamp != self.seen {
                self.seen = cache.stamp;
                self.mode = 2;
                self.cursor = 0;
                self.failed = false;
            } else {
                self.failed = true;
            }
        }
    }
    pub fn event(&mut self, event: RuntimeEvent) {
        self.poll();
        if self.mode == 1 {
            match event {
                RuntimeEvent::Rotate(d) => self.keyboard.rotate(d),
                RuntimeEvent::Press => match self.keyboard.press() {
                    Action::Character(c) => self.error = u8::from(self.text.push(c).is_err()),
                    Action::Backspace => {
                        self.text.pop();
                        self.error = 0;
                    }
                    Action::Suggestion(s) => {
                        complete(&mut self.text, s);
                    }
                    Action::Submit => {
                        if self.text.trim().is_empty() {
                            self.error = 2;
                        } else {
                            self.seen = web::cache(0).stamp;
                            self.pending = web::fetch(0, &self.text, "", 0, "", "");
                            self.failed = !self.pending;
                            self.mode = 2;
                        }
                    }
                    Action::Cancel => self.back(),
                    _ => {}
                },
                _ => {}
            }
            self.keyboard.suggest(&self.text);
            return;
        }
        let cache = web::cache(0);
        if self.failed {
            if event == RuntimeEvent::Press {
                self.begin_input();
            }
            return;
        }
        if self.pending {
            return;
        }
        let rows = cache.value["results"].as_array();
        let count = rows.map_or(0, |v| v.len());
        match event {
            RuntimeEvent::Rotate(d) if count > 0 => {
                self.cursor =
                    (self.cursor as i64 + i64::from(d)).clamp(0, count as i64 - 1) as usize
            }
            RuntimeEvent::Press if self.mode == 2 => {
                if let Some(url) = rows
                    .and_then(|a| a.get(self.cursor))
                    .and_then(|v| v["url"].as_str())
                {
                    super::request_search_result(String::from(url));
                }
            }
            RuntimeEvent::Press => self.begin_input(),
            _ => {}
        }
    }
    pub fn view(&self) -> View {
        let status = web::snapshot();
        let cache = web::cache(0);
        let rows = if self.failed || self.pending {
            None
        } else {
            cache.value["results"].as_array()
        };
        let mut v = View {
            kind: 0,
            mode: self.mode,
            keyboard: self.keyboard.view,
            error: if self.failed { 3 } else { self.error },
            busy: self.pending,
            message: status.message,
            cursor: self.cursor as u16,
            len: self.text.len() as u16,
            count: rows.map_or(0, |a| a.len()) as u16,
            offset: (self.cursor.saturating_sub(1) * 2) as u16,
            online: status.online,
            ..View::default()
        };
        v.text[..self.text.len()].copy_from_slice(self.text.as_bytes());
        let provider = "DuckDuckGo";
        v.provider[..provider.len()].copy_from_slice(provider.as_bytes());
        v.provider_len = provider.len() as u8;
        if let Some(rows) = rows {
            for (i, item) in rows
                .iter()
                .skip(self.cursor.saturating_sub(1))
                .take(3)
                .enumerate()
            {
                let url = item["url"].as_str().unwrap_or("");
                let host = url
                    .strip_prefix("https://")
                    .unwrap_or(url)
                    .split('/')
                    .next()
                    .unwrap_or("");
                for (j, text) in [
                    host,
                    item["snippet"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(item["title"].as_str().unwrap_or("")),
                ]
                .iter()
                .enumerate()
                {
                    let mut n = text.len().min(128);
                    while !text.is_char_boundary(n) {
                        n -= 1;
                    }
                    v.lines[i * 2 + j][..n].copy_from_slice(&text.as_bytes()[..n]);
                    v.lengths[i * 2 + j] = n as u8;
                }
            }
        }
        v
    }
}
