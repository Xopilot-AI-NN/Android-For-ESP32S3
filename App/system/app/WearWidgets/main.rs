// Модуль основных экранов часов.
//! pages main Material surface implementation.
use super::*;

impl Surface {

    /// Рисует выбранную системную плитку с актуальными значениями.
    pub(super) fn widgets(&mut self, f: MaterialFrame, t: Theme) {
        let cur = f.cursor.min(2);
        // Edge-to-edge Wear tile card.  Each tile gets its own accent family
        // instead of looking like another Settings page.
        let (bg, fg, sub) = if cur == 0 {
            (
                t.primary_container,
                t.on_primary_container,
                if f.wifi {
                    "WI-FI CONNECTED"
                } else {
                    "WI-FI OFF"
                },
            )
        } else if cur == 1 {
            (
                t.primary_container,
                t.on_primary_container,
                if f.timer_secs > 0 {
                    "TIMER RUNNING"
                } else {
                    "CHOOSE A TIMER"
                },
            )
        } else {
            (
                t.primary_container,
                t.on_primary_container,
                if f.alarm_enabled {
                    "ALARM ON"
                } else {
                    "ALARM OFF"
                },
            )
        };
        self.round_rect(10, 24, 220, 208, 42, bg);
        if cur == 0 {
            self.icon_wifi(52, 66, fg);
            self.text(82, 55, "CONNECTIVITY", 1, fg);
            self.text(27, 112, sub, 2, fg);
            self.text(27, 153, "PRESS TO OPEN", 1, fg);
        } else if cur == 1 {
            self.icon_clock(52, 66, fg);
            self.text(82, 55, "CLOCK", 1, fg);
            self.text(27, 112, sub, 2, fg);
            let timer = Self::mmss_text(if f.timer_secs == 0 {
                f.timer_duration_secs
            } else {
                f.timer_secs
            });
            let timer_s = core::str::from_utf8(&timer).unwrap_or("05:00");
            self.text(27, 153, timer_s, 2, fg);
        } else {
            self.icon_clock(52, 66, fg);
            self.text(82, 55, "ALARM", 1, fg);
            self.text(27, 112, sub, 2, fg);
            self.text(27, 153, "PRESS TO OPEN", 1, fg);
        }
        // Carousel dots deliberately sit outside the card like Wear tiles.
        for i in 0..3 {
            self.circle(
                108 + i * 12,
                254,
                if i as u8 == cur { 4 } else { 2 },
                if i as u8 == cur { t.primary } else { t.outline },
            );
        }
    }
}
