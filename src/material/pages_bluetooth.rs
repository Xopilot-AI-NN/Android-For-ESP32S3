//! pages bluetooth Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn bluetooth_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Bluetooth", t);
        self.round_rect(
            13,
            64,
            214,
            61,
            30,
            if f.cursor == 0 {
                t.primary_container
            } else {
                t.surface_high
            },
        );
        self.icon_bt(
            44,
            94,
            if f.bt {
                t.primary
            } else {
                t.on_surface_variant
            },
        );
        self.text(74, 86, "Bluetooth", 1, t.on_surface);
        self.toggle(169, 81, f.bt, t);
        self.info_card(
            136,
            "PAIR PHONE / PC",
            if f.bt {
                "OPEN PAIRING"
            } else {
                "TURN ON BLUETOOTH"
            },
            f.cursor == 1,
            t,
        );
        // Match the three rows in the original watch photographs.
        self.info_card(205, "MEDIA CONTROLS", "PHONE / PC", f.cursor == 2, t);
        self.text_center(270, "HOLD CROWN: BACK", 1, t.on_surface_variant);
    }

    pub(super) fn display_settings(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Display", t);
        let brightness = alloc::format!("{} %", f.brightness);
        let timeout = if f.lock_timeout_secs == 30 {
            alloc::string::String::from("30 секунд")
        } else {
            alloc::format!("{} мин", f.lock_timeout_secs / 60)
        };
        let theme_name = match f.theme {
            1 => "Blue",
            2 => "Purple",
            3 => "Coral",
            _ => "Teal",
        };
        let face_name = ["Цифровой", "Аналоговый", "Компликации"][f.watch_face.min(2) as usize];
        let labels = [
            "Brightness",
            "COLOR THEME",
            "Сон экрана",
            "Циферблат",
            "BACK",
        ];
        let values = [
            brightness.as_str(),
            theme_name,
            timeout.as_str(),
            face_name,
            "К настройкам",
        ];
        let cur = f.cursor.min(4);
        let start = cur.saturating_sub(1).min(2);
        for row in 0..3 {
            let index = (start + row) as usize;
            self.info_card(
                62 + i32::from(row) * 65,
                labels[index],
                values[index],
                index == usize::from(cur),
                t,
            );
        }
        self.scroll_indicator(cur, 5, t);
    }

    pub(super) fn brightness_editor(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Brightness", t);
        self.circle(120, 105, 39, t.primary_container);
        self.icon_sun(120, 105, t.primary);
        self.text_center(155, &alloc::format!("{} %", f.brightness), 2, t.on_surface);
        self.slider(24, 187, 192, f.brightness, t);
        self.text_center(224, "ROTATE CROWN", 1, t.on_surface);
        self.text_center(245, "PRESS TO CLOSE", 1, t.on_surface_variant);
    }

    pub(super) fn bluetooth_pairing(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Bluetooth", t);
        self.circle(120, 81, 25, t.primary_container);
        self.icon_bt(120, 81, t.primary);
        self.text_center(116, "AOSP Wear", 2, t.on_surface);
        if let Some(pin) = f.bt_pin {
            let digits = [
                b'0' + ((pin / 100000) % 10) as u8,
                b'0' + ((pin / 10000) % 10) as u8,
                b'0' + ((pin / 1000) % 10) as u8,
                b'0' + ((pin / 100) % 10) as u8,
                b'0' + ((pin / 10) % 10) as u8,
                b'0' + (pin % 10) as u8,
            ];
            self.text_center(
                145,
                core::str::from_utf8(&digits).unwrap_or("------"),
                2,
                t.primary,
            );
        } else {
            let status = match f.bt_status {
                1 => "Новое устройство: начать",
                2 => "Connected / securing",
                3 => "Media remote ready",
                5 => "Ошибка BLE. Перезапустите",
                6 => "Выберите AOSP Wear на телефоне",
                7 => "Запуск Bluetooth...",
                _ => "Press Pair new device",
            };
            self.text_center(145, status, 1, t.on_surface_variant);
        }
        let labels = if f.bt_pin.is_some() {
            ["Confirm matching code", "Cancel", "Cancel"]
        } else {
            ["Pair new device", "Disconnect", "Forget devices"]
        };
        for (i, label) in labels.iter().enumerate() {
            self.round_rect(
                25,
                170 + i as i32 * 32,
                190,
                29,
                14,
                if f.cursor as usize == i {
                    t.primary_container
                } else {
                    t.surface_high
                },
            );
            self.text_center(178 + i as i32 * 32, label, 1, t.on_surface);
        }
        self.text_center(266, "HOLD CROWN: BACK", 1, t.on_surface_variant);
    }
}
