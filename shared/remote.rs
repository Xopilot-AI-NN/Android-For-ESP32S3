//! Общий контроллер запросов Google-приложений; OAuth-учётные данные здесь не хранятся.
use crate::{
    runtime::RuntimeEvent,
    text_input::{Action, Keyboard, KeyboardView},
};
use core::marker::PhantomData;
use heapless::String;
/// Пакет определяет доступные действия и собственный offline-перевод, а не копирует OAuth.
pub trait Endpoint {
    const KIND: u8;
    const INPUT: bool;
    fn offline(_text: &str) -> Option<&'static str> {
        None
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct View {
    pub kind: u8,
    pub mode: u8,
    pub cursor: u8,
    pub keyboard: KeyboardView,
    pub text: [u8; 128],
    pub len: u8,
    pub result: [u8; 192],
    pub result_len: u8,
    pub message: &'static str,
}
pub struct Controller<E: Endpoint> {
    keyboard: Keyboard,
    text: String<128>,
    result: String<192>,
    mode: u8,
    cursor: u8,
    message: &'static str,
    endpoint: PhantomData<E>,
}
impl<E: Endpoint> Default for Controller<E> {
    fn default() -> Self {
        Self {
            keyboard: Keyboard::default(),
            text: String::new(),
            result: String::new(),
            mode: 0,
            cursor: 0,
            message: "HTTPS-клиент часов ещё не подключён",
            endpoint: PhantomData,
        }
    }
}
impl<E: Endpoint> Controller<E> {
    pub fn open(&mut self) {
        self.mode = 0;
        self.cursor = 0;
    }
    pub fn account(&mut self) {
        self.mode = 3;
        self.cursor = 0;
    }
    fn submit(&mut self) {
        self.mode = 2;
        self.result.clear();
        if let Some(value) = E::offline(&self.text) {
            let _ = self.result.push_str(value);
            self.message = "Офлайн-словарь RU / EN";
        } else {
            self.message = crate::google_gateway::submit(E::KIND, &self.text);
        }
    }
    pub fn event(&mut self, event: RuntimeEvent) {
        if self.mode == 1 {
            match event {
                RuntimeEvent::Rotate(d) => self.keyboard.rotate(d),
                RuntimeEvent::Press => match self.keyboard.press() {
                    Action::Character(c) => {
                        let _ = self.text.push(c);
                    }
                    Action::Backspace => {
                        self.text.pop();
                    }
                    Action::Suggestion(s) => {
                        let _ = self.text.push_str(s);
                    }
                    Action::Submit => self.submit(),
                    Action::Cancel => self.mode = 0,
                    _ => {}
                },
                _ => {}
            }
            return;
        }
        match event {
            RuntimeEvent::Rotate(d) => {
                self.cursor = (i32::from(self.cursor) + d).rem_euclid(3) as u8
            }
            RuntimeEvent::Press => {
                if self.mode == 3 {
                    self.message = crate::google_gateway::account_action(self.cursor);
                } else if self.cursor == 2 {
                    self.account();
                } else if E::INPUT {
                    self.mode = 1;
                } else {
                    self.message = crate::google_gateway::submit(E::KIND, "");
                }
            }
            _ => {}
        }
    }
    pub fn back(&mut self) -> bool {
        if self.mode != 0 {
            self.mode = 0;
            true
        } else {
            false
        }
    }
    pub fn view(&self) -> View {
        let mut v = View {
            kind: E::KIND,
            mode: self.mode,
            cursor: self.cursor,
            keyboard: self.keyboard.view,
            text: [0; 128],
            len: self.text.len() as u8,
            result: [0; 192],
            result_len: self.result.len() as u8,
            message: self.message,
        };
        v.text[..self.text.len()].copy_from_slice(self.text.as_bytes());
        v.result[..self.result.len()].copy_from_slice(self.result.as_bytes());
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
            text: [0; 128],
            len: 0,
            result: [0; 192],
            result_len: 0,
            message: "",
        }
    }
}
