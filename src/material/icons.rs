// Модуль Material-пиктограмм.
//! icons Material surface implementation.
use super::*;

impl Surface {
    /// Tint Google's checked-in SVG coverage directly into the RGB565 surface.
    pub(super) fn symbol(&mut self, symbol: Symbol, cx: i32, cy: i32, color: u16) {
        if let Some(atlas) = crate::ui_resources::icons(false) {
            self.symbol_mask(symbol, cx, cy, color, &atlas, super::symbols::SIZE);
        }
    }

    /// Смешивает маску пиктограммы с кадром и учитывает старые атласы.
    fn symbol_mask(
        &mut self,
        symbol: Symbol,
        cx: i32,
        cy: i32,
        color: u16,
        atlas: &[u8],
        size: usize,
    ) {
        // New app symbols are appended; older system packs retain valid IDs.
        let symbol = if (symbol as usize + 1) * size * size > atlas.len() * 2 {
            match symbol {
                Symbol::Weather => Symbol::Sun,
                Symbol::World => Symbol::Clock,
                Symbol::Breathing => Symbol::Person,
                Symbol::Memory => Symbol::Developer,
                Symbol::Shift => Symbol::Previous,
                Symbol::Backspace => Symbol::Back,
                Symbol::Language => Symbol::Settings,
                Symbol::Space => Symbol::Next,
                Symbol::Done => Symbol::Play,
                _ => return,
            }
        } else {
            symbol
        };
        let base = symbol as usize * size * size;
        for row in 0..size {
            for col in 0..size {
                let offset = base + row * size + col;
                let packed = atlas[offset / 2];
                let alpha = u32::from(if offset % 2 == 0 {
                    packed >> 4
                } else {
                    packed & 15
                });
                let x = cx - size as i32 / 2 + col as i32;
                let y = cy - size as i32 / 2 + row as i32;
                if alpha == 0 || x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
                    continue;
                }
                if alpha == 15 {
                    self.pixel(x, y, color);
                    continue;
                }
                let bg = u32::from(self.pixels[y as usize * WIDTH + x as usize]);
                let blend = |shift: u32, mask: u32| -> u16 {
                    (((((u32::from(color) >> shift) & mask) * alpha
                        + ((bg >> shift) & mask) * (15 - alpha)
                        + 7)
                        / 15)
                        << shift) as u16
                };
                self.pixel(x, y, blend(11, 31) | blend(5, 63) | blend(0, 31));
            }
        }
    }
    /// Рисует пиктограмму Wi-Fi.
    pub(super) fn icon_wifi(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Wifi, cx, cy, c);
    }

    /// Рисует пиктограмму Bluetooth.
    pub(super) fn icon_bt(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Bluetooth, cx, cy, c);
    }

    /// Рисует пиктограмму «Не беспокоить».
    pub(super) fn icon_dnd(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Dnd, cx, cy, c);
    }

    /// Рисует пиктограмму авиарежима.
    pub(super) fn icon_airplane(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Airplane, cx, cy, c);
    }

    /// Рисует пиктограмму яркости.
    pub(super) fn icon_sun(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Sun, cx, cy, c);
    }

    /// Рисует пиктограмму настроек.
    pub(super) fn icon_settings(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Settings, cx, cy, c);
    }

    /// Рисует пиктограмму уведомлений.
    pub(super) fn icon_bell(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Bell, cx, cy, c);
    }

    /// Рисует пиктограмму часов.
    pub(super) fn icon_clock(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Clock, cx, cy, c);
    }

    /// Рисует пиктограмму медиаплеера.
    pub(super) fn icon_media(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Play, cx, cy, c);
    }

    /// Рисует пиктограмму сведений.
    pub(super) fn icon_info(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Info, cx, cy, c);
    }

    /// Рисует пиктограмму возврата.
    pub(super) fn icon_back(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Back, cx, cy, c);
    }

    /// Рисует пиктограмму блокировки.
    pub(super) fn icon_lock(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Lock, cx, cy, c);
    }

    /// Рисует пиктограмму звука.
    pub(super) fn icon_sound(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Sound, cx, cy, c);
    }

    /// Рисует пиктограмму специальных возможностей.
    pub(super) fn icon_person(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Person, cx, cy, c);
    }
    /// Выбирает и рисует пиктограмму по стабильному ID приложения.
    pub(super) fn app_icon(&mut self, icon: u8, cx: i32, cy: i32, c: u16) {
        // These simple tool pictograms also work with older resource packages.
        match icon {
            crate::apps::CALENDAR | crate::apps::GOOGLE_CALENDAR => {
                self.line(cx - 11, cy - 9, cx + 11, cy - 9, 2, c);
                self.line(cx - 11, cy - 9, cx - 11, cy + 11, 2, c);
                self.line(cx + 11, cy - 9, cx + 11, cy + 11, 2, c);
                self.line(cx - 11, cy + 11, cx + 11, cy + 11, 2, c);
                self.line(cx - 11, cy - 3, cx + 11, cy - 3, 2, c);
                for x in [-5, 5] {
                    self.line(cx + x, cy - 13, cx + x, cy - 6, 2, c);
                    for y in [2, 7] {
                        self.circle(cx + x, cy + y, 1, c);
                    }
                }
                return;
            }
            crate::apps::GALLERY => {
                self.line(cx - 12, cy - 10, cx + 12, cy - 10, 2, c);
                self.line(cx - 12, cy - 10, cx - 12, cy + 10, 2, c);
                self.line(cx + 12, cy - 10, cx + 12, cy + 10, 2, c);
                self.line(cx - 12, cy + 10, cx + 12, cy + 10, 2, c);
                self.circle(cx + 6, cy - 4, 3, c);
                self.line(cx - 10, cy + 8, cx - 3, cy, 2, c);
                self.line(cx - 3, cy, cx + 5, cy + 8, 2, c);
                return;
            }
            crate::apps::BT_KEYBOARD => {
                self.round_rect(cx - 14, cy - 9, 28, 19, 3, c);
                let ink = rgb565(8, 20, 17);
                for row in 0..2 {
                    for col in 0..5 {
                        self.round_rect(cx - 10 + col * 5, cy - 5 + row * 5, 3, 3, 0, ink);
                    }
                }
                self.line(cx - 7, cy + 6, cx + 7, cy + 6, 2, ink);
                return;
            }
            crate::apps::GMAIL => {
                self.line(cx - 13, cy - 9, cx + 13, cy - 9, 2, c);
                self.line(cx - 13, cy + 9, cx + 13, cy + 9, 2, c);
                self.line(cx - 13, cy - 9, cx - 13, cy + 9, 2, c);
                self.line(cx + 13, cy - 9, cx + 13, cy + 9, 2, c);
                self.line(cx - 13, cy - 9, cx, cy + 1, 2, c);
                self.line(cx + 13, cy - 9, cx, cy + 1, 2, c);
                return;
            }
            crate::apps::CONVERTER => {
                for (y, direction) in [(-5, 1), (5, -1)] {
                    self.line(cx - 11, cy + y, cx + 11, cy + y, 2, c);
                    self.line(
                        cx + direction * 11,
                        cy + y,
                        cx + direction * 5,
                        cy + y - 5,
                        2,
                        c,
                    );
                    self.line(
                        cx + direction * 11,
                        cy + y,
                        cx + direction * 5,
                        cy + y + 5,
                        2,
                        c,
                    );
                }
                return;
            }
            crate::apps::COUNTER => {
                self.ring(cx, cy, 12, 2, c);
                self.line(cx - 6, cy, cx + 6, cy, 2, c);
                self.line(cx, cy - 6, cx, cy + 6, 2, c);
                return;
            }
            _ => {}
        }
        let symbol = match icon {
            0 => Symbol::Clock,
            1 => Symbol::Play,
            2 => Symbol::Settings,
            3 => Symbol::Connectivity,
            4 => Symbol::Info,
            5 => Symbol::Assistant,
            6 => Symbol::Calculator,
            7 => Symbol::Folder,
            8 => Symbol::Share,
            9 => Symbol::Diagnostics,
            10 => Symbol::Flashlight,
            11 => Symbol::Memory,
            12 => Symbol::Weather,
            13 => Symbol::World,
            14 => Symbol::Breathing,
            18 => Symbol::Info,
            19 => Symbol::Done,
            20 => Symbol::Calculator,
            21 => Symbol::Timer,
            22 => Symbol::Devices,
            25 => Symbol::Search,
            28 => Symbol::Language,
            _ => Symbol::Grid,
        };
        if let Some(atlas) = crate::ui_resources::icons(true) {
            self.symbol_mask(symbol, cx, cy, c, &atlas, super::symbols::LAUNCHER_SIZE);
        }
    }

    /// Возвращает цвет иконки приложения.
    pub(super) fn app_accent(icon: u8) -> u16 {
        match icon {
            0 => rgb565(181, 232, 211), // clock mint
            1 => rgb565(255, 191, 171), // media coral
            2 => rgb565(174, 203, 255), // settings blue
            3 => rgb565(102, 221, 205), // connectivity teal
            4 => rgb565(218, 190, 255), // system lavender
            5 => rgb565(126, 225, 165), // assistant green
            6 => rgb565(196, 224, 156),
            7 => rgb565(255, 218, 143),
            8 => rgb565(133, 216, 246),
            9 => rgb565(255, 184, 180),
            10 => rgb565(255, 239, 167),
            11 => rgb565(174, 203, 255),
            12 => rgb565(255, 214, 125),
            13 => rgb565(163, 212, 255),
            14 => rgb565(192, 171, 243),
            15 => rgb565(255, 186, 188),
            16 => rgb565(162, 224, 238),
            17 => rgb565(226, 218, 151),
            18 => rgb565(188, 225, 251),
            19 => rgb565(172, 232, 190),
            20 => rgb565(255, 214, 153),
            21 => rgb565(218, 195, 253),
            22 => rgb565(156, 219, 237),
            23 => rgb565(255, 193, 216),
            24 => rgb565(130, 235, 250),
            25 => rgb565(164, 204, 255),
            26 => rgb565(169, 211, 255),
            27 => rgb565(255, 184, 180),
            28 => rgb565(184, 223, 255),
            _ => rgb565(50, 59, 54),
        }
    }

    /// Выбирает значок по смыслу системной строки меню.
    pub(super) fn row_icon(&mut self, label: &str, cx: i32, cy: i32, color: u16) {
        match label {
            "Чувствительность" | "Направление" => {
                self.symbol(Symbol::Gestures, cx, cy, color)
            }
            "Задать PIN" | "Изменить PIN" | "Отключить PIN" | "Заблокировать" => {
                self.icon_lock(cx, cy, color)
            }
            "Автоблокировка" | "Применить" | "Сон экрана" => {
                self.icon_clock(cx, cy, color)
            }
            "Контрастность" => self.icon_sun(cx, cy, color),
            "Connectivity" | "Wi-Fi" | "CONNECTIVITY" | "WI-FI" => self.icon_wifi(cx, cy, color),
            "Bluetooth" | "BLUETOOTH" => self.icon_bt(cx, cy, color),
            "Apps & notifications" | "Notifications" | "APPS & NOTIFS" | "NOTIFICATIONS" => {
                self.icon_bell(cx, cy, color)
            }
            "Display" | "Brightness" | "DISPLAY" | "BRIGHTNESS" => self.icon_sun(cx, cy, color),
            "Sound & vibration" | "SOUND & VIBRATION" => self.icon_sound(cx, cy, color),
            "Accessibility" | "ACCESSIBILITY" => self.icon_person(cx, cy, color),
            "Security" | "Screen lock" | "SECURITY" | "SCREEN LOCK" => {
                self.icon_lock(cx, cy, color)
            }
            "Date & time"
            | "Clock"
            | "DATE & TIME"
            | "CLOCK"
            | "Автовремя"
            | "Час"
            | "Минута"
            | "Часовой пояс" => self.icon_clock(cx, cy, color),
            "About" | "System info" | "ABOUT" | "SYSTEM INFO" | "BUILD NUMBER" => {
                self.icon_info(cx, cy, color)
            }
            "Developer options" | "DEVELOPER OPTIONS" => {
                self.symbol(Symbol::Developer, cx, cy, color)
            }
            "System" | "Settings" | "SYSTEM" | "SETTINGS" => self.icon_settings(cx, cy, color),
            "Gestures" | "GESTURES" => self.symbol(Symbol::Gestures, cx, cy, color),
            "NETWORKS" => self.symbol(Symbol::Search, cx, cy, color),
            "PAIR PHONE / PC" => self.symbol(Symbol::Devices, cx, cy, color),
            "TIMER" => self.symbol(Symbol::Timer, cx, cy, color),
            "ALARM" => self.symbol(Symbol::Alarm, cx, cy, color),
            "STOPWATCH" => self.icon_clock(cx, cy, color),
            "Мировое время" => self.symbol(Symbol::World, cx, cy, color),
            "Media controls" | "MEDIA CONTROLS" => self.icon_media(cx, cy, color),
            "Airplane mode" | "AIRPLANE MODE" => self.icon_airplane(cx, cy, color),
            "Back" | "BACK" => self.icon_back(cx, cy, color),
            _ => self.icon_info(cx, cy, color),
        }
    }
}
