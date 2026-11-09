// Модуль основных экранов часов.
//! pages main Material surface implementation.
use super::*;

impl Surface {

    /// Рисует заблокированный экран без выдачи доступа к приложениям.
    pub(super) fn lock_screen(&mut self, f: MaterialFrame, t: Theme) {
        self.big_time(64, f.time_minutes, f.time_valid, t.on_surface);
        self.icon_lock(120, 195, t.primary);
        self.text_center(238, "PRESS CROWN TO UNLOCK", 1, t.on_surface_variant);
    }

    /// Рисует уведомления и доступное действие над выбранной записью.
    pub(super) fn notifications(&mut self, f: MaterialFrame, t: Theme) {
        if f.notes == 0 {
            self.circle(120, 113, 32, t.surface);
            self.icon_bell(120, 113, t.on_surface_variant);
            self.text_center(161, "NO NOTIFICATIONS", 1, t.on_surface_variant);
        } else {
            let selected = f.cursor == 0;
            self.round_rect(
                12,
                50,
                216,
                112,
                35,
                Self::focus_fill(t.surface, selected),
            );
            self.circle(44, 82, 14, t.primary);
            self.icon_bell(44, 82, t.on_primary);
            self.text(67, 67, "SYSTEM", 1, Self::focus_ink(t, selected));
            self.text(
                67,
                88,
                "NOTIFICATION",
                2,
                Self::focus_ink(t, selected),
            );
            self.text_fitted(67, 119, "PRESS TO DISMISS", 1, 149, Self::focus_ink(t, selected));
            if f.notes > 1 {
                self.round_rect(27, 175, 186, 48, 24, t.surface);
                self.text_center(191, "MORE NOTIFICATIONS", 1, t.on_surface_variant);
            }
        }
    }

    /// Рисует быстрые переключатели и управление яркостью.
    pub(super) fn quick_settings(&mut self, f: MaterialFrame, t: Theme) {
        self.quick_button(63, 76, "Wi-Fi", f.wifi, f.cursor == 0, t);
        self.quick_button(177, 76, "Bluetooth", f.bt, f.cursor == 1, t);
        self.quick_button(63, 140, "DND", f.dnd, f.cursor == 2, t);
        self.quick_button(177, 140, "Airplane", f.airplane, f.cursor == 3, t);
        self.quick_button(63, 204, "Flashlight", false, f.cursor == 4, t);
        self.quick_button(177, 204, "Settings", false, f.cursor == 5, t);
        self.icon_sun(27, 256, t.primary);
        self.slider(48, 251, 173, f.brightness, t);
        if f.cursor == 6 {
            self.ring(
                48 + 7 + (159 * f.brightness as i32 / 100),
                255,
                14,
                2,
                t.on_surface,
            );
        }
    }
}
