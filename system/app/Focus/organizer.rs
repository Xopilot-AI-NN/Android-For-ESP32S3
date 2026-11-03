// Модуль экранов задач, расходов, фокуса и телефона.
use super::*;
impl Surface {
    /// Рисует экран рабочих сессий и перерывов с актуальным состоянием и фокусом.
    pub(super) fn focus_page(&mut self, f: MaterialFrame, t: Theme) {
        let v = f.app.focus;
        self.photo_header(f, "Фокус", t);
        self.text_center(
            59,
            if v.phase == 0 {
                "Время для работы"
            } else {
                "Перерыв · 5 минут"
            },
            1,
            t.on_surface_variant,
        );
        let time = Self::mmss_text(v.remaining);
        self.clock_text(83, &time, t.primary);
        self.text_center(
            153,
            &alloc::format!("Сессий: {}", v.sessions),
            1,
            t.on_surface_variant,
        );
        let labels = [
            if v.running {
                alloc::string::String::from("Пауза")
            } else if v.complete {
                alloc::string::String::from("Начать следующий этап")
            } else {
                alloc::string::String::from("Начать / продолжить")
            },
            alloc::format!("Работа: {} минут", v.work),
            alloc::string::String::from("Сбросить сессию"),
        ];
        for (i, label) in labels.iter().enumerate() {
            self.keyboard_key(
                18,
                179 + i as i32 * 31,
                204,
                28,
                label,
                v.cursor as usize == i,
                t,
            );
        }
    }
}
