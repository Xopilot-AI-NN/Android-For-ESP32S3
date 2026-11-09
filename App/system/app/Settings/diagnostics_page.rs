// All values come from cached snapshots; diagnostics does not query hardware while painting.
use super::*;
use core::fmt::Write;
impl Surface {
    pub(super) fn diagnostics_page(&mut self, f: MaterialFrame, t: Theme) {
        use crate::apps::*;
        let a = f.app;
        let stats = storage::display_snapshot();
        let titles = [
            "Система",
            "ОЗУ",
            "Накопитель",
            "Кадры",
            "Датчики",
            "Дисплей",
            "Рендеринг",
            "Сеть и время",
            "Swap и Gateway",
        ];
        self.photo_header(f, titles[usize::from(a.cursor).min(8)], t);
        let mut lines: [heapless::String<80>; 5] =
            core::array::from_fn(|_| heapless::String::new());
        match a.cursor {
            0 => {
                let _ = write!(
                    lines[0],
                    "Работает: {} ч {} мин",
                    a.uptime / 3600,
                    (a.uptime / 60) % 60
                );
                let _ = lines[1].push_str("ESP32-S3 · CPU 240 МГц");
                let _ = lines[2].push_str("Flash 4 МиБ · PSRAM 2 МиБ");
                let _ = write!(lines[3], "Приложений: {}", COUNT);
                let _ = lines[4].push_str("Плата ESP32-S3-Zero N4R2");
            }
            1 => {
                let _ = write!(lines[0], "Свободно: {} КиБ", a.free_kib);
                let _ = write!(lines[1], "Внутренняя: {} КиБ", a.internal_kib);
                let _ = write!(
                    lines[2],
                    "Внешняя: {} КиБ",
                    a.free_kib.saturating_sub(a.internal_kib)
                );
                let _ = lines[3].push_str(if a.internal_kib < 16 {
                    "Мало внутренней памяти"
                } else {
                    "Внутренняя память доступна"
                });
                let _ = lines[4].push_str("Swap не увеличивает DMA RAM");
            }
            2 => {
                let (disk, user, system, _) = storage::snapshot();
                let _ = write!(lines[0], "Носитель: {} МиБ", disk);
                let _ = write!(lines[1], "userdata: {} МиБ", user);
                let _ = write!(lines[2], "super: {} МиБ", system);
                let _ = lines[3].push_str(if storage::ready() {
                    "GPT · метаданные прочитаны"
                } else {
                    "Чтение сведений…"
                });
                let _ = lines[4].push_str("Размеры разделов, не остаток");
            }
            3 => {
                let _ = write!(lines[0], "Последний кадр: {} мс", stats.last_us / 1000);
                let _ = write!(lines[1], "Максимум: {} мс", stats.max_us / 1000);
                let _ = write!(lines[2], "Нарисовано: {}", stats.drawn);
                let _ = write!(lines[3], "Без изменений: {}", stats.skipped);
                let _ = write!(
                    lines[4],
                    "Обновлено: {} %",
                    stats.pixels.saturating_mul(100) / (240 * 280)
                );
            }
            4 => {
                if let Some(value) = a.temperature_deci_c {
                    let _ = write!(
                        lines[0],
                        "Чип: {}{}.{:01} °C",
                        if value < 0 { "−" } else { "" },
                        value.abs() / 10,
                        value.abs() % 10
                    );
                } else {
                    let _ = lines[0].push_str("Датчик чипа недоступен");
                }
                let _ = lines[1].push_str("Температура внутри ESP32-S3");
                let _ = lines[2].push_str("Обновляется раз в секунду");
                let _ = lines[3].push_str("Ток: нет датчика");
                let _ = lines[4].push_str("Заряд: нет измерителя");
            }
            5 => {
                #[cfg(feature = "display-st7789")]
                {
                    let _ = lines[0].push_str("ST7789 · 240×280 · 16 бит");
                    let _ = write!(
                        lines[1],
                        "Панель: ~{} Гц, номинал",
                        crate::config::TFT_NOMINAL_HZ
                    );
                    let _ = write!(lines[2], "SPI: {} МГц", crate::config::TFT_MHZ);
                    let tenths = crate::config::TFT_MHZ * 10_000_000 / (240 * 280 * 16);
                    let _ = write!(
                        lines[3],
                        "Весь кадр: до {}.{} FPS",
                        tenths / 10,
                        tenths % 10
                    );
                    let _ = lines[4].push_str("Предел SPI, без отрисовки");
                }
                #[cfg(not(feature = "display-st7789"))]
                {
                    let _ = lines[0].push_str("Дисплей: сборка SSD1306");
                }
            }
            6 => {
                let _ = write!(lines[0], "Растеризация: {} мкс", stats.raster_us);
                let _ = write!(lines[1], "Композиция: {} мкс", stats.compose_us);
                let _ = write!(lines[2], "Передача: {} мкс", stats.transfer_us);
                let _ = write!(lines[3], "Rhai: {} мкс", a.script_us);
                let _ = write!(lines[4], "Попаданий кэша: {}", a.cache_hits);
            }
            7 => {
                let _ = lines[0].push_str(if f.wifi_link_state == 4 {
                    "Wi-Fi подключён"
                } else {
                    "Wi-Fi не подключён"
                });
                if let Some(ip) = f.wifi_ip {
                    let _ = write!(lines[1], "IP: {}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
                } else {
                    let _ = lines[1].push_str("IP не получен");
                }
                let _ = write!(
                    lines[2],
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
                let _ = lines[3].push_str(if crate::rtc::valid() {
                    "Часы синхронизированы"
                } else {
                    "Время не установлено"
                });
                let _ = write!(lines[4], "UTC {:+} мин", crate::rtc::timezone_minutes());
            }
            _ => {
                let _ = write!(lines[0], "Swap активен: {} МиБ", a.memory.active);
                let _ = write!(lines[1], "Выбрано: {} МиБ", a.memory.selected);
                let _ = write!(lines[2], "Доступно до: {} МиБ", a.memory.maximum);
                let gateway = crate::google_gateway::snapshot();
                let _ = lines[3].push_str(if gateway.online {
                    "Gateway: сеть доступна"
                } else {
                    "Gateway: нет сети"
                });
                let _ = lines[4].push_str(gateway.message);
            }
        }
        for (row, line) in lines.iter().enumerate() {
            self.utility_line(row as i32, line, t);
        }
        self.scroll_indicator(a.cursor, DIAGNOSTIC_PAGES, t);
        self.text_center(259, "Колесо: раздел · Назад", 1, t.on_surface_variant);
    }
}
