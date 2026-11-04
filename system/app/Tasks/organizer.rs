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
                16,
                if row.selected {
                    t.primary
                } else {
                    t.surface_high
                },
            );
            let text = core::str::from_utf8(&row.text[..row.len as usize]).unwrap_or("");
            let ink = Self::focus_ink(t, row.selected);
            let checked = text.starts_with("[x]");
            let task = checked || text.starts_with("[ ]");
            if task {
                self.round_rect(24, y + 14, 18, 18, 5, if checked { ink } else { t.outline });
                if checked {
                    let bg = if row.selected {
                        t.primary
                    } else {
                        t.surface_high
                    };
                    self.line(28, y + 23, 32, y + 27, 2, bg);
                    self.line(32, y + 27, 39, y + 18, 2, bg);
                } else {
                    self.round_rect(
                        26,
                        y + 16,
                        14,
                        14,
                        3,
                        if row.selected {
                            t.primary
                        } else {
                            t.surface_high
                        },
                    );
                }
            } else if text.starts_with("Добав") {
                self.line(26, y + 23, 38, y + 23, 2, ink);
                self.line(32, y + 17, 32, y + 29, 2, ink);
            } else {
                self.symbol(
                    if text.contains("Сохран") {
                        Symbol::Done
                    } else if text.contains("Отмен") || text.contains("Назад") {
                        Symbol::Back
                    } else {
                        Symbol::Info
                    },
                    32,
                    y + 23,
                    ink,
                );
            }
            self.text_fitted_raw(
                52,
                y + 17,
                if task {
                    text.get(4..).unwrap_or(text)
                } else {
                    text
                },
                1,
                162,
                ink,
            );
        }
        self.text_fitted_raw(
            12,
            255,
            if v.busy {
                "Чтение/сохранение…"
            } else if v.dirty {
                "Не сохранено: нажмите Сохранить"
            } else {
                v.message
            },
            1,
            216,
            t.on_surface_variant,
        );
        self.scroll_indicator(v.cursor, v.count, t);
    }
}
