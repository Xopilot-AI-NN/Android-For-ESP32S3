// Модуль погоды, мирового времени и дыхания.
//! Current conditions from the on-watch HTTP client; errors never invent readings.
use super::*;
use core::fmt::Write;
impl Surface {
    /// Рисует экран мирового времени с актуальным состоянием и фокусом.
    pub(super) fn world_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Мировое время", t);
        let offset = i32::from(f.app.world_offset);
        let minutes = (i32::from(f.time_minutes) - crate::rtc::timezone_minutes() + offset * 60)
            .rem_euclid(1440) as u16;
        if f.time_valid {
            self.big_time(87, minutes, true, t.primary);
        } else {
            self.text_center(109, "--:--", 3, t.primary);
        }
        let mut label = heapless::String::<24>::new();
        let _ = write!(label, "UTC{:+}", offset);
        self.text_center_raw(174, &label, 2, t.on_surface);
        self.text_center(
            211,
            if f.time_valid {
                "Колесо: часовой пояс"
            } else {
                "Сначала установите время"
            },
            1,
            t.on_surface_variant,
        );
        self.round_rect(25, 240, 190, 34, 17, t.surface_high);
        self.text_center(249, "Нажать: пояс устройства", 1, t.on_surface);
    }
}
