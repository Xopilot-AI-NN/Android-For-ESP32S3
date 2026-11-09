use super::*;
impl Surface {
    pub(super) fn focus_page(&mut self, f: MaterialFrame, t: Theme) {
        let v = f.app.focus;
        self.photo_header(f, "Фокус", t);
        self.round_rect(12, 62, 216, 108, 20, t.primary_container);
        self.text_center_raw(
            72,
            if v.phase == 0 {
                "Время для работы"
            } else {
                "Перерыв · 5 минут"
            },
            1,
            t.on_surface_variant,
        );
        self.clock_text(84, &Self::mmss_text(v.remaining), t.primary);
        self.text_center_raw(
            147,
            &alloc::format!("Завершено сессий: {}", v.sessions),
            1,
            t.on_surface_variant,
        );
        let action = if v.running {
            "Пауза"
        } else if v.complete {
            "Следующий этап"
        } else {
            "Начать / продолжить"
        };
        self.action_chip(180, 36, action, v.cursor == 0, t);
        self.action_chip(
            222,
            24,
            &alloc::format!("Работа · {} мин", v.work),
            v.cursor == 1,
            t,
        );
        self.action_chip(252, 24, "Сбросить", v.cursor == 2, t);
    }
}
