// Модуль экранов календаря, конвертера и счётчика.
//! Offline applications share the system header, focus colors and crown navigation.
use super::*;
use crate::apps::utilities::{MONTHS, days_in_month, first_weekday};

impl Surface {

    /// Рисует экран календаря с актуальным состоянием и фокусом.
    pub(super) fn calendar_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Календарь", t);
        let tools = f.app.tools;
        let year = tools.month.div_euclid(12);
        let month = tools.month.rem_euclid(12) as u8 + 1;
        self.text_center_raw(
            62,
            &alloc::format!("{} {}", MONTHS[usize::from(month - 1)], year),
            2,
            t.primary,
        );
        for (col, day) in ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"]
            .iter()
            .enumerate()
        {
            self.text_center_x(24 + col as i32 * 32, 99, day, 1, t.on_surface_variant);
        }
        let offset = first_weekday(year, month);
        for day in 1..=days_in_month(year, month) {
            let cell = i32::from(offset + day - 1);
            let x = 24 + (cell % 7) * 32;
            let y = 123 + (cell / 7) * 21;
            let today = tools.today == Some((year as u16, month, day));
            if today {
                self.round_rect(x - 14, y - 3, 28, 21, 8, t.primary);
            }
            self.text_center_x(
                x,
                y,
                &alloc::format!("{}", day),
                1,
                if today { t.on_primary } else { t.on_surface },
            );
        }
        self.text_center(249, "Колесо: месяц", 1, t.on_surface_variant);
        self.text_center(
            265,
            if tools.today.is_some() {
                "Нажать: сегодня"
            } else {
                "Дата не синхронизирована"
            },
            1,
            t.on_surface_variant,
        );
    }
}
