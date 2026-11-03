// Модуль экранов календаря, конвертера и счётчика.
//! Offline applications share the system header, focus colors and crown navigation.
use super::*;

impl Surface {

    /// Рисует экран конвертера единиц с актуальным состоянием и фокусом.
    pub(super) fn converter_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Конвертер", t);
        let tools = f.app.tools;
        let (from, to) = tools.units();
        self.round_rect(14, 60, 212, 66, 22, t.surface);
        self.text_center_raw(73, &tools.result(), 2, t.primary);
        self.text_center_raw(104, to, 1, t.on_surface_variant);
        let labels = [
            alloc::format!("{} / {}", from, to),
            alloc::format!("Число: {}", tools.amount),
            alloc::string::String::from("Поменять местами"),
        ];
        for (i, label) in labels.iter().enumerate() {
            let selected = usize::from(tools.converter_action) == i;
            self.keyboard_key(18, 138 + i as i32 * 44, 204, 35, label, selected, t);
        }
        self.text_center(
            265,
            if tools.editing_amount {
                "Колесо: число · Нажать: OK"
            } else {
                "Нажмите для изменения"
            },
            1,
            t.on_surface_variant,
        );
    }
}
