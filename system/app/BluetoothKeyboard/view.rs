use super::*;
impl Surface {
    pub(super) fn keyboard_app_page(&mut self, mut f: MaterialFrame, t: Theme) {
        let v = f.app.keyboard_app;
        if v.typing {
            f.keyboard = v.keyboard;
            let text = core::str::from_utf8(&v.text[..v.len as usize]).unwrap_or("");
            self.text_keyboard(f, t, "BT-клавиатура", text, "Enter");
            return;
        }
        self.photo_header(f, "BT-клавиатура", t);
        self.symbol(Symbol::Bluetooth, 120, 93, t.primary);
        self.text_center_raw(
            120,
            if matches!(f.bt_status, 2 | 3) {
                "Устройство подключено"
            } else {
                "Нет подключения"
            },
            1,
            t.on_surface_variant,
        );
        for (index, label) in ["Сопряжение", "Открыть клавиатуру"].iter().enumerate()
        {
            let selected = v.cursor as usize == index;
            let y = 158 + index as i32 * 45;
            self.round_rect(
                10,
                y,
                220,
                38,
                14,
                if selected { t.primary } else { t.surface_high },
            );
            self.text_fitted_raw(22, y + 12, label, 1, 196, Self::focus_ink(t, selected));
        }
        self.text_fitted_raw(10, 257, v.message, 1, 220, t.on_surface_variant);
    }
}
