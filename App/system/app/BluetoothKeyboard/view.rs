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
        self.round_rect(12, 64, 216, 84, 20, t.surface);
        self.circle(46, 105, 22, t.primary_container);
        self.symbol(Symbol::Bluetooth, 46, 105, t.primary);
        self.text_raw(80, 83, "Bluetooth HID", 1, t.on_surface);
        self.text_fitted_raw(80, 108, if matches!(f.bt_status, 2 | 3) { "Подключено" } else { "Нет подключения" }, 1, 136, t.on_surface_variant);
        self.action_chip(166, 40, "Сопряжение", v.cursor == 0, t);
        self.action_chip(216, 40, "Открыть клавиатуру", v.cursor != 0, t);
        self.text_fitted_raw(10, 257, v.message, 1, 220, t.on_surface_variant);
    }
}
