// Модуль редактируемого имени устройства.
//! Bounded UTF-8 friendly name; persisted separately from immutable identity.
use core::{
    cell::RefCell,
    sync::atomic::{AtomicU32, Ordering},
};
use critical_section::Mutex;
use heapless::String;

/// Параметр NAME компонента редактируемого имени устройства.
static NAME: Mutex<RefCell<String<32>>> = Mutex::new(RefCell::new(String::new()));
/// Счётчик изменений, по которому обновляются зависимые кэши.
static REVISION: AtomicU32 = AtomicU32::new(0);

/// Возвращает согласованный снимок текущего состояния.
pub fn snapshot() -> String<32> {
    critical_section::with(|cs| {
        let name = NAME.borrow(cs).borrow();
        if name.is_empty() {
            String::try_from(crate::device_info::DEFAULT_DEVICE_NAME).unwrap_or_default()
        } else {
            name.clone()
        }
    })
}
/// Возвращает счётчик изменений для обновления зависимых кэшей.
pub fn revision() -> u32 {
    REVISION.load(Ordering::Relaxed)
}
/// Проверяет имя и увеличивает ревизию только при реальном изменении.
pub fn set(raw: &str) -> bool {
    let value = raw.trim();
    if value.is_empty() || value.len() > 32 || value.chars().any(char::is_control) {
        return false;
    }
    critical_section::with(|cs| {
        let mut name = NAME.borrow(cs).borrow_mut();
        if name.as_str() != value {
            name.clear();
            let _ = name.push_str(value);
            REVISION.fetch_add(1, Ordering::Relaxed);
        }
    });
    true
}

/// Device-name editor uses the same layout and actions as Wi-Fi text input.
#[derive(Default)]
pub struct Editor {
    /// Признак активности режима.
    pub active: bool,
    /// Состояние общей RU/EN-клавиатуры.
    pub keyboard: crate::text_input::Keyboard,
    /// Черновик ввода; его изменение ещё не означает запись на носитель.
    pub draft: String<32>,
    /// Ошибка последней операции или признак ошибки.
    pub error: u8,
}
impl Editor {
    /// Открывает редактор с копией сохранённого имени.
    pub fn begin(&mut self) {
        self.draft = snapshot();
        self.keyboard = Default::default();
        self.error = 0;
        self.active = true;
        self.keyboard.suggest(&self.draft);
    }
    /// Перемещает фокус на число шагов колёсика с учётом границ.
    pub fn rotate(&mut self, delta: i32) {
        self.keyboard.rotate(delta);
    }
    /// Обрабатывает клавишу и сохраняет имя только после подтверждения.
    pub fn press(&mut self) {
        use crate::text_input::Action;
        self.error = 0;
        match self.keyboard.press() {
            Action::Character(ch) => {
                if self.draft.push(ch).is_err() {
                    self.error = 1;
                }
            }
            Action::Suggestion(word) => {
                if !crate::text_input::complete(&mut self.draft, word) {
                    self.error = 1;
                }
            }
            Action::Backspace => {
                self.draft.pop();
            }
            Action::Submit => {
                if set(&self.draft) {
                    self.active = false;
                } else {
                    self.error = 2;
                }
            }
            Action::Cancel => self.active = false,
            Action::None => {}
        }
        self.keyboard.suggest(&self.draft);
    }
}
