//! Small crown-driven utilities. No network or storage I/O occurs while painting.
use super::*;
use core::fmt::Write;
impl Surface {
    fn utility_line(&mut self, row: i32, text: &str, t: Theme) {
        self.text_center_raw(77 + row * 34, text, 1, t.on_surface);
    }
    pub(super) fn native_application(&mut self, f: MaterialFrame, t: Theme) {
        use crate::apps::*;
        let a = f.app;
        if a.id == WEATHER {
            self.weather_page(f, t);
            return;
        }
        if a.id == WORLD {
            self.world_page(f, t);
            return;
        }
        if a.id == BREATHING {
            self.breathing_page(f, t);
            return;
        }
        self.photo_header(f, LABELS[usize::from(a.id)], t);
        match a.id {
            CALCULATOR => {
                let value =
                    core::str::from_utf8(&a.text[..usize::from(a.len).min(32)]).unwrap_or("Ошибка");
                self.round_rect(12, 53, 216, 40, 16, t.surface);
                self.text_center_raw(67, value, if value.len() > 12 { 1 } else { 2 }, t.primary);
                for (index, label) in calculator::KEYS.iter().enumerate() {
                    let x = 12 + (index % 4) as i32 * 55;
                    let y = 103 + (index / 4) as i32 * 33;
                    self.keyboard_key(x, y, 51, 29, label, index == usize::from(a.cursor), t);
                }
            }
            FILES => {
                self.text_center(54, "Обои системы · PNG", 1, t.primary);
                let cur = usize::from(a.cursor);
                let count = usize::from(crate::wallpaper::builtin_count());
                let start = cur.saturating_sub(1).min(count.saturating_sub(3));
                for row in 0..3 {
                    let index = start + row;
                    self.wear_row(
                        73 + row as i32 * 57,
                        crate::wallpaper::name(index as u8 + 1),
                        index == cur,
                        t,
                    );
                }
                self.scroll_indicator(a.cursor, count as u8, t);
                self.text_center(
                    250,
                    if f.wallpaper_enabled && f.wallpaper_index == a.cursor + 1 {
                        "Установлены на циферблат"
                    } else {
                        "Нажать: установить обои"
                    },
                    1,
                    t.on_surface_variant,
                );
            }
            SHARE => {
                let ip = f.wifi_ip;
                if let Some([a, b, c, d]) = ip {
                    let mut line = heapless::String::<48>::new();
                    let _ = write!(line, "{}.{}.{}.{}:5555", a, b, c, d);
                    self.utility_line(0, &line, t);
                    match f.app.cursor {
                        0 => {
                            self.utility_line(1, "ADB по Wi-Fi", t);
                            self.utility_line(2, "adb connect IP:5555", t);
                            self.utility_line(
                                3,
                                if f.adb {
                                    "ADB включён"
                                } else {
                                    "Включите ADB в настройках"
                                },
                                t,
                            );
                        }
                        1 => {
                            self.utility_line(1, "Загрузить PNG 240x280", t);
                            self.utility_line(2, "adb push image.png", t);
                            self.utility_line(3, "/sdcard/Wallpapers/", t);
                        }
                        _ => {
                            self.utility_line(1, "Скачать системные обои", t);
                            self.utility_line(2, "adb pull", t);
                            self.utility_line(3, "/sdcard/Wallpapers/1.png", t);
                        }
                    }
                } else {
                    self.utility_line(1, "Подключитесь к Wi-Fi", t);
                    self.utility_line(2, "Оба устройства в одной сети", t);
                }
                self.text_center(253, "Колесо: инструкция 1–3", 1, t.on_surface_variant);
            }
            MEMORY => {
                let memory = a.memory;
                let mut active = heapless::String::<64>::new();
                let _ = write!(
                    active,
                    "Активно {} / выбрано {} МиБ",
                    memory.active, memory.selected
                );
                self.text_center_raw(55, &active, 1, t.on_surface_variant);
                let start = usize::from(a.cursor).saturating_sub(1).min(3);
                for row in 0..2 {
                    let index = start + row;
                    let size = crate::swap_config::CHOICES[index];
                    let mut label = heapless::String::<24>::new();
                    let _ = write!(label, "{} МиБ", size);
                    let value = if size > memory.maximum {
                        "Недостаточно места"
                    } else if size == memory.selected {
                        "Выбрано"
                    } else {
                        "Нажмите для выбора"
                    };
                    self.info_card(
                        82 + row as i32 * 65,
                        &label,
                        value,
                        index == usize::from(a.cursor),
                        t,
                    );
                }
                self.scroll_indicator(a.cursor, 5, t);
                let status = match memory.message {
                    1 => "Сохранение…",
                    2 => "Применится после перезагрузки",
                    3 => "Недостаточно места на носителе",
                    4 => "Нет подходящей области userdata",
                    5 => "Не удалось сохранить настройку",
                    _ => "ZPager: хранилище страниц",
                };
                self.text_center(225, status, 1, t.primary);
                self.text_center(253, "Это не дополнительная RAM", 1, t.on_surface_variant);
            }
            DIAGNOSTICS => {
                let mut line = heapless::String::<64>::new();
                match a.cursor {
                    0 => {
                        let _ = write!(
                            line,
                            "Работает: {} ч {} мин",
                            a.uptime / 3600,
                            (a.uptime / 60) % 60
                        );
                        self.utility_line(0, &line, t);
                        self.utility_line(1, "ESP32-S3 · CPU 240 MHz", t);
                        self.utility_line(2, "Flash 4 МиБ · PSRAM 2 МиБ", t);
                        self.utility_line(3, "Экран ST7789 · 240x280", t);
                    }
                    1 => {
                        let _ = write!(line, "Свободно: {} КиБ", a.free_kib);
                        self.utility_line(0, &line, t);
                        line.clear();
                        let _ = write!(line, "Внутренняя: {} КиБ", a.internal_kib);
                        self.utility_line(1, &line, t);
                        self.utility_line(2, "Остальное: внешняя PSRAM", t);
                        self.utility_line(
                            3,
                            if a.internal_kib < 16 {
                                "Мало внутренней памяти"
                            } else {
                                "Память доступна"
                            },
                            t,
                        );
                    }
                    2 => {
                        let (disk, user, system, _) = storage::snapshot();
                        let _ = write!(line, "Носитель: {} МиБ", disk);
                        self.utility_line(0, &line, t);
                        line.clear();
                        let _ = write!(line, "userdata: {} МиБ", user);
                        self.utility_line(1, &line, t);
                        line.clear();
                        let _ = write!(line, "super: {} МиБ", system);
                        self.utility_line(2, &line, t);
                        self.utility_line(
                            3,
                            if storage::ready() {
                                "GPT · чтение метаданных"
                            } else {
                                "Чтение сведений…"
                            },
                            t,
                        );
                    }
                    3 => {
                        let (_, _, _, frame_us) = storage::snapshot();
                        let _ = write!(line, "Кадр: {} мс", frame_us / 1000);
                        self.utility_line(0, &line, t);
                        line.clear();
                        let _ = write!(line, "Rhai: {} мкс", a.script_us);
                        self.utility_line(1, &line, t);
                        line.clear();
                        let _ = write!(line, "Попаданий кэша: {}", a.cache_hits);
                        self.utility_line(2, &line, t);
                        self.utility_line(
                            3,
                            if f.wifi_link_state == 4 {
                                "Wi-Fi подключён"
                            } else {
                                "Wi-Fi не подключён"
                            },
                            t,
                        );
                        line.clear();
                        let _ = write!(
                            line,
                            "BLE: {}",
                            match f.bt_status {
                                0 => "выключен",
                                1 => "доступен",
                                2 => "подключён",
                                3 => "готов",
                                4 => "подтвердите PIN",
                                5 => "ошибка",
                                6 => "сопряжение",
                                _ => "запуск",
                            }
                        );
                        self.utility_line(4, &line, t);
                    }
                    _ => {
                        self.utility_line(0, "Температура: нет драйвера", t);
                        self.utility_line(1, "Ток: нет датчика", t);
                        self.utility_line(2, "Заряд: нет измерителя", t);
                        self.utility_line(3, "Измерьте ток питания", t);
                        self.utility_line(4, "LCD AOD включает подсветку", t);
                    }
                }
                self.scroll_indicator(a.cursor, 5, t);
                self.text_center(
                    259,
                    "Колесо: раздел · Назад: выход",
                    1,
                    t.on_surface_variant,
                );
            }
            _ => {}
        }
    }
}
