// Модуль погоды, мирового времени и дыхания.
//! Current conditions from the on-watch HTTP client; errors never invent readings.
use super::*;
impl Surface {
    /// Рисует экран дыхательной сессии с актуальным состоянием и фокусом.
    pub(super) fn breathing_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Дыхание", t);
        let phase = f.app.breathe_elapsed % 10;
        let radius = if !f.app.breathing {
            37
        } else if phase < 4 {
            34 + i32::from(phase) * 5
        } else {
            54 - i32::from(phase - 4) * 4
        };
        self.circle(120, 127, radius, t.primary_container);
        self.symbol(Symbol::Breathing, 120, 127, t.primary);
        let title = if f.app.breathing {
            if phase < 4 { "Вдох" } else { "Выдох" }
        } else if f.app.breathe_elapsed == 120 {
            "Готово"
        } else {
            "Две минуты покоя"
        };
        self.text_center(194, title, 2, t.on_surface);
        self.round_rect(40, 237, 160, 36, 18, t.primary);
        self.text_center(
            247,
            if f.app.breathing {
                "Остановить"
            } else {
                "Начать"
            },
            1,
            t.on_primary,
        );
    }
}
