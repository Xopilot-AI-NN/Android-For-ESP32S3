// Модуль нативных утилит.
//! Small crown-driven utilities. No network or storage I/O occurs while painting.
use super::*;
impl Surface {
    /// Выводит строку сведений в стандартной позиции утилиты.
    pub(super) fn utility_line(&mut self, row: i32, text: &str, t: Theme) {
        self.text_center_raw(77 + row * 34, text, 1, t.on_surface);
    }
    /// Выбирает отрисовку активного нативного приложения.
    pub(super) fn native_application(&mut self, f: MaterialFrame, t: Theme) {
        use crate::apps::*;
        let a = f.app;
        match a.id {
            FILES | EDITOR => { self.files_page(f, t); return; }
            TASKS | EXPENSES => { self.organizer_page(f, t); return; }
            FOCUS => { self.focus_page(f, t); return; }
            PHONE => { self.phone_page(f, t); return; }
            CALENDAR => { self.calendar_page(f, t); return; }
            CONVERTER => { self.converter_page(f, t); return; }
            COUNTER => { self.counter_page(f, t); return; }
            _ => {}
        }
        if a.id == WEATHER {
            self.weather_page(f, t);
            return;
        }
        if a.id == WORLD {
            self.world_page(f, t);
            return;
        }
        if a.id == BREATHING {
            self.breathing_page(f, t);
            return;
        }
        match a.id {
            CALCULATOR => self.calculator_page(f, t),
            SHARE => self.share_page(f, t),
            MEMORY => self.memory_page(f, t),
            DIAGNOSTICS => self.diagnostics_page(f, t),
            _ => {}
        }
    }
}
