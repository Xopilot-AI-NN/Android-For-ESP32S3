//! Paint a bounded flow layout; HTML parsing and network I/O never run here.
use super::*;
impl Surface {
    pub(super) fn browser_page(&mut self, mut f: MaterialFrame, t: Theme) {
        let v = f.app.browser;
        let address = core::str::from_utf8(&v.address[..v.address_len as usize]).unwrap_or("");
        if v.mode == 1 {
            f.keyboard = v.keyboard;
            f.keyboard_error = v.error;
            self.text_keyboard(f, t, "Адрес сайта", address, "Открыть");
            return;
        }
        self.photo_header(f, "Браузер", t);
        if v.mode == 0 {
            self.text_center_raw(80, "Сайты на ваших часах", 1, t.on_surface_variant);
            for (i, label) in [
                "Открыть адрес",
                "История",
                if v.light {
                    "Светлая тема"
                } else {
                    "Тёмная тема"
                },
            ]
            .iter()
            .enumerate()
            {
                self.action_chip(116 + i as i32 * 45, 36, label, v.cursor == i as u16, t);
            }
        } else if v.mode == 3 {
            let labels = [
                "Адрес сайта",
                "Назад",
                "Вперёд",
                "Обновить",
                "Сменить тему",
                "Главная",
                "Закрыть браузер",
            ];
            let start = usize::from(v.cursor).saturating_sub(2).min(2);
            for (row, index) in (start..7).take(5).enumerate() {
                self.action_chip(
                    66 + row as i32 * 36,
                    32,
                    labels[index],
                    v.cursor as usize == index,
                    t,
                );
            }
        } else if v.mode == 5 {
            self.text_center_raw(66, "История сеанса", 1, t.on_surface_variant);
            if v.total == 0 {
                self.text_center_raw(140, "Пока пусто", 1, t.on_surface_variant);
            }
            for (i, row) in v.rows[..v.rows_len as usize].iter().enumerate() {
                let text = core::str::from_utf8(&row.text[..row.len as usize]).unwrap_or("");
                self.action_chip(88 + i as i32 * 36, 32, text, row.selected, t);
            }
        } else {
            let title = core::str::from_utf8(&v.title[..v.title_len as usize]).unwrap_or("");
            self.text_fitted_raw(12, 58, title, 1, 216, t.primary);
            let bg = if v.light { 0xffff } else { t.bg };
            let ink = if v.light { 0x1082 } else { t.on_surface };
            self.round_rect(0, 78, 240, 164, 0, bg);
            if v.mode == 4 {
                if let Some(image) = crate::google_gateway::mail_image(7) {
                    let x0 = (240 - image.width as i32 * 2) / 2;
                    let y0 = 80 + (160 - image.height as i32 * 2) / 2;
                    for y in 0..image.height {
                        for x in 0..image.width {
                            self.round_rect(
                                x0 + x as i32 * 2,
                                y0 + y as i32 * 2,
                                2,
                                2,
                                0,
                                image.pixels[y * image.width + x],
                            );
                        }
                    }
                }
            } else {
                let mut y = 81;
                for row in &v.rows[..v.rows_len as usize] {
                    let text = core::str::from_utf8(&row.text[..row.len as usize]).unwrap_or("");
                    let x = 14 + i32::from(row.indent) * 8;
                    if row.selected {
                        self.round_rect(3, y, 3, row.height - 3, 1, t.primary);
                    }
                    let color = if row.kind == 3 {
                        if v.light { 0x0250 } else { t.primary }
                    } else {
                        ink
                    };
                    match row.kind {
                        4 => {
                            self.round_rect(x, y, 212, 36, 7, t.surface_high);
                            self.text_fitted_raw(
                                x + 8,
                                y + 10,
                                if text.is_empty() {
                                    "Открыть изображение"
                                } else {
                                    text
                                },
                                1,
                                192,
                                t.primary,
                            );
                        }
                        6 => self.round_rect(x, y + 5, 210, 1, 0, t.on_surface_variant),
                        _ => self.text_fitted_raw(
                            x,
                            y + 2,
                            text,
                            if row.kind == 1 { 2 } else { 1 },
                            224 - x,
                            color,
                        ),
                    }
                    y += row.height;
                }
                if v.rows_len == 0 {
                    self.text_fitted_raw(
                        14,
                        110,
                        if v.busy {
                            "Загрузка…"
                        } else {
                            "Нет содержимого"
                        },
                        1,
                        212,
                        ink,
                    );
                }
            }
            self.text_fitted_raw(10, 246, v.message, 1, 220, t.on_surface_variant);
            self.text_center_raw(
                266,
                &alloc::format!(
                    "{} / {} · {}",
                    v.page + 1,
                    v.pages.max(1),
                    if v.busy {
                        "Загрузка"
                    } else {
                        "2×: меню"
                    }
                ),
                1,
                t.on_surface_variant,
            );
        }
        if matches!(v.mode, 0 | 3 | 5) {
            self.text_fitted_raw(10, 260, v.message, 1, 220, t.on_surface_variant);
        }
    }
}
