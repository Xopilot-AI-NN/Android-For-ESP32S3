// Экран Settings; состояние поступает из контроллера приложения.
use super::*;
#[allow(unused_imports)]
use core::fmt::Write;
impl Surface {
    /// Отрисовывает текущее состояние приложения без операций ввода-вывода.
    pub(super) fn diagnostics_page(&mut self, f: MaterialFrame, t: Theme) {
        use crate::apps::*;
        let a = f.app;
        self.photo_header(f, LABELS[usize::from(a.id)], t);

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
}
