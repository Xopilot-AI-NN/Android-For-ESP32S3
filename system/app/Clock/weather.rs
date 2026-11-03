// Модуль погоды, мирового времени и дыхания.
//! Current conditions from the on-watch HTTP client; errors never invent readings.
use super::*;
impl Surface {
    /// Рисует экран мирового времени с актуальным состоянием и фокусом.
    pub(super) fn world_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Мировые часы", t);
        let utc = i32::from(f.time_minutes) - crate::rtc::timezone_minutes();
        for (i, offset) in [0, 3, i32::from(f.app.world_offset)].iter().enumerate() {
            let y = 68 + i as i32 * 59;
            let active = i == 2;
            self.round_rect(
                10,
                y,
                220,
                51,
                16,
                if active {
                    t.primary_container
                } else {
                    t.surface
                },
            );
            self.symbol(Symbol::World, 29, y + 25, t.primary);
            self.text_raw(
                49,
                y + 8,
                &alloc::format!("UTC{:+}", offset),
                1,
                t.on_surface_variant,
            );
            if active {
                self.text_raw(49, y + 29, "Выбранный", 1, t.on_surface_variant);
            }
            let text = Self::time_text((utc + offset * 60).rem_euclid(1440) as u16);
            self.text_fitted_raw(
                137,
                y + 17,
                if f.time_valid {
                    core::str::from_utf8(&text).unwrap_or("--:--")
                } else {
                    "--:--"
                },
                2,
                86,
                t.on_surface,
            );
        }
    }
}
