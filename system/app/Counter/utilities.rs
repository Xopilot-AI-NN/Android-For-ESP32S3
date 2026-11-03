// Модуль экранов календаря, конвертера и счётчика.
//! Offline applications share the system header, focus colors and crown navigation.
use super::*;

impl Surface {

    /// Рисует экран ручного счётчика с актуальным состоянием и фокусом.
    pub(super) fn counter_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Счётчик", t);
        let tools = f.app.tools;
        self.text_center_raw(80, &alloc::format!("{}", tools.count), 3, t.primary);
        self.text_center(129, "Ручной подсчёт", 1, t.on_surface_variant);
        for (i, label) in [
            "+1",
            "−1",
            if tools.reset_armed {
                "Подтвердить сброс"
            } else {
                "Сбросить"
            },
        ]
        .iter()
        .enumerate()
        {
            self.keyboard_key(
                18,
                157 + i as i32 * 40,
                204,
                33,
                label,
                usize::from(tools.counter_action) == i,
                t,
            );
        }
        self.text_center(
            264,
            "Сбросится при перезапуске",
            1,
            t.on_surface_variant,
        );
    }
}
