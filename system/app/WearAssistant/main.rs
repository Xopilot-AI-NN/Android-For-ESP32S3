// Модуль основных экранов часов.
//! pages main Material surface implementation.
use super::*;

impl Surface {

    /// Рисует экран ассистента с актуальным состоянием и фокусом.
    pub(super) fn assistant_page(&mut self, f: MaterialFrame, t: Theme) {
        // Assistant shell is ready for the future phone companion.  The UI is
        // intentionally service-neutral: it mirrors the Wear assistant surface
        // without claiming a Google/Gemini backend that does not exist here.
        self.ring(120, 116, 70, 2, t.surface_high);
        self.icon_assistant(120, 91, t.primary);
        self.text_center(126, "ASK ASSISTANT", 2, t.on_surface);
        self.text_center(
            158,
            if f.bt_status == 3 {
                "Ассистент устройства"
            } else {
                "Подключите устройство"
            },
            1,
            t.on_surface_variant,
        );
        self.line(79, 183, 101, 183, 3, rgb565(80, 190, 255));
        self.line(101, 183, 123, 183, 3, rgb565(126, 225, 165));
        self.line(123, 183, 145, 183, 3, rgb565(224, 187, 255));
        self.line(145, 183, 167, 183, 3, rgb565(255, 184, 161));
        self.round_rect(51, 224, 138, 38, 19, t.surface_high);
        self.text_center(236, "Нажмите для запуска", 1, t.on_surface_variant);
    }
}
