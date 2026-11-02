//! Native utilities use an overlay, preserving the on-card framework screen ABI.
pub mod calculator;
pub mod storage;
use crate::runtime::RuntimeEvent;
pub const NONE: u8 = 255;
pub const CALCULATOR: u8 = 6;
pub const FILES: u8 = 7;
pub const SHARE: u8 = 8;
pub const DIAGNOSTICS: u8 = 9;
pub const MEMORY: u8 = 11;
pub const WEATHER: u8 = 12;
pub const WORLD: u8 = 13;
pub const BREATHING: u8 = 14;
pub const COUNT: u8 = 15;
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
    "Память и swap",
    "Погода",
    "Мировое время",
    "Дыхание",
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
    pub memory: crate::swap_config::Snapshot,
    pub weather: crate::weather::Snapshot,
    pub weather_editor: crate::weather::EditorView,
    pub world_offset: i8,
    pub breathe_elapsed: u16,
    pub breathing: bool,
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
            memory: crate::swap_config::Snapshot::default(),
            weather: crate::weather::Snapshot::default(),
            weather_editor: crate::weather::EditorView::default(),
            world_offset: 3,
            breathe_elapsed: 0,
            breathing: false,
        }
    }
}
pub struct State {
    view: View,
    calculator: calculator::Calculator,
    weather: crate::weather::Controller,
    world_offset: i8,
    breathe_started: Option<u64>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            view: View::default(),
            calculator: calculator::Calculator::default(),
            weather: crate::weather::Controller::default(),
            world_offset: 3,
            breathe_started: None,
        }
    }
}
impl State {
    pub fn keep_awake(&self) -> bool {
        self.view.id == BREATHING
            && self.breathe_started.is_some_and(|start| {
                esp_hal::time::Instant::now()
                    .duration_since_epoch()
                    .as_secs()
                    .saturating_sub(start)
                    < 125
            })
    }
    pub fn active(&self) -> bool {
        self.view.id != NONE
    }
    pub fn close(&mut self) {
        self.view.id = NONE;
        self.weather.cancel();
        self.breathe_started = None;
    }
    pub fn open(&mut self, id: u8) {
        if !(CALCULATOR..=BREATHING).contains(&id) {
            return;
        }
        self.view.id = id;
        self.view.cursor = if id == MEMORY {
            crate::swap_config::CHOICES
                .iter()
                .position(|&m| m == crate::swap_config::snapshot().selected)
                .unwrap_or(0) as u8
        } else {
            0
        };
        self.refresh_text();
    }
    pub fn cancel_editor(&mut self) -> bool {
        self.view.id == WEATHER && self.weather.cancel()
    }
    pub fn view(&self) -> View {
        let mut view = self.view;
        if view.id == WEATHER {
            view.weather = crate::weather::snapshot();
            view.weather_editor = self.weather.view();
        }
        view.world_offset = self.world_offset;
        if view.id == BREATHING {
            if let Some(start) = self.breathe_started {
                let elapsed = esp_hal::time::Instant::now()
                    .duration_since_epoch()
                    .as_secs()
                    .saturating_sub(start);
                view.breathe_elapsed = elapsed.min(120) as u16;
                view.breathing = elapsed < 120;
            }
        }
        view
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
        if self.view.id == WEATHER {
            self.weather.event(event);
            return None;
        }
        if self.view.id == WORLD {
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.world_offset = ((i32::from(self.world_offset) + 12 + d.rem_euclid(27))
                        .rem_euclid(27)
                        - 12) as i8
                }
                RuntimeEvent::Press => self.world_offset = crate::rtc::timezone_hours(),
                _ => {}
            }
            return None;
        }
        if self.view.id == BREATHING {
            if event == RuntimeEvent::Press {
                let now = esp_hal::time::Instant::now()
                    .duration_since_epoch()
                    .as_secs();
                self.breathe_started = if self
                    .breathe_started
                    .is_some_and(|start| now.saturating_sub(start) < 120)
                {
                    None
                } else {
                    Some(now)
                };
            }
            return None;
        }
        let count = match self.view.id {
            CALCULATOR => 20,
            FILES => i32::from(crate::wallpaper::builtin_count()),
            SHARE => 3,
            MEMORY => 5,
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
            RuntimeEvent::Press if self.view.id == MEMORY => crate::swap_config::request(
                crate::swap_config::CHOICES[usize::from(self.view.cursor)],
            ),
            _ => {}
        }
        None
    }
}
