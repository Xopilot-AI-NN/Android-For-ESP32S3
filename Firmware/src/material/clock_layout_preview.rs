//! Host-only layout studies. The watch's active UI is unchanged until a layout is chosen.
use super::*;
impl Surface {
    pub fn preview_clock_layout(&mut self, mut frame: MaterialFrame, dock: bool) {
        frame.screen = 6;
        frame.app.id = crate::apps::NONE;
        frame.cursor = 0;
        frame.time_minutes = 21 * 60 + 2;
        frame.time_valid = true;
        frame.date_day = 4;
        frame.date_month = 11;
        self.render(frame);
        let t = theme(frame.theme);
        self.fill_rect(0, 54, 240, 226, t.bg);
        let time = Self::time_text(frame.time_minutes);
        let time = core::str::from_utf8(&time).unwrap_or("--:--");
        let icons = [Symbol::Alarm, Symbol::Timer, Symbol::Clock, Symbol::World];
        if dock {
            // A large content card and a separate four-action dock. The focused
            // action gets a readable label above the icons, rather than four
            // tiny labels squeezed into 48-pixel buttons.
            self.round_rect(12, 62, 216, 128, 22, t.surface);
            self.clock_text(83, time.as_bytes(), t.primary);
            self.text_center_raw(151, "04 ноября · среда", 1, t.on_surface_variant);
            self.text_center_raw(200, "Будильник", 1, t.primary);
            for (i, icon) in icons.iter().enumerate() {
                let x = 12 + i as i32 * 56;
                let selected = i == 0;
                self.round_rect(
                    x,
                    222,
                    48,
                    44,
                    13,
                    if selected { t.primary } else { t.surface_high },
                );
                self.symbol(*icon, x + 24, 244, Self::focus_ink(t, selected));
            }
        } else {
            self.round_rect(12, 62, 216, 70, 18, t.surface);
            self.text_raw(24, 77, time, 3, t.primary);
            self.text_fitted_raw(160, 86, "04.11", 1, 60, t.on_surface_variant);
            for (i, icon) in icons.iter().enumerate() {
                let (x, y) = (12 + i as i32 % 2 * 114, 144 + i as i32 / 2 * 66);
                let selected = i == 0;
                let ink = Self::focus_ink(t, selected);
                self.round_rect(
                    x,
                    y,
                    102,
                    56,
                    14,
                    if selected { t.primary } else { t.surface_high },
                );
                self.symbol(*icon, x + 51, y + 16, ink);
                if i == 3 {
                    self.text_center_x(x + 51, y + 28, "Мировые", 1, ink);
                    self.text_center_x(x + 51, y + 41, "часы", 1, ink);
                } else {
                    self.text_center_x(
                        x + 51,
                        y + 35,
                        ["Будильник", "Таймер", "Секундомер"][i],
                        1,
                        ink,
                    );
                }
            }
        }
    }
}
