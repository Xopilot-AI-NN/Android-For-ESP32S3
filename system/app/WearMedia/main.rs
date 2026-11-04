use super::*;
impl Surface {
    pub(super) fn media(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Медиа", t);
        self.round_rect(12, 63, 216, 48, 16, t.surface);
        self.symbol(Symbol::Bluetooth, 34, 87, t.primary);
        self.text_fitted_raw(
            56,
            71,
            if f.bt_status == 3 {
                "Подключено"
            } else {
                "Нет подключения"
            },
            1,
            160,
            t.on_surface,
        );
        self.text_fitted_raw(
            56,
            92,
            if f.media_known {
                "Плеер с телефона"
            } else {
                "Медиакнопки"
            },
            1,
            160,
            t.on_surface_variant,
        );
        for (index, x, radius) in [(0, 43, 25), (1, 120, 35), (2, 197, 25)] {
            let selected = f.cursor == index;
            self.circle(
                x,
                162,
                radius,
                if selected {
                    t.primary
                } else if index == 1 {
                    t.primary_container
                } else {
                    t.surface_high
                },
            );
            self.symbol(
                match index {
                    0 => Symbol::Previous,
                    2 => Symbol::Next,
                    _ if f.media_playing => Symbol::Pause,
                    _ => Symbol::Play,
                },
                x,
                162,
                if selected {
                    t.on_primary
                } else if index == 1 {
                    t.primary
                } else {
                    t.on_surface
                },
            );
        }
        if f.media_known {
            self.round_rect(12, 208, 216, 4, 2, t.surface_high);
            self.round_rect(
                12,
                208,
                (216 * i32::from(f.media_progress) / 1000).max(2),
                4,
                2,
                t.primary,
            );
        }
        self.text_center_raw(
            220,
            if f.media_known {
                if f.media_playing {
                    "Воспроизведение"
                } else {
                    "Пауза"
                }
            } else {
                "Пуск / пауза"
            },
            1,
            t.on_surface_variant,
        );
        for (index, x, label) in [(3, 12, "Тише"), (4, 86, "Громче"), (5, 160, "Назад")]
        {
            self.keyboard_key(x, 245, 68, 31, label, f.cursor == index, t);
        }
    }
}
