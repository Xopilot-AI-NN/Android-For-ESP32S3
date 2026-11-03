// Экран Calculator; состояние поступает из контроллера приложения.
use super::*;
#[allow(unused_imports)]
use core::fmt::Write;
impl Surface {
    /// Отрисовывает текущее состояние приложения без операций ввода-вывода.
    pub(super) fn calculator_page(&mut self, f: MaterialFrame, t: Theme) {
        use crate::apps::*;
        let a = f.app;
        self.photo_header(f, LABELS[usize::from(a.id)], t);

                let value =
                    core::str::from_utf8(&a.text[..usize::from(a.len).min(32)]).unwrap_or("Ошибка");
                let expression = core::str::from_utf8(&a.calculator.expression[..usize::from(a.calculator.len)]).unwrap_or("");
                self.text_fitted_raw(12, 53, expression, 1, 216, t.on_surface_variant);
                self.round_rect(12, 74, 216, 31, 12, t.surface);
                self.text_center_raw(81, if a.calculator.message.is_empty() { value } else { a.calculator.message },
                    if value.len() > 12 || !a.calculator.message.is_empty() { 1 } else { 2 }, t.primary);
                for (index, label) in calculator::KEYS.iter().enumerate() {
                    let x = 12 + (index % 4) as i32 * 55;
                    let y = 112 + (index / 4) as i32 * 27;
                    let selected = index == usize::from(a.cursor);
                    if *label == "DEL" {
                        self.round_rect(x, y, 51, 24, 7, Self::focus_fill(t.surface_high, selected));
                        self.symbol(Symbol::Backspace, x + 25, y + 12, Self::focus_ink(t, selected));
                    } else { self.keyboard_key(x, y, 51, 24, label, selected, t); }
                }

    }
}
