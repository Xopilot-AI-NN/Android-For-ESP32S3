// Модуль экранов задач, расходов, фокуса и телефона.
use super::*;
impl Surface {
    /// Рисует экран подтверждённого моста KDE Connect с актуальным состоянием и фокусом.
    pub(super) fn phone_page(&mut self, f: MaterialFrame, t: Theme) {
        let v = f.app.phone;
        self.photo_header(f, "Телефон · KDE Connect", t);
        let name = core::str::from_utf8(&v.name[..v.name_len as usize]).unwrap_or("KDE Connect");
        self.text_center_raw(61, name, 1, t.primary);
        if v.pending {
            self.text_center(92, "Разрешить мост с кодом?", 1, t.on_surface);
            self.text_center_raw(
                118,
                core::str::from_utf8(&v.code).unwrap_or("----"),
                3,
                t.primary,
            );
            for (i, label) in ["Разрешить", "Отклонить"].iter().enumerate() {
                self.keyboard_key(
                    18,
                    177 + i as i32 * 43,
                    204,
                    35,
                    label,
                    v.cursor as usize == i,
                    t,
                );
            }
        } else {
            self.text_center(
                84,
                if v.online {
                    "Подключено через компьютер"
                } else if v.paired {
                    "Мост не в сети"
                } else {
                    "Запустите мост на ПК"
                },
                1,
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
                self.keyboard_key(
                    18,
                    110 + i as i32 * 37,
                    204,
                    31,
                    label,
                    v.cursor as usize == i,
                    t,
                );
            }
        }
        self.text_center(264, v.message, 1, t.on_surface_variant);
    }
}
