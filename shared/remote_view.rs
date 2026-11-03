use super::*;
impl Surface {
    /// Один согласованный интерфейс сервиса; письма/события появятся только после реального входа.
    pub(super) fn remote_app_page(&mut self, mut f: MaterialFrame, t: Theme) {
        let v = f.app.remote;
        let title = match v.kind {
            0 => "Google",
            1 => "Google Calendar",
            2 => "Gmail",
            _ => "Переводчик",
        };
        let text = core::str::from_utf8(&v.text[..v.len as usize]).unwrap_or("");
        if v.mode == 1 {
            f.keyboard = v.keyboard;
            self.text_keyboard(f, t, title, text, "Готово");
            return;
        }
        self.photo_header(
            f,
            if v.mode == 3 {
                "Аккаунт Google"
            } else {
                title
            },
            t,
        );
        self.symbol(
            match v.kind {
                0 => Symbol::Search,
                1 => Symbol::Clock,
                2 => Symbol::Share,
                _ => Symbol::Language,
            },
            120,
            85,
            t.primary,
        );
        if v.mode == 2 {
            self.text_fitted_raw(12, 117, text, 1, 216, t.on_surface_variant);
            let result = core::str::from_utf8(&v.result[..v.result_len as usize]).unwrap_or("");
            for (i, line) in result.split('\n').take(3).enumerate() {
                self.text_fitted_raw(12, 151 + i as i32 * 23, line, 1, 216, t.on_surface);
            }
            self.text_fitted_raw(12, 240, v.message, 1, 216, t.on_surface_variant);
            return;
        }
        let labels = if v.mode == 3 {
            ["Войти по QR", "Синхронизация", "Выйти"]
        } else {
            match v.kind {
                0 => ["Введите запрос", "Последний запрос", "Аккаунт Google"],
                1 => ["Ближайшие события", "Обновить", "Аккаунт Google"],
                2 => ["Входящие", "Обновить", "Аккаунт Google"],
                _ => ["Введите текст", "RU ↔ EN", "Аккаунт Google"],
            }
        };
        for (i, label) in labels.iter().enumerate() {
            self.round_rect(
                10,
                118 + i as i32 * 39,
                220,
                34,
                13,
                Self::focus_fill(t.surface_high, v.cursor as usize == i),
            );
            self.text_fitted_raw(
                23,
                128 + i as i32 * 39,
                label,
                1,
                194,
                Self::focus_ink(t, v.cursor as usize == i),
            );
        }
        self.text_fitted_raw(10, 257, v.message, 1, 220, t.on_surface_variant);
    }
}
