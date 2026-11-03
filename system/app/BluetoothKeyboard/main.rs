//! Стандартная BLE HID-клавиатура: клавиши передаются только зашифрованному подписчику.
use crate::{
    runtime::{BluetoothUiAction, RuntimeEvent},
    text_input::{Action, Keyboard, KeyboardView},
};
use heapless::{String, Vec};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct View {
    pub keyboard: KeyboardView,
    pub typing: bool,
    pub cursor: u8,
    pub message: &'static str,
    pub text: [u8; 64],
    pub len: u8,
}
pub struct Controller {
    keyboard: Keyboard,
    typing: bool,
    cursor: u8,
    pending: Vec<BluetoothUiAction, 16>,
    text: String<64>,
    message: &'static str,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            keyboard: Keyboard::english(),
            typing: false,
            cursor: 0,
            pending: Vec::new(),
            text: String::new(),
            message: "",
        }
    }
}
/// HID Usage Page 7 кодирует физическую клавишу; раскладку выбирает принимающее устройство.
pub fn report(ch: char) -> Option<[u8; 8]> {
    let (usage, shift) = match ch {
        'a'..='z' => (ch as u8 - b'a' + 4, false),
        'A'..='Z' => (ch as u8 - b'A' + 4, true),
        '1'..='9' => (ch as u8 - b'1' + 30, false),
        '0' => (39, false),
        ' ' => (44, false),
        '\n' => (40, false),
        '\u{8}' => (42, false),
        '.' => (55, false),
        ',' => (54, false),
        '-' => (45, false),
        '_' => (45, true),
        '!' => (30, true),
        '?' => (56, true),
        ':' => (51, true),
        '/' => (56, false),
        _ => return None,
    };
    Some([if shift { 2 } else { 0 }, 0, usage, 0, 0, 0, 0, 0])
}
impl Controller {
    /// Сбрасывает неподанные действия, чтобы повторный вход не отправлял старые клавиши.
    pub fn open(&mut self) {
        self.pending.clear();
        self.typing = false;
        self.cursor = 0;
    }
    /// Ставит key-down в ограниченную очередь; key-up добавляет защищённый BLE-сервис.
    fn key(&mut self, ch: char) {
        if let Some(report) = report(ch) {
            self.message = if self
                .pending
                .push(BluetoothUiAction::Keyboard(report))
                .is_ok()
            {
                if ch == '\u{8}' {
                    self.text.pop();
                } else {
                    let _ = self.text.push(ch);
                }
                "Клавиша в очереди"
            } else {
                "Очередь занята"
            };
        } else {
            self.message = "Выберите EN; раскладка — на приёмнике";
        }
    }
    /// Навигирует общей клавиатурой; неподдерживаемый Unicode не подменяет ASCII.
    pub fn event(&mut self, e: RuntimeEvent) {
        if !self.typing {
            match e {
                RuntimeEvent::Rotate(d) => {
                    self.cursor = (i32::from(self.cursor) + d).rem_euclid(2) as u8
                }
                RuntimeEvent::Press if self.cursor == 0 => {
                    let _ = self.pending.push(BluetoothUiAction::Pair);
                }
                RuntimeEvent::Press => self.typing = true,
                _ => {}
            }
            return;
        }
        match e {
            RuntimeEvent::Rotate(d) => self.keyboard.rotate(d),
            RuntimeEvent::Press => match self.keyboard.press() {
                Action::Character(c) => self.key(c),
                Action::Backspace => self.key('\u{8}'),
                Action::Submit => self.key('\n'),
                Action::Cancel => self.typing = false,
                Action::Suggestion(word) => {
                    for c in word.chars() {
                        self.key(c);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    /// Передаёт следующее действие radio-router без доступа приложения к GATT.
    pub fn take(&mut self) -> Option<BluetoothUiAction> {
        if self.pending.is_empty() {
            None
        } else {
            Some(self.pending.remove(0))
        }
    }
    /// Закрывает клавиатуру перед выходом из приложения.
    pub fn back(&mut self) -> bool {
        if self.typing {
            self.typing = false;
            true
        } else {
            false
        }
    }
    /// Возвращает локальный UI-снимок; отправка BLE не выполняется при рисовании.
    pub fn view(&self) -> View {
        let mut v = View {
            keyboard: self.keyboard.view,
            typing: self.typing,
            cursor: self.cursor,
            message: self.message,
            text: [0; 64],
            len: self.text.len() as u8,
        };
        v.text[..self.text.len()].copy_from_slice(self.text.as_bytes());
        v
    }
}
impl Default for View {
    fn default() -> Self {
        Controller::default().view()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_have_modifier_and_release_compatible_usages() {
        assert_eq!(report('A').unwrap(), [2, 0, 4, 0, 0, 0, 0, 0]);
        assert_eq!(report('a').unwrap()[2], 4);
        assert_eq!(report('\n').unwrap()[2], 40);
        assert!(report('Ж').is_none());
    }
}
