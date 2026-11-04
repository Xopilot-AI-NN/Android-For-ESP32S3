use super::*;
impl Surface {
    pub(super) fn calculator_page(&mut self, f: MaterialFrame, t: Theme) {
        let a = f.app;
        self.photo_header(f, "Калькулятор", t);
        let value = core::str::from_utf8(&a.text[..usize::from(a.len).min(32)]).unwrap_or("Ошибка");
        let expression =
            core::str::from_utf8(&a.calculator.expression[..usize::from(a.calculator.len)])
                .unwrap_or("");
        self.round_rect(12, 60, 216, 46, 16, t.surface);
        self.text_fitted_raw(22, 64, expression, 1, 196, t.on_surface_variant);
        let result = if a.calculator.message.is_empty() {
            value
        } else {
            a.calculator.message
        };
        let scale = if Self::text_width(result, 2) <= 196 {
            2
        } else {
            1
        };
        let width = Self::text_width(result, scale).min(196);
        self.text_fitted_raw(218 - width, 84, result, scale, 196, t.primary);
        for (index, label) in crate::apps::calculator::KEYS.iter().enumerate() {
            let (x, y) = (12 + (index % 4) as i32 * 55, 113 + (index / 4) as i32 * 27);
            let selected = index == usize::from(a.cursor);
            let operation = matches!(*label, "/" | "*" | "-" | "+" | "%" | "sqrt" | "x2");
            let bg = if *label == "=" {
                t.primary
            } else if operation {
                t.primary_container
            } else {
                t.surface_high
            };
            let ink = if *label == "=" {
                t.on_primary
            } else if *label == "C" {
                t.error
            } else {
                t.on_surface
            };
            self.round_rect(x, y, 51, 24, 11, if selected { t.primary } else { bg });
            if selected {
                self.round_rect(x + 2, y + 2, 47, 20, 9, bg);
            }
            if *label == "DEL" {
                self.symbol(Symbol::Backspace, x + 25, y + 12, ink);
            } else {
                let width = Self::text_width(label, 1).min(43);
                self.text_fitted_raw(x + (51 - width) / 2, y + 5, label, 1, 43, ink);
            }
        }
    }
}
