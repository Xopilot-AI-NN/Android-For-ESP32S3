// Модуль основных экранов часов.
//! pages main Material surface implementation.
use super::*;

impl Surface {

    /// Рисует циферблат с реальными таймером, секундомером и системными индикаторами.
    pub(super) fn complication_face(&mut self, f: MaterialFrame, t: Theme) {
        let accent = t.primary;
        let muted = t.surface_high;
        for (x, y, progress) in [
            (56, 65, if f.alarm_enabled { 100 } else { 0 }),
            (
                184,
                65,
                if f.wifi_link_state == 4 {
                    100
                } else if f.wifi {
                    25
                } else {
                    0
                },
            ),
            (
                56,
                217,
                (u32::from(f.timer_secs) * 100 / u32::from(f.timer_duration_secs.max(1))) as u8,
            ),
            (184, 217, ((f.stopwatch_secs % 60) * 100 / 60) as u8),
        ] {
            self.circle(x, y, 30, t.surface);
            self.progress_ring(x, y, 33, progress, accent, muted);
        }
        self.symbol(Symbol::Alarm, 56, 54, accent);
        let alarm = Self::time_text(f.alarm_minutes);
        self.text_center_x(
            56,
            77,
            if f.alarm_enabled {
                core::str::from_utf8(&alarm).unwrap_or("07:00")
            } else {
                ""
            },
            1,
            t.on_surface,
        );
        self.symbol(Symbol::Wifi, 184, 54, accent);
        self.text_center_x(
            184,
            77,
            if f.wifi_link_state == 4 {
                ""
            } else if f.wifi {
                ""
            } else {
                "Off"
            },
            1,
            t.on_surface,
        );
        self.big_time(117, f.time_minutes, f.time_valid, accent);
        self.text_center(
            174,
            if f.time_valid { "" } else { "Set time to sync" },
            1,
            t.on_surface_variant,
        );
        self.symbol(Symbol::Timer, 56, 205, accent);
        self.symbol(Symbol::Clock, 184, 205, accent);
        let timer = Self::mmss_text(f.timer_secs);
        let stopwatch = Self::mmss_text(f.stopwatch_secs);
        self.text_center_x(
            56,
            229,
            core::str::from_utf8(&timer).unwrap_or("00:00"),
            1,
            t.on_surface,
        );
        self.text_center_x(
            184,
            229,
            core::str::from_utf8(&stopwatch).unwrap_or("00:00"),
            1,
            t.on_surface,
        );

        for i in 0..3 {
            self.circle(
                110 + i * 10,
                269,
                if i == 1 { 3 } else { 2 },
                if i == 1 { accent } else { muted },
            );
        }
        if f.notes > 0 {
            self.circle(120, 199, 3, t.primary);
        }
    }

    /// Рисует общий раздел таймера, секундомера, будильника и мирового времени.
    pub(super) fn clock(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Clock", t);
        let labels = ["TIMER", "STOPWATCH", "ALARM", "Мировое время", "Back"];
        let cursor = usize::from(f.cursor.min(4));
        let start = cursor.saturating_sub(1).min(2);
        for row in 0..3 {
            let index = start + row;
            self.wear_row(63 + row as i32 * 67, labels[index], index == cursor, t);
        }
        self.scroll_indicator(cursor as u8, 5, t);
    }

    /// Рисует экран секундомера с актуальным состоянием и фокусом.
    pub(super) fn stopwatch_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Stopwatch", t);
        let time = Self::mmss_text(f.stopwatch_secs);
        self.clock_text(99, &time, t.primary);
        let labels = [
            if f.stopwatch_running {
                "Пауза"
            } else {
                "Пуск"
            },
            "Сброс",
            "BACK",
        ];
        for (i, label) in labels.iter().enumerate() {
            self.round_rect(
                32,
                178 + i as i32 * 31,
                176,
                28,
                14,
                Self::focus_fill(t.surface_high, f.cursor as usize == i),
            );
            self.text_center(187 + i as i32 * 31, label, 1, Self::focus_ink(t, f.cursor as usize == i));
        }
    }

    /// Рисует предустановки или редактирование длительности таймера.
    pub(super) fn timer_presets(&mut self, f: MaterialFrame, t: Theme) {
        let lavender = t.primary_container;
        let lime = t.primary;
        let ink = t.on_primary_container;
        self.photo_header(f, "Timer", t);
        if f.timer_editor == 2 {
            let duration = Self::mmss_text(f.timer_duration_secs);
            self.clock_text(104, &duration, t.primary);
            self.text_center(188, "Crown: adjust minutes", 1, t.on_surface_variant);
            self.round_rect(55, 229, 130, 38, 19, lime);
            self.text_center(242, "Start timer", 1, t.on_primary);
            return;
        }
        if f.timer_secs > 0 {
            let time = Self::mmss_text(f.timer_secs);
            self.text_center(71, core::str::from_utf8(&time).unwrap_or("00:00"), 1, lime);
        }
        for (i, label) in ["5", "10", "15", "30"].iter().enumerate() {
            let x = 18 + (i % 2) as i32 * 105;
            let y = 90 + (i / 2) as i32 * 67;
            self.round_rect(
                x,
                y,
                99,
                61,
                29,
                Self::focus_fill(lavender, f.cursor as usize == i),
            );
            self.text_center_x(
                x + 49,
                y + 12,
                label,
                2,
                if f.cursor as usize == i { Self::focus_ink(t, true) } else { ink },
            );
            self.text_center_x(
                x + 49,
                y + 39,
                "Min",
                1,
                if f.cursor as usize == i { Self::focus_ink(t, true) } else { ink },
            );
        }
        self.round_rect(
            63,
            235,
            114,
            35,
            17,
            Self::focus_fill(lime, f.cursor == 4),
        );
        self.text_center(
            244,
            "+",
            2,
            t.on_primary,
        );
    }

    /// Выбирает экран инструмента часов и рисует его актуальное состояние.
    pub(super) fn clock_tools(&mut self, f: MaterialFrame, t: Theme) {
        if f.timer_editor == 3 {
            self.stopwatch_page(f, t);
            return;
        }
        if f.timer_editor != 0 {
            self.timer_presets(f, t);
            return;
        }
        self.title("CLOCK", t);
        let cur = f.cursor.min(4) as usize;
        let timer = Self::mmss_text(if f.timer_secs == 0 {
            f.timer_duration_secs
        } else {
            f.timer_secs
        });
        let stopwatch = Self::mmss_text(f.stopwatch_secs);
        let timer_s = core::str::from_utf8(&timer).unwrap_or("05:00");
        let stopwatch_s = core::str::from_utf8(&stopwatch).unwrap_or("00:00");
        let labels = ["TIMER", "STOPWATCH", "ALARM", "RESET STOPWATCH", "BACK"];
        let start = cur.saturating_sub(1).min(2);
        for row in 0..3 {
            let i = (start + row).min(4);
            let y = 57 + row as i32 * 64;
            match i {
                0 => self.info_card(y, labels[i], timer_s, i == cur, t),
                1 => self.info_card(y, labels[i], stopwatch_s, i == cur, t),
                2 => self.info_card(y, labels[i], if f.alarm_enabled { "ON" } else { "OFF" }, i == cur, t),
                _ => self.wear_row(y, labels[i], i == cur, t),
            }
        }
    }

    /// Рисует экран alarm с актуальным состоянием и фокусом.
    pub(super) fn alarm_editor(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Alarm", t);
        let time = Self::time_text(f.alarm_minutes);
        self.text_center(
            68,
            core::str::from_utf8(&time).unwrap_or("07:00"),
            2,
            t.primary,
        );
        let labels = [
            if f.alarm_enabled {
                "Alarm on"
            } else {
                "Alarm off"
            },
            "Hour +",
            "Minute +",
            "Back",
        ];
        for (i, label) in labels.iter().enumerate() {
            self.round_rect(
                22,
                103 + i as i32 * 39,
                196,
                34,
                17,
                Self::focus_fill(t.surface_high, f.cursor as usize == i),
            );
            self.text_center(114 + i as i32 * 39, label, 1, Self::focus_ink(t, f.cursor as usize == i));
        }
    }
}
