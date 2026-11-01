//! icons Material surface implementation.
use super::*;

impl Surface {
    /// Tint Google's checked-in SVG coverage directly into the RGB565 surface.
    pub(super) fn symbol(&mut self, symbol: Symbol, cx: i32, cy: i32, color: u16) {
        use super::symbols::{ATLAS, SIZE};
        let base = symbol as usize * SIZE * SIZE;
        for row in 0..SIZE {
            for col in 0..SIZE {
                let offset = base + row * SIZE + col;
                let packed = ATLAS[offset / 2];
                let alpha = u32::from(if offset % 2 == 0 {
                    packed >> 4
                } else {
                    packed & 15
                });
                let x = cx - SIZE as i32 / 2 + col as i32;
                let y = cy - SIZE as i32 / 2 + row as i32;
                if alpha == 0 || x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
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
    pub(super) fn icon_wifi(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Wifi, cx, cy, c);
    }

    pub(super) fn icon_bt(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Bluetooth, cx, cy, c);
    }

    pub(super) fn icon_dnd(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Dnd, cx, cy, c);
    }

    pub(super) fn icon_airplane(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Airplane, cx, cy, c);
    }

    pub(super) fn icon_sun(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Sun, cx, cy, c);
    }

    pub(super) fn icon_settings(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Settings, cx, cy, c);
    }

    pub(super) fn icon_bell(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Bell, cx, cy, c);
    }

    pub(super) fn icon_clock(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Clock, cx, cy, c);
    }

    pub(super) fn icon_media(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Play, cx, cy, c);
    }

    pub(super) fn icon_info(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Info, cx, cy, c);
    }

    pub(super) fn icon_grid(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Grid, cx, cy, c);
    }

    pub(super) fn icon_assistant(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Assistant, cx, cy, c);
    }

    pub(super) fn icon_connectivity(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Connectivity, cx, cy, c);
    }

    pub(super) fn icon_back(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Back, cx, cy, c);
    }

    pub(super) fn icon_lock(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Lock, cx, cy, c);
    }

    pub(super) fn icon_sound(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Sound, cx, cy, c);
    }

    pub(super) fn icon_person(&mut self, cx: i32, cy: i32, c: u16) {
        self.symbol(Symbol::Person, cx, cy, c);
    }
    pub(super) fn app_icon(&mut self, icon: u8, cx: i32, cy: i32, c: u16) {
        match icon {
            0 => self.icon_clock(cx, cy, c),
            1 => self.icon_media(cx, cy, c),
            2 => self.icon_settings(cx, cy, c),
            3 => self.icon_connectivity(cx, cy, c),
            4 => self.icon_info(cx, cy, c),
            5 => self.icon_assistant(cx, cy, c),
            _ => self.icon_grid(cx, cy, c),
        }
    }

    pub(super) fn app_accent(icon: u8) -> u16 {
        match icon {
            0 => rgb565(181, 232, 211), // clock mint
            1 => rgb565(255, 191, 171), // media coral
            2 => rgb565(174, 203, 255), // settings blue
            3 => rgb565(102, 221, 205), // connectivity teal
            4 => rgb565(218, 190, 255), // system lavender
            5 => rgb565(126, 225, 165), // assistant green
            _ => rgb565(50, 59, 54),
        }
    }

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
            "Date & time" | "Clock" | "DATE & TIME" | "CLOCK" => self.icon_clock(cx, cy, color),
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
            "Media controls" | "MEDIA CONTROLS" => self.icon_media(cx, cy, color),
            "Airplane mode" | "AIRPLANE MODE" => self.icon_airplane(cx, cy, color),
            "Back" | "BACK" => self.icon_back(cx, cy, color),
            _ => self.icon_info(cx, cy, color),
        }
    }
}
