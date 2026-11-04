// Модуль нативных утилит.
//! Small crown-driven utilities. No network or storage I/O occurs while painting.
use super::*;
impl Surface {
    /// Выводит строку сведений в стандартной позиции утилиты.
    pub(super) fn utility_line(&mut self, row: i32, text: &str, t: Theme) {
        let y = 68 + row * 36;
        self.round_rect(10, y, 220, 31, 10, t.surface_high);
        self.text_fitted_raw(20, y + 8, text, 1, 200, t.on_surface);
    }
    /// Выбирает отрисовку активного нативного приложения.
    pub(super) fn native_application(&mut self, f: MaterialFrame, t: Theme) {
        use crate::apps::*;
        let a = f.app;
        match a.id {
            AIR_KEYBOARD => {
                self.air_keyboard_page(f, t);
                return;
            }
            BROWSER => {
                self.browser_page(f, t);
                return;
            }
            FILES | EDITOR => {
                self.files_page(f, t);
                return;
            }
            TASKS => {
                self.organizer_page(f, t);
                return;
            }
            FOCUS => {
                self.focus_page(f, t);
                return;
            }
            PHONE => {
                self.phone_page(f, t);
                return;
            }
            CALENDAR => {
                if a.cursor == 1 {
                    self.remote_app_page(f, t);
                } else {
                    self.calendar_page(f, t);
                }
                return;
            }
            GALLERY => {
                self.gallery_page(f, t);
                return;
            }
            BT_KEYBOARD => {
                self.keyboard_app_page(f, t);
                return;
            }
            GOOGLE | GOOGLE_CALENDAR | GMAIL | TRANSLATOR | CONTACTS | MESSAGES => {
                self.remote_app_page(f, t);
                return;
            }

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
        match a.id {
            CALCULATOR => self.calculator_page(f, t),
            SHARE => self.share_page(f, t),
            MEMORY => self.memory_page(f, t),
            DIAGNOSTICS => self.diagnostics_page(f, t),
            _ => {}
        }
    }
}
