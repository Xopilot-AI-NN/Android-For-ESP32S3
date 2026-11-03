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
        self.photo_header(f, "Часы", t);
        let time = Self::time_text(f.time_minutes);
        self.text_center_raw(
            61,
            if f.time_valid {
                core::str::from_utf8(&time).unwrap_or("--:--")
            } else {
                "--:--"
            },
            3,
            t.primary,
        );
        self.text_center_raw(
            105,
            &alloc::format!("{:02}.{:02}", f.date_day, f.date_month),
            1,
            t.on_surface_variant,
        );
        let weather = crate::weather::snapshot();
        if weather.valid {
            self.text_center_raw(
                126,
                &alloc::format!("{} °C", weather.temp / 10),
                1,
                t.on_surface_variant,
            );
        }
        let labels = ["Будильник", "Таймер", "Секундомер", "Мировые часы", "Назад"];
        let icons = [
            Symbol::Alarm,
            Symbol::Timer,
            Symbol::Clock,
            Symbol::World,
            Symbol::Back,
        ];
        let start = if f.cursor == 4 { 1 } else { 0 };
        for row in 0..4 {
            let i = start + row;
            let y = 149 + row as i32 * 30;
            let selected = f.cursor as usize == i;
            let ink = Self::focus_ink(t, selected);
            self.round_rect(
                10,
                y,
                220,
                27,
                12,
                if selected { t.primary } else { t.surface_high },
            );
            self.symbol(icons[i], 28, y + 13, ink);
            self.text_fitted_raw(48, y + 5, labels[i], 1, 171, ink);
        }
    }

    /// Рисует экран секундомера с актуальным состоянием и фокусом.
    pub(super) fn stopwatch_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Секундомер", t);
        let text = Self::mmss_text(f.stopwatch_secs);
        self.text_center_raw(
            100,
            core::str::from_utf8(&text).unwrap_or("00:00"),
            3,
            t.on_surface,
        );
        self.text_center_raw(
            157,
            if f.stopwatch_running {
                "Работает"
            } else {
                "На паузе"
            },
            1,
            t.on_surface_variant,
        );
        let labels = [
            if f.stopwatch_running {
                "Пауза"
            } else {
                "Пуск"
            },
            "Сброс",
            "Назад",
        ];
        for (i, label) in labels.iter().enumerate() {
            let x = 8 + i as i32 * 77;
            let selected = f.cursor as usize == i;
            self.round_rect(
                x,
                228,
                72,
                39,
                18,
                if selected { t.primary } else { t.surface_high },
            );
            self.text_fitted_raw(x + 5, 240, label, 1, 62, Self::focus_ink(t, selected));
        }
    }

    /// Рисует предустановки или редактирование длительности таймера.
    pub(super) fn timer_presets(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Таймер", t);
        let duration = if f.timer_secs > 0 {
            f.timer_secs
        } else {
            f.timer_duration_secs
        };
        let text = Self::mmss_text(duration);
        if f.timer_editor == 2 {
            self.text_center_raw(
                102,
                core::str::from_utf8(&text).unwrap_or("05:00"),
                3,
                t.primary,
            );
            self.text_center_raw(172, "Колесо: минуты", 1, t.on_surface_variant);
            self.round_rect(15, 229, 210, 40, 20, t.primary);
            self.text_center_raw(242, "Запустить", 1, t.on_primary);
            return;
        }
        let progress = if f.timer_secs > 0 {
            (u32::from(f.timer_secs) * 60 / u32::from(f.timer_duration_secs.max(1))).min(60)
        } else {
            60
        };
        for tick in 0..60 {
            let p = super::dial::point(tick * 60, 62);
            let q = super::dial::point(((tick + 1) % 60) * 60, 62);
            self.line(
                120 + p.0,
                121 + p.1,
                120 + q.0,
                121 + q.1,
                6,
                if u32::from(tick) < progress {
                    t.primary
                } else {
                    t.surface_high
                },
            );
        }
        self.text_center_raw(
            105,
            core::str::from_utf8(&text).unwrap_or("05:00"),
            3,
            t.on_surface,
        );
        for (i, label) in ["5 м", "10 м", "15 м", "30 м"].iter().enumerate() {
            let x = 9 + i as i32 * 57;
            let selected = f.cursor as usize == i;
            self.round_rect(
                x,
                197,
                51,
                27,
                11,
                if selected { t.primary } else { t.surface_high },
            );
            self.text_fitted_raw(x + 5, 204, label, 1, 41, Self::focus_ink(t, selected));
        }
        for (i, label) in [
            "Другое",
            if f.timer_secs > 0 {
                "Стоп"
            } else {
                "Пуск"
            },
        ]
        .iter()
        .enumerate()
        {
            let x = 10 + i as i32 * 114;
            let selected = f.cursor as usize == 4 + i;
            self.round_rect(
                x,
                237,
                106,
                35,
                17,
                if selected { t.primary } else { t.surface_high },
            );
            self.text_center_x(x + 53, 247, label, 1, Self::focus_ink(t, selected));
        }
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
                2 => self.info_card(
                    y,
                    labels[i],
                    if f.alarm_enabled { "ON" } else { "OFF" },
                    i == cur,
                    t,
                ),
                _ => self.wear_row(y, labels[i], i == cur, t),
            }
        }
    }

    /// Рисует экран alarm с актуальным состоянием и фокусом.
    pub(super) fn alarm_editor(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Будильник", t);
        self.round_rect(10, 65, 220, 89, 20, t.surface);
        let hour = alloc::format!("{:02}", f.alarm_minutes / 60);
        let minute = alloc::format!("{:02}", f.alarm_minutes % 60);
        for (i, text) in [&hour, &minute].iter().enumerate() {
            let x = 33 + i as i32 * 103;
            let selected = f.cursor as usize == i + 1;
            self.round_rect(
                x,
                86,
                70,
                43,
                11,
                if selected { t.primary } else { t.surface_high },
            );
            self.text_center_x(x + 35, 92, text, 3, Self::focus_ink(t, selected));
            self.line(x + 29, 76, x + 35, 71, 2, t.primary);
            self.line(x + 35, 71, x + 41, 76, 2, t.primary);
            self.line(x + 29, 140, x + 35, 145, 2, t.primary);
            self.line(x + 35, 145, x + 41, 140, 2, t.primary);
        }
        self.text_center_raw(95, ":", 3, t.on_surface);
        self.round_rect(
            10,
            171,
            220,
            35,
            15,
            if f.cursor == 0 {
                t.primary
            } else {
                t.surface_high
            },
        );
        self.text_raw(22, 180, "Включён", 1, Self::focus_ink(t, f.cursor == 0));
        self.round_rect(
            176,
            179,
            40,
            19,
            9,
            if f.alarm_enabled {
                t.primary
            } else {
                t.outline
            },
        );
        self.circle(
            if f.alarm_enabled { 207 } else { 185 },
            188,
            7,
            t.on_surface,
        );
        self.round_rect(
            10,
            234,
            220,
            37,
            18,
            if f.cursor == 3 {
                t.primary
            } else {
                t.surface_high
            },
        );
        self.text_center_raw(245, "Сохранить", 1, Self::focus_ink(t, f.cursor == 3));
    }
}
