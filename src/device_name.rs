//! Bounded UTF-8 friendly name; persisted separately from immutable identity.
use core::{
    cell::RefCell,
    sync::atomic::{AtomicU32, Ordering},
};
use critical_section::Mutex;
use heapless::String;

static NAME: Mutex<RefCell<String<32>>> = Mutex::new(RefCell::new(String::new()));
static REVISION: AtomicU32 = AtomicU32::new(0);

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
pub fn revision() -> u32 {
    REVISION.load(Ordering::Relaxed)
}
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
    pub active: bool,
    pub keyboard: crate::text_input::Keyboard,
    pub draft: String<32>,
    pub error: u8,
}
impl Editor {
    pub fn begin(&mut self) {
        self.draft = snapshot();
        self.keyboard = Default::default();
        self.error = 0;
        self.active = true;
    }
    pub fn rotate(&mut self, delta: i32) {
        self.keyboard.rotate(delta);
    }
    pub fn press(&mut self) {
        use crate::text_input::Action;
        self.error = 0;
        match self.keyboard.press() {
            Action::Character(ch) => {
                if self.draft.push(ch).is_err() {
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
    }
}
