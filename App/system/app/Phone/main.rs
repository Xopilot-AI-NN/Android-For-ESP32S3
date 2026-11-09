use super::*;
impl Surface {
    pub(super) fn phone_page(&mut self, f: MaterialFrame, t: Theme) {
        let v = f.app.phone;
        self.photo_header(f, "Телефон", t);
        let name = if v.name_len == 0 {
            "KDE Connect"
        } else {
            core::str::from_utf8(&v.name[..v.name_len as usize]).unwrap_or("KDE Connect")
        };
        if v.pending {
            self.text_fitted_raw(16, 65, name, 1, 208, t.on_surface_variant);
            self.text_center_raw(96, "Разрешить подключение?", 1, t.on_surface);
            self.round_rect(12, 122, 216, 55, 18, t.primary_container);
            self.text_center_raw(
                130,
                core::str::from_utf8(&v.code).unwrap_or("----"),
                3,
                t.primary,
            );
            self.action_chip(189, 36, "Разрешить", v.cursor == 0, t);
            self.action_chip(231, 36, "Отклонить", v.cursor != 0, t);
        } else {
            self.round_rect(12, 62, 216, 54, 16, t.surface);
            self.symbol(Symbol::Bluetooth, 34, 89, t.primary);
            self.text_fitted_raw(55, 70, name, 1, 160, t.on_surface);
            self.text_fitted_raw(
                55,
                91,
                if v.online {
                    "Подключён через ПК"
                } else if v.paired {
                    "Не в сети"
                } else {
                    "Мост не запущен"
                },
                1,
                160,
                t.on_surface_variant,
            );
            for (i, label) in [
                "Найти телефон",
                "Отправить сигнал",
                "Отправить текст",
                "Отключить мост",
            ]
            .iter()
            .enumerate()
            {
                self.action_chip(124 + i as i32 * 34, 30, label, v.cursor as usize == i, t);
            }
            self.text_fitted_raw(12, 264, v.message, 1, 216, t.on_surface_variant);
        }
    }
}
