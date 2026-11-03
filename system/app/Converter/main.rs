// Логика приложения Converter: события энкодера и вычисления.
use crate::apps::utilities::*;
use crate::runtime::RuntimeEvent;
use core::fmt::Write;
impl Tools {
    /// Переключает единицы, направление и режим изменения числа.
    pub fn converter_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Rotate(d) if self.editing_amount => {
                self.amount = self.amount.saturating_add(d).clamp(-100_000, 100_000);
            }
            RuntimeEvent::Rotate(d) => {
                self.converter_action =
                    (i32::from(self.converter_action) + d.rem_euclid(3)).rem_euclid(3) as u8;
            }
            RuntimeEvent::Press => match self.converter_action {
                0 => self.conversion = (self.conversion + 1) % CONVERSIONS.len() as u8,
                1 => self.editing_amount = !self.editing_amount,
                _ => self.reverse = !self.reverse,
            },
            _ => {}
        }
    }
    /// Возвращает исходную и конечную единицы с учётом направления.
    pub fn units(&self) -> (&'static str, &'static str) {
        let (from, to) = CONVERSIONS[usize::from(self.conversion)];
        if self.reverse { (to, from) } else { (from, to) }
    }
    /// Integer rational conversions, rounded to a hundredth; no floating runtime.
    pub fn result(&self) -> heapless::String<32> {
        let n = i64::from(self.amount);
        let (num, den) = match (self.conversion, self.reverse) {
            (0, false) => (n * 1_000_000, 1_609_344),
            (0, true) => (n * 1_609_344, 1_000_000),
            (1, false) => (n * 9 + 160, 5),
            (1, true) => ((n - 32) * 5, 9),
            (2, false) => (n * 100_000_000, 45_359_237),
            (2, true) => (n * 45_359_237, 100_000_000),
            (3, false) => (n * 100, 254),
            (3, true) => (n * 254, 100),
            (4, false) => (n * 1_000_000_000, 3_785_411_784),
            (4, true) => (n * 3_785_411_784, 1_000_000_000),
            (_, false) => (n * 10, 36),
            (_, true) => (n * 36, 10),
        };
        let scaled = (num.abs() * 100 + den / 2) / den;
        let mut result = heapless::String::new();
        let _ = write!(
            result,
            "{}{}.{:02}",
            if num < 0 && scaled != 0 { "-" } else { "" },
            scaled / 100,
            scaled % 100
        );
        result
    }

}
