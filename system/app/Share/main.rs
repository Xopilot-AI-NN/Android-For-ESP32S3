use super::*;
impl Surface {
    pub(super) fn share_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Передача", t);
        let Some([a, b, c, d]) = f.wifi_ip else {
            self.empty_state(Symbol::Wifi, "Нужен Wi-Fi", "Подключите часы к сети", t);
            return;
        };
        self.round_rect(12, 62, 216, 62, 18, t.surface);
        self.symbol(Symbol::Share, 34, 85, t.primary);
        self.text_fitted_raw(
            55,
            70,
            if f.adb {
                "ADB включён"
            } else {
                "Включите ADB"
            },
            1,
            160,
            t.on_surface,
        );
        self.text_fitted_raw(
            24,
            99,
            &alloc::format!("{a}.{b}.{c}.{d}:5555"),
            1,
            192,
            t.primary,
        );
        let page = f.app.cursor.min(2);
        let (title, first, second) = match page {
            0 => (
                "Подключить компьютер",
                "adb connect",
                alloc::format!("{a}.{b}.{c}.{d}:5555"),
            ),
            1 => (
                "Загрузить обои",
                "adb push image.png",
                alloc::string::String::from("/sdcard/Wallpapers/"),
            ),
            _ => (
                "Скачать обои",
                "adb pull",
                alloc::string::String::from("/sdcard/Wallpapers/1.png"),
            ),
        };
        self.text_fitted_raw(12, 140, title, 1, 216, t.on_surface_variant);
        self.round_rect(12, 165, 216, 74, 18, t.primary_container);
        self.text_raw(24, 178, first, 1, t.on_surface);
        self.text_wrapped(24, 202, &second, 192, t.on_surface);
        if page == 1 {
            self.text_center_raw(241, "PNG · 240×280", 1, t.on_surface_variant);
        }
        self.text_raw(12, 263, "Колесо: инструкция", 1, t.on_surface_variant);
        self.text_fitted_raw(
            190,
            263,
            &alloc::format!("{}/3", page + 1),
            1,
            38,
            t.primary,
        );
    }
}
