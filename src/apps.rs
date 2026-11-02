//! Native utilities use an overlay, preserving the on-card framework screen ABI.
pub mod calculator;
pub mod storage;
use crate::runtime::RuntimeEvent;
pub const NONE: u8 = 255;
pub const CALCULATOR: u8 = 6;
pub const FILES: u8 = 7;
pub const SHARE: u8 = 8;
pub const DIAGNOSTICS: u8 = 9;
pub const COUNT: u8 = 11;
pub const LABELS: [&str; COUNT as usize] = [
    "Clock",
    "Media",
    "Settings",
    "Connectivity",
    "System info",
    "Assistant",
    "Калькулятор",
    "Файлы",
    "Передача файлов",
    "Диагностика",
    "Flashlight",
];
pub fn route(index: u8) -> Option<u8> {
    match index {
        0 => Some(6),
        1 => Some(10),
        2 => Some(5),
        3 => Some(7),
        4 => Some(8),
        5 => Some(27),
        10 => Some(29),
        _ => None,
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct View {
    pub id: u8,
    pub cursor: u8,
    pub text: [u8; 32],
    pub len: u8,
    pub uptime: u64,
    pub free_kib: u32,
    pub internal_kib: u32,
    pub script_us: u64,
    pub cache_hits: u32,
}
impl Default for View {
    fn default() -> Self {
        Self {
            id: NONE,
            cursor: 0,
            text: [0; 32],
            len: 0,
            uptime: 0,
            free_kib: 0,
            internal_kib: 0,
            script_us: 0,
            cache_hits: 0,
        }
    }
}
pub struct State {
    view: View,
    calculator: calculator::Calculator,
}
impl Default for State {
    fn default() -> Self {
        Self {
            view: View::default(),
            calculator: calculator::Calculator::default(),
        }
    }
}
impl State {
    pub fn active(&self) -> bool {
        self.view.id != NONE
    }
    pub fn close(&mut self) {
        self.view.id = NONE;
    }
    pub fn open(&mut self, id: u8) {
        if !(CALCULATOR..=DIAGNOSTICS).contains(&id) {
            return;
        }
        self.view.id = id;
        self.view.cursor = 0;
        self.refresh_text();
    }
    pub fn view(&self) -> View {
        self.view
    }
    fn refresh_text(&mut self) {
        let text = self.calculator.text();
        self.view.text.fill(0);
        self.view.len = text.len().min(32) as u8;
        self.view.text[..usize::from(self.view.len)]
            .copy_from_slice(&text.as_bytes()[..usize::from(self.view.len)]);
    }
    /// Work is performed only on input; utility pages have no polling tasks.
    pub fn event(&mut self, event: RuntimeEvent) -> Option<u8> {
        let count = match self.view.id {
            CALCULATOR => 20,
            FILES => i32::from(crate::wallpaper::builtin_count()),
            SHARE => 3,
            _ => 5,
        };
        match event {
            RuntimeEvent::Rotate(delta) => {
                self.view.cursor =
                    (i32::from(self.view.cursor) + delta.rem_euclid(count)).rem_euclid(count) as u8
            }
            RuntimeEvent::Swipe(delta) => {
                self.view.cursor =
                    (i32::from(self.view.cursor) + delta.signum()).rem_euclid(count) as u8
            }
            RuntimeEvent::Press if self.view.id == CALCULATOR => {
                self.calculator.press(self.view.cursor);
                self.refresh_text();
            }
            RuntimeEvent::Press if self.view.id == FILES => return Some(self.view.cursor + 1),
            _ => {}
        }
        None
    }
}
