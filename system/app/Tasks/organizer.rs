// Модуль экранов задач, расходов, фокуса и телефона.
use super::*;
impl Surface {

    /// Рисует экран задач или расходов с актуальным состоянием и фокусом.
    pub(super) fn organizer_page(&mut self, f: MaterialFrame, t: Theme) {
        let v = f.app.organizer;
        let expenses = f.app.id == crate::apps::EXPENSES;
        if v.editing {
            let value = core::str::from_utf8(&v.input[..v.input_len as usize]).unwrap_or("");
            self.text_keyboard(
                f,
                t,
                if expenses {
                    "12.50 продукты"
                } else {
                    "Новая задача"
                },
                value,
                "OK",
            );
            return;
        }
        self.photo_header(
            f,
            if v.discarding {
                "Отменить изменения?"
            } else if v.confirming {
                "Удалить расход?"
            } else if expenses {
                "Расходы"
            } else {
                "Задачи"
            },
            t,
        );
        self.text_center_raw(
            60,
            &if expenses {
                alloc::format!("Всего: {}", crate::apps::organizer::money(v.total))
            } else {
                alloc::format!("Выполнено: {}", v.total)
            },
            1,
            t.primary,
        );
        for (i, row) in v.rows.iter().enumerate() {
            if row.len == 0 {
                continue;
            }
            let y = 88 + i as i32 * 54;
            self.round_rect(
                12,
                y,
                216,
                47,
                20,
                Self::focus_fill(t.surface_high, row.selected),
            );
            let text = core::str::from_utf8(&row.text[..row.len as usize]).unwrap_or("");
            self.text_fitted_raw(23, y + 17, text, 1, 194, Self::focus_ink(t, row.selected));
        }
        self.text_center(
            255,
            if v.busy {
                "Чтение/сохранение…"
            } else if v.dirty {
                "Не сохранено: нажмите Сохранить"
            } else {
                v.message
            },
            1,
            t.on_surface_variant,
        );
        self.scroll_indicator(v.cursor, v.count, t);
    }
}
