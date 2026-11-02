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

// Crown editor actions follow characters: erase, save, cancel.
pub const KEYS: &str = " ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.";
#[derive(Default)]
pub struct Editor {
    pub active: bool,
    pub key: u8,
    pub draft: String<32>,
}
impl Editor {
    pub fn begin(&mut self) {
        self.draft = snapshot();
        self.key = 0;
        self.active = true;
    }
    pub fn rotate(&mut self, delta: i32) {
        self.key = (i32::from(self.key) + delta).rem_euclid(KEYS.len() as i32 + 3) as u8;
    }
    pub fn press(&mut self) {
        let key = usize::from(self.key);
        if key < KEYS.len() {
            let _ = self.draft.push(KEYS.as_bytes()[key] as char);
        } else if key == KEYS.len() {
            self.draft.pop();
        } else if key == KEYS.len() + 1 {
            if set(&self.draft) {
                self.active = false;
            }
        } else {
            self.active = false;
        }
    }
}
