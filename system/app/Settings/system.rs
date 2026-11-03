// Модуль экранов настроек.
//! pages settings Material surface implementation.
use super::*;

impl Surface {
    /// Рисует экран системных настроек с актуальным состоянием и фокусом.
    pub(super) fn system_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("SYSTEM", t);
        let labels = [
            "DATE & TIME",
            "ABOUT",
            "DEVELOPER OPTIONS",
            "Имя устройства",
            "Диагностика",
            "Меню питания",
            "Память и swap",
            "BACK",
        ];
        let cur = f.cursor.min(7) as usize;
        let start = cur.saturating_sub(1).min(5);
        for row in 0..3 {
            let i = (start + row).min(7);
            self.wear_row(55 + row as i32 * 64, labels[i], i == cur, t);
        }
        self.scroll_indicator(f.cursor, 8, t);
    }

    /// Рисует экран редактора имени устройства с актуальным состоянием и фокусом.
    pub(super) fn device_name_page(&mut self, f: MaterialFrame, t: Theme) {
        let draft = core::str::from_utf8(&f.name_draft[..usize::from(f.name_draft_len).min(32)])
            .unwrap_or("");
        self.text_keyboard(f, t, "Имя устройства", draft, "OK");
    }

    /// Рисует настройки RTC: автоматический режим, час, минуты и часовой пояс.
    pub(super) fn date_time(&mut self, f: MaterialFrame, t: Theme) {
        self.title("DATE & TIME", t);
        let labels = [
            "Автовремя",
            "Час",
            "Минута",
            "Часовой пояс",
            "BACK",
        ];
        let cur = f.cursor.min(4) as usize;
        let start = cur.saturating_sub(1).min(2);
        for row in 0..3 {
            let i = (start + row).min(4);
            let y = 56 + row as i32 * 64;
            let tm = Self::time_text(f.time_minutes);
            let zone = alloc::format!("UTC{:+}", f.timezone_hours);
            let value = match i {
                0 => if f.auto_time { "ON" } else { "OFF" },
                1 | 2 => core::str::from_utf8(&tm).unwrap_or("--:--"),
                3 => &zone,
                _ => "К системе",
            };
            self.info_card(y, labels[i], value, i == cur, t);
        }
        if f.auto_time && !f.time_valid {
            self.text_center(251, "SYNCING OVER WI-FI / PHONE", 1, t.primary);
        }
        self.scroll_indicator(f.cursor, 5, t);
    }

    /// Рисует экран недоступного оборудования с актуальным состоянием и фокусом.
    pub(super) fn unavailable_page(&mut self, title: &str, icon: &str, body: &str, t: Theme) {
        self.title(title, t);
        self.circle(120, 103, 37, t.surface);
        self.symbol(
            if icon == "S" {
                Symbol::Sound
            } else {
                Symbol::Info
            },
            120,
            103,
            t.primary,
        );
        self.text_center(157, body, 1, t.on_surface);
        self.text_center(180, "NOT PRESENT ON THIS BOARD", 1, t.on_surface_variant);
        self.round_rect(45, 217, 150, 38, 19, t.surface_high);
        self.text_center(229, "BACK", 1, t.on_surface);
    }

    /// Рисует экран звука и вибрации с актуальным состоянием и фокусом.
    pub(super) fn sound_page(&mut self, _f: MaterialFrame, t: Theme) {
        self.unavailable_page("SOUND & VIBRATION", "S", "AUDIO HARDWARE", t);
    }

    /// Рисует экран управления колёсиком с актуальным состоянием и фокусом.
    pub(super) fn gestures_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "GESTURES", t);
        self.info_card(
            63,
            "Чувствительность",
            ["Низкая", "Обычная", "Высокая"][f.crown_sensitivity.min(2) as usize],
            f.cursor == 0,
            t,
        );
        self.info_card(
            130,
            "Направление",
            if f.crown_reverse {
                "Обратное"
            } else {
                "Обычное"
            },
            f.cursor == 1,
            t,
        );
        self.info_card(197, "BACK", "К настройкам", f.cursor == 2, t);
    }
    /// Рисует экран специальных возможностей с актуальным состоянием и фокусом.
    pub(super) fn accessibility_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "ACCESSIBILITY", t);
        self.info_card(
            72,
            "Контрастность",
            if f.high_contrast {
                "Повышенная"
            } else {
                "Обычная"
            },
            f.cursor == 0,
            t,
        );
        self.info_card(150, "BACK", "К настройкам", f.cursor == 1, t);
    }
    /// Рисует экран локальной безопасности с актуальным состоянием и фокусом.
    pub(super) fn security_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "SECURITY", t);
        let labels = [
            if f.has_pin {
                "Изменить PIN"
            } else {
                "Задать PIN"
            },
            "Автоблокировка",
            "Заблокировать",
            "Отключить PIN",
            "BACK",
        ];
        let cur = f.cursor.min(4) as usize;
        let start = cur.saturating_sub(1).min(2);
        for row in 0..3 {
            let index = start + row;
            let y = 62 + row as i32 * 62;
            if index == 1 {
                let value = if f.lock_timeout_secs == 30 {
                    alloc::string::String::from("30 секунд")
                } else {
                    alloc::format!("{} мин", f.lock_timeout_secs / 60)
                };
                self.info_card(y, "Автоблокировка", &value, index == cur, t);
                continue;
            }
            self.wear_row(y, labels[index], index == cur, t);
        }
        let message = if f.security_pending {
            "Сохранение настроек..."
        } else {
            match f.security_message {
                1 => "Неверный PIN",
                2 => "PIN не совпали",
                3 => "Слишком много попыток",
                4 => "Нет хранилища для PIN",
                5 => "PIN задан",
                6 => "PIN отключён",
                7 => "Дождитесь сохранения",
                _ => "PIN защищает доступ к часам",
            }
        };
        self.text_center(262, message, 1, t.on_surface_variant);
        self.scroll_indicator(f.cursor, 5, t);
    }
    /// Рисует ввод PIN, защитное ожидание либо настройки срока блокировки.
    pub(super) fn pin_entry(&mut self, f: MaterialFrame, t: Theme) {
        if f.security_view == 7 {
            self.symbol(Symbol::Lock, 120, 65, t.error);
            self.text_center(115, "Ошибка хранилища PIN", 1, t.error);
            self.text_center(156, "Восстановите резервную", 1, t.on_surface);
            self.text_center(178, "копию образа системы", 1, t.on_surface);
            return;
        }
        if f.security_view == 5 {
            self.photo_header(
                f,
                if f.timeout_is_display {
                    "Сон экрана"
                } else {
                    "Автоблокировка"
                },
                t,
            );
            let label = if f.pin_digit == 0 {
                alloc::string::String::from("30 секунд")
            } else {
                alloc::format!("{} мин", f.pin_digit)
            };
            self.text_center(116, &label, 2, t.primary);
            self.text_center(175, "Вращайте для выбора", 1, t.on_surface_variant);
            self.info_card(216, "Применить", "Нажмите колёсико", true, t);
            return;
        }
        self.symbol(Symbol::Lock, 120, 35, t.primary);
        self.text_center(
            65,
            match f.security_view {
                1 => "Новый PIN",
                2 => "Повторите PIN",
                3 => "Введите PIN",
                _ => "Текущий PIN",
            },
            2,
            t.on_surface,
        );
        for i in 0..4 {
            self.circle(
                87 + i * 22,
                108,
                if i < i32::from(f.pin_count) { 6 } else { 3 },
                t.primary,
            );
        }
        if f.lockout_secs > 0 {
            self.text_center(
                150,
                &alloc::format!("Повтор через {} с", f.lockout_secs),
                1,
                t.error,
            );
        } else {
            self.clock_text(143, &[b'0' + f.pin_digit.min(9)], t.primary);
        }
        self.text_center(
            218,
            if f.security_message == 1 {
                "Неверный PIN"
            } else if f.security_message == 2 {
                "PIN не совпали"
            } else {
                "Вращайте: цифра"
            },
            1,
            t.on_surface_variant,
        );
        self.text_center(243, "Нажмите: выбрать", 1, t.on_surface_variant);
        self.text_center(264, "Удержание: стереть", 1, t.on_surface_variant);
    }

    /// Рисует экран приложений и уведомлений с актуальным состоянием и фокусом.
    pub(super) fn apps_notifications_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("APPS & NOTIFS", t);
        let labels = ["NOTIFICATIONS", "APP INFO", "BACK"];
        let cur = f.cursor.min(2) as usize;
        for i in 0..3 {
            let y = 57 + i as i32 * 64;
            if i == 0 {
                let value = alloc::format!("{}", f.notes);
                self.info_card(y, labels[i], &value, i == cur, t);
            } else if i == 1 {
                let value = alloc::format!("{} приложений", crate::apps::COUNT);
                self.info_card(y, labels[i], &value, i == cur, t);
            } else {
                self.wear_row(y, labels[i], i == cur, t);
            }
        }
    }

    /// Рисует экран настроек разработчика с актуальным состоянием и фокусом.
    pub(super) fn developer_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("DEVELOPER OPTIONS", t);
        let labels = ["WIRELESS DEBUG", "BUILD NUMBER", "SYSTEM", "BACK"];
        let cur = f.cursor.min(3) as usize;
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let i = (start + row).min(3);
            let y = 57 + row as i32 * 64;
            if i == 0 {
                self.info_card(y, labels[i], if f.adb { "ON" } else { "OFF" }, i == cur, t);
            } else if i == 1 {
                self.info_card(y, labels[i], "17.2.1", i == cur, t);
            } else {
                self.wear_row(y, labels[i], i == cur, t);
            }
        }
        if f.adb {
            self.text_center(253, "WIRELESS ADB IS A DEVELOPER FEATURE", 1, t.error);
        }
        self.scroll_indicator(f.cursor, 4, t);
    }
}
