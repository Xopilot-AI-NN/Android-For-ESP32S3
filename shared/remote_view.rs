//! Only bounded text and QR modules reach the renderer. Mail/HTML never become UI code.
use super::*;
impl Surface {
    fn google_button(&mut self, y: i32, label: &str, selected: bool, t: Theme) {
        self.action_chip(y, 32, label, selected, t);
    }
    pub(super) fn remote_app_page(&mut self, mut f: MaterialFrame, t: Theme) {
        let v = f.app.remote;
        let title = match v.kind {
            0 => {
                if v.provider_len > 0 {
                    core::str::from_utf8(&v.provider[..v.provider_len as usize]).unwrap_or("Поиск")
                } else {
                    "Google"
                }
            }
            1 => "События",
            2 => "Gmail",
            4 => "Контакты Google",
            5 => "Google Фото",
            6 => "Сообщения",
            _ => "Переводчик",
        };
        let text = core::str::from_utf8(&v.text[..v.len as usize]).unwrap_or("");
        if v.mode == 1 {
            f.keyboard = v.keyboard;
            f.keyboard_error = v.error;
            self.text_keyboard(f, t, title, text, "Готово");
            return;
        }
        self.photo_header(
            f,
            if v.mode == 3 || v.mode == 6 {
                if v.phase == crate::google_gateway::Phase::Waiting as u8 {
                    "Сканируйте QR"
                } else {
                    "Аккаунт Google"
                }
            } else {
                title
            },
            t,
        );
        if v.mode == 7 {
            self.text_center_raw(64, "Выберите фото в браузере", 1, t.on_surface_variant);
            if v.qr_size > 0 {
                let n = usize::from(v.qr_size);
                let scale = 160 / (n + 8);
                let size = (n + 8) * scale;
                let left = (240 - size as i32) / 2;
                self.round_rect(left, 87, size as i32, size as i32, 0, rgb565(255, 255, 255));
                for y in 0..n {
                    for x in 0..n {
                        let bit = y * n + x;
                        if v.qr[bit / 8] & (1 << (bit % 8)) != 0 {
                            self.round_rect(
                                left + ((x + 4) * scale) as i32,
                                87 + ((y + 4) * scale) as i32,
                                scale as i32,
                                scale as i32,
                                0,
                                0,
                            );
                        }
                    }
                }
            }
            self.text_center_raw(255, "или localhost:8080 на ПК", 1, t.on_surface_variant);
            return;
        }
        if v.mode == 3 {
            use crate::google_gateway::Phase;
            let phase = v.phase;
            if phase == Phase::Waiting as u8 && v.qr_size > 0 {
                // Four white modules of quiet zone, integer scaling: no antialiasing on QR.
                let n = i32::from(v.qr_size);
                let scale = 200 / (n + 8);
                let size = (n + 8) * scale;
                let x = (240 - size) / 2;
                let y = 49 + (200 - size) / 2;
                self.round_rect(x, y, size, size, 0, rgb565(255, 255, 255));
                for row in 0..n {
                    for col in 0..n {
                        let bit = (row * n + col) as usize;
                        if v.qr[bit / 8] & (1 << (bit % 8)) != 0 {
                            self.round_rect(
                                x + (col + 4) * scale,
                                y + (row + 4) * scale,
                                scale,
                                scale,
                                0,
                                0,
                            );
                        }
                    }
                }
                self.google_button(246, "Отмена", true, t);
                return;
            }
            if phase == Phase::Confirm as u8 {
                let email = core::str::from_utf8(&v.email[..v.email_len as usize]).unwrap_or("");
                self.symbol(Symbol::Person, 120, 88, t.primary);
                self.text_fitted_raw(10, 119, email, 1, 220, t.on_surface);
                let mail = match v.services & 3 {
                    3 => "Календарь · Gmail",
                    1 => "Календарь",
                    2 => "Gmail",
                    _ => "Доступ только на чтение",
                };
                self.text_center_raw(145, mail, 1, t.on_surface_variant);
                let extra = match v.services & 12 {
                    12 => "Контакты · Google Фото",
                    4 => "Контакты Google",
                    8 => "Выбранные Google Фото",
                    _ => "",
                };
                self.text_center_raw(167, extra, 1, t.on_surface_variant);
                self.google_button(195, "Подтвердить", v.cursor % 2 == 0, t);
                self.google_button(237, "Отмена", v.cursor % 2 == 1, t);
                return;
            }
            if v.kind >= 4 {
                self.symbol(Symbol::Person, 120, 90, t.primary);
                self.text_center_raw(
                    127,
                    if v.kind == 4 {
                        "Доступ к контактам"
                    } else {
                        "Доступ к выбранным фото"
                    },
                    1,
                    t.on_surface_variant,
                );
                self.google_button(177, "Разрешить в Google", v.cursor == 0, t);
                self.google_button(219, "Назад", v.cursor == 1, t);
                self.text_fitted_raw(10, 258, v.message, 1, 220, t.on_surface_variant);
                return;
            }
            let connected = phase == Phase::Connected as u8;
            let email = core::str::from_utf8(&v.email[..v.email_len as usize]).unwrap_or("");
            self.text_fitted_raw(
                10,
                54,
                if connected {
                    email
                } else {
                    "Google не подключён"
                },
                1,
                220,
                t.on_surface_variant,
            );
            let labels = if connected {
                [
                    "Переподключить Calendar",
                    "Подключить Gmail",
                    "Синхронизировать",
                    "Выйти",
                ]
            } else {
                [
                    "Подключить Calendar",
                    "Подключить Gmail",
                    "Подключить оба сервиса",
                    "Назад",
                ]
            };
            if connected && v.sync_len > 0 {
                self.text_fitted_raw(
                    10,
                    86,
                    core::str::from_utf8(&v.sync[..v.sync_len as usize]).unwrap_or(""),
                    1,
                    220,
                    t.on_surface_variant,
                );
            }
            for (i, label) in labels.iter().enumerate() {
                self.google_button(110 + i as i32 * 36, label, v.cursor as usize == i, t);
            }
            if connected {
                self.text_fitted_raw(
                    10,
                    70,
                    &alloc::format!(
                        "Calendar: {} · Gmail: {}",
                        if v.services & 1 != 0 {
                            "да"
                        } else {
                            "нет"
                        },
                        if v.services & 2 != 0 {
                            "да"
                        } else {
                            "нет"
                        }
                    ),
                    1,
                    220,
                    t.on_surface_variant,
                );
            }
            self.text_fitted_raw(10, 258, v.message, 1, 220, t.on_surface_variant);
            return;
        }
        if v.kind == 0 && v.mode == 0 {
            self.round_rect(12, 66, 216, 49, 20, t.surface_high);
            self.symbol(Symbol::Search, 37, 90, t.primary);
            self.text_fitted_raw(58, 83, "Что найти?", 1, 155, t.on_surface);
            self.google_button(132, "Новый запрос", v.cursor == 0, t);
            self.google_button(174, "Повторить поиск", v.cursor == 1, t);
            self.google_button(216, "Аккаунт Google", v.cursor == 2, t);
            self.text_fitted_raw(
                12,
                263,
                if v.busy {
                    "Загрузка…"
                } else {
                    v.message
                },
                1,
                216,
                t.on_surface_variant,
            );
            return;
        }
        if v.mode == 6 {
            self.text_center_raw(100, "Отключить аккаунт?", 1, t.on_surface);
            self.text_center_raw(129, "Кэш будет очищен", 1, t.on_surface_variant);
            self.google_button(180, "Выйти", v.cursor == 0, t);
            self.google_button(223, "Отмена", v.cursor != 0, t);
            return;
        }
        if v.mode == 5 {
            if !v.month_valid {
                self.text_center_raw(127, "Синхронизируйте дату", 1, t.on_surface_variant);
                return;
            }
            use crate::apps::utilities::{MONTHS, days_in_month, first_weekday};
            let year = v.month.div_euclid(12);
            let month = v.month.rem_euclid(12) as u8 + 1;
            self.text_center_raw(
                63,
                &alloc::format!("{} {year}", MONTHS[month as usize - 1]),
                1,
                t.primary,
            );
            for (i, label) in ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"]
                .iter()
                .enumerate()
            {
                self.text_center_x(24 + i as i32 * 32, 95, label, 1, t.on_surface_variant);
            }
            let offset = first_weekday(year, month);
            for day in 1..=days_in_month(year, month) {
                let pos = i32::from(offset + day - 1);
                let (x, y) = (24 + pos % 7 * 32, 121 + pos / 7 * 21);
                self.text_center_x(x, y, &alloc::format!("{day}"), 1, t.on_surface);
                if v.days & (1 << (day - 1)) != 0 {
                    self.circle(x, y + 16, 2, t.primary);
                }
            }
            self.google_button(244, "События месяца", true, t);
            return;
        }
        if v.mode == 2 || v.mode == 4 {
            self.text_fitted_raw(
                10,
                60,
                if v.busy {
                    "Загрузка…"
                } else if v.cached {
                    "Офлайн · последний кэш"
                } else {
                    v.message
                },
                1,
                220,
                t.on_surface_variant,
            );
            if v.mode == 2 {
                for i in 0..3 {
                    let index = i * 2;
                    if v.lengths[index] == 0 {
                        continue;
                    }
                    let selected = (v.offset as usize / 2 + i) == v.cursor as usize;
                    let y = 84 + i as i32 * 54;
                    self.round_rect(
                        8,
                        y,
                        224,
                        48,
                        13,
                        if selected { t.primary } else { t.surface_high },
                    );
                    let ink = Self::focus_ink(t, selected);
                    let title = core::str::from_utf8(&v.lines[index][..v.lengths[index] as usize])
                        .unwrap_or("");
                    let subtitle =
                        core::str::from_utf8(&v.lines[index + 1][..v.lengths[index + 1] as usize])
                            .unwrap_or("");
                    self.text_fitted_raw(18, y + 6, title, 1, 204, ink);
                    self.text_fitted_raw(18, y + 28, subtitle, 1, 204, ink);
                }
                if v.count == 0 {
                    self.text_center_raw(
                        131,
                        if v.busy {
                            "Подождите"
                        } else {
                            "Список пуст"
                        },
                        1,
                        t.on_surface_variant,
                    );
                }
                if !matches!(v.kind, 2 | 4 | 5 | 6) && v.lengths[6] != 0 {
                    let text =
                        core::str::from_utf8(&v.lines[6][..v.lengths[6] as usize]).unwrap_or("");
                    self.google_button(246, text, v.cursor >= v.count, t);
                }
            } else if v.image {
                if let Some(image) = crate::google_gateway::mail_image(v.kind) {
                    let x0 = (240 - image.width as i32 * 2) / 2;
                    let y0 = 84 + (160 - image.height as i32 * 2) / 2;
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
                self.text_fitted_raw(
                    10,
                    257,
                    "Колесо: дальше / назад",
                    1,
                    220,
                    t.on_surface_variant,
                );
            } else {
                for (i, line) in v.lines.iter().enumerate() {
                    let text = core::str::from_utf8(&line[..v.lengths[i] as usize]).unwrap_or("");
                    self.text_raw(10, 87 + i as i32 * 23, text, 1, t.on_surface);
                }
                self.text_fitted_raw(
                    10,
                    257,
                    &alloc::format!(
                        "{}-{} / {}",
                        if v.total_lines == 0 { 0 } else { v.offset + 1 },
                        (v.offset + 7).min(v.total_lines),
                        v.total_lines
                    ),
                    1,
                    158,
                    t.on_surface_variant,
                );
                if v.more && !matches!(v.kind, 2 | 5) {
                    self.round_rect(181, 248, 49, 29, 12, t.primary);
                    self.symbol(Symbol::Next, 205, 262, t.on_primary);
                }
            }
            return;
        }
        self.round_rect(12, 62, 216, 44, 16, t.surface);
        self.symbol(
            match v.kind {
                0 => Symbol::Search,
                1 => Symbol::Clock,
                2 => Symbol::Share,
                _ => Symbol::Language,
            },
            34,
            84,
            t.primary,
        );
        self.text_fitted_raw(
            56,
            77,
            match v.kind {
                0 => "Текстовый поиск",
                1 => "События Google",
                2 => "Почта Google",
                4 => "Телефоны и адреса",
                5 => "Фото, выбранные вами",
                _ => "Текст и языки",
            },
            1,
            160,
            t.on_surface_variant,
        );
        let languages = [
            "RU → EN",
            "EN → RU",
            "Авто → EN",
            "Авто → RU",
            "Авто → DE",
            "Авто → FR",
            "Авто → ES",
        ];
        let labels = match v.kind {
            0 => ["Введите запрос", "Обновить поиск", "Аккаунт Google", ""],
            1 => ["Месяц", "События", "Обновить", "Аккаунт Google"],
            2 => ["Входящие", "Обновить", "Аккаунт Google", ""],
            4 => ["Мои контакты", "Обновить", "Доступ Google", ""],
            5 => ["Выбрать фото", "Выбранные фото", "Доступ Google", ""],
            _ => [
                "Введите текст",
                languages[v.language as usize % 7],
                "Аккаунт Google",
                "",
            ],
        };
        for (i, label) in labels.iter().enumerate() {
            if !label.is_empty() {
                self.google_button(116 + i as i32 * 36, label, v.cursor as usize == i, t);
            }
        }
        self.text_fitted_raw(10, 263, v.message, 1, 220, t.on_surface_variant);
    }
}
