//! Paint a bounded flow layout; HTML parsing and network I/O never run here.
use super::*;
impl Surface {
    pub(super) fn browser_page(&mut self, mut f: MaterialFrame, t: Theme) {
        let v = f.app.browser;
        let address = core::str::from_utf8(&v.address[..v.address_len as usize]).unwrap_or("");
        if v.mode == 1 {
            f.keyboard = v.keyboard;
            f.keyboard_error = v.error;
            self.text_keyboard(
                f,
                t,
                if v.error == 3 {
                    v.message
                } else {
                    "Адрес сайта"
                },
                address,
                "Открыть",
            );
            return;
        }
        if v.mode == 6 {
            if f.app.remote.mode == 0 {
                self.photo_header(f, "Браузер", t);
                self.round_rect(12, 70, 216, 106, 20, t.surface);
                self.symbol(Symbol::Search, 120, 98, t.primary);
                self.text_center_raw(125, "Что найти?", 1, t.on_surface);
                self.text_center_raw(151, "Нажмите: поиск", 1, t.on_surface_variant);
            } else if f.app.remote.error == 3 && f.app.remote.mode != 1 {
                self.photo_header(f, "Поиск", t);
                for (row, line) in crate::apps::remote::wrap(f.app.remote.message)
                    .iter()
                    .take(4)
                    .enumerate()
                {
                    self.text_fitted_raw(12, 92 + row as i32 * 20, line, 1, 216, t.on_surface);
                }
                self.text_center_raw(198, "Нажмите: новый запрос", 1, t.on_surface_variant);
            } else {
                self.remote_app_page(f, t);
            }
            if f.app.remote.mode != 1 {
                self.browser_dock(v.tab, v.dock_focus, t);
            }
            return;
        }
        self.photo_header(f, "Браузер", t);
        if v.mode == 0 {
            self.round_rect(12, 70, 216, 106, 20, t.surface);
            self.symbol(Symbol::World, 120, 98, t.primary);
            self.text_center_raw(125, "Открыть сайт", 1, t.on_surface);
            self.text_center_raw(151, "Нажмите: адрес сайта", 1, t.on_surface_variant);
            for (row, line) in crate::apps::remote::wrap(v.message)
                .iter()
                .take(2)
                .enumerate()
            {
                self.text_fitted_raw(
                    12,
                    201 + row as i32 * 17,
                    line,
                    1,
                    216,
                    t.on_surface_variant,
                );
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
                    match index {
                        1 if !v.can_back => "Назад · недоступно",
                        2 if !v.can_forward => "Вперёд · недоступно",
                        _ => labels[index],
                    },
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
                if let Some(image) = crate::browser_service::mail_image(7) {
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
            // Position within this document page stays separate from the page
            // counter: long HTML pages otherwise give no scrolling feedback.
            if v.mode == 2 && v.total > 0 {
                let track = 160;
                let thumb = (track * i32::from(v.rows_len) / i32::from(v.total)).clamp(8, track);
                let top = (track - thumb) * i32::from(v.cursor)
                    / i32::from(v.total.saturating_sub(1).max(1));
                self.round_rect(236, 80, 2, track, 1, t.surface_high);
                self.round_rect(236, 80 + top, 2, thumb, 1, t.primary);
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
        if matches!(v.mode, 0 | 5) {
            self.browser_dock(v.tab, v.dock_focus, t);
        }
        if v.mode == 3 {
            self.text_fitted_raw(10, 260, v.message, 1, 220, t.on_surface_variant);
        }
    }
    fn browser_dock(&mut self, tab: u8, _focused: bool, t: Theme) {
        self.round_rect(0, 242, 240, 38, 0, t.bg);
        for (i, label) in ["Главная", "Поиск", "История"].iter().enumerate() {
            let selected = i == usize::from(tab);
            let x = 4 + i as i32 * 79;
            self.round_rect(
                x,
                246,
                74,
                30,
                10,
                if selected { t.primary } else { t.surface_high },
            );
            self.text_center_x(x + 37, 255, label, 1, Self::focus_ink(t, selected));
        }
    }
}
