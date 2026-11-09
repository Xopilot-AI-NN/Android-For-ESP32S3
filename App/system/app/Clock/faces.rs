// Модуль циферблатов.
//! Watch faces use actual clock/radio state; unavailable health data is not fabricated.
use super::*;
impl Surface {
    /// Выбирает текущий вариант циферблата.
    pub(super) fn watchface(&mut self, f: MaterialFrame, t: Theme) {
        match f.watch_face {
            1 => self.analog_face(f, t),
            2 => self.complication_face(f, t),
            3 => self.minimal_face(f, t),
            4 => self.dashboard_face(f, t),
            _ => self.digital_face(f, t),
        }
    }
    /// Рисует дату только при наличии календарных данных.
    fn face_date(&mut self, f: MaterialFrame, y: i32, t: Theme) {
        if f.date_day != 0 {
            self.text_center(
                y,
                &alloc::format!("{:02}.{:02}", f.date_day, f.date_month),
                1,
                t.primary,
            );
        }
    }
    /// Рисует цифровой циферблат с действительными системными индикаторами.
    fn digital_face(&mut self, f: MaterialFrame, t: Theme) {
        self.ring(120, 140, 111, 1, t.surface_high);
        self.face_date(f, 30, t);
        let tm = Self::time_text(f.time_minutes);
        self.clock_text(68, &tm[..2], t.primary);
        self.clock_text(137, &tm[3..], t.primary);
        self.symbol(
            Symbol::Wifi,
            36,
            99,
            if f.wifi_link_state == 4 {
                t.primary
            } else {
                t.outline
            },
        );
        self.symbol(
            Symbol::Bluetooth,
            204,
            99,
            if f.bt_status >= 2 && f.bt_status <= 4 {
                t.primary
            } else {
                t.outline
            },
        );
        self.symbol(
            Symbol::Alarm,
            36,
            177,
            if f.alarm_enabled {
                t.primary
            } else {
                t.outline
            },
        );
        self.symbol(
            Symbol::Timer,
            204,
            177,
            if f.timer_secs > 0 {
                t.primary
            } else {
                t.outline
            },
        );
        if f.timer_secs > 0 {
            let timer = Self::mmss_text(f.timer_secs);
            self.text_center(
                228,
                core::str::from_utf8(&timer).unwrap_or(""),
                1,
                t.on_surface,
            );
        }
        if f.notes > 0 {
            self.circle(120, 258, 3, t.primary);
        }
    }
    /// Рисует деления и стрелки аналогового циферблата по RTC.
    fn analog_face(&mut self, f: MaterialFrame, t: Theme) {
        self.face_date(f, 34, t);
        self.ring(120, 151, 98, 1, t.surface_high);
        for tick in 0..60 {
            let phase = tick * 60;
            let a = super::dial::point(phase, 93);
            let b = super::dial::point(phase, if tick % 5 == 0 { 83 } else { 89 });
            self.line(
                120 + a.0,
                151 + a.1,
                120 + b.0,
                151 + b.1,
                if tick % 5 == 0 { 2 } else { 1 },
                if tick % 5 == 0 { t.primary } else { t.outline },
            );
        }
        for (label, phase) in [("12", 0), ("3", 900), ("6", 1800), ("9", 2700)] {
            let p = super::dial::point(phase, 74);
            self.text_center_x(120 + p.0, 144 + p.1, label, 1, t.on_surface_variant);
        }
        let minute = f.time_minutes % 60;
        let hour = f.time_minutes / 60 % 12;
        let seconds = if f.show_seconds {
            u16::from(f.time_seconds)
        } else {
            0
        };
        let h = super::dial::point(hour * 300 + minute * 5 + seconds / 12, 49);
        let m = super::dial::point(minute * 60 + seconds, 73);
        let s = super::dial::point(
            u16::from(f.time_seconds) * 60 + u16::from(f.animation_phase) * 60 / 255,
            84,
        );
        self.line(120, 151, 120 + h.0, 151 + h.1, 5, t.on_surface);
        self.line(120, 151, 120 + m.0, 151 + m.1, 3, t.primary);
        if f.show_seconds {
            self.line(
                120 - s.0 / 5,
                151 - s.1 / 5,
                120 + s.0,
                151 + s.1,
                1,
                t.error,
            );
        }
        self.circle(120, 151, 4, t.primary);
        if f.alarm_enabled {
            let time = Self::time_text(f.alarm_minutes);
            self.text_center(259, &alloc::format!("Будильник {}", core::str::from_utf8(&time).unwrap_or("--:--")), 1, t.on_surface_variant);
        }
    }
    /// Рисует минималистичный циферблат и при необходимости секунды.
    fn minimal_face(&mut self, f: MaterialFrame, t: Theme) {
        self.face_date(f, 67, t);
        self.big_time(110, f.time_minutes, f.time_valid, t.on_surface);
        if f.show_seconds { self.text_center_raw(194, &alloc::format!("{:02}", f.time_seconds), 2, t.primary); }
        self.round_rect(88, 227, 64, 3, 1, t.primary);
        self.text_center(247, if f.time_valid { "" } else { "Время не синхронизировано" }, 1, t.on_surface_variant);
        if f.notes > 0 { self.text_center(264, &alloc::format!("Уведомлений: {}", f.notes), 1, t.primary); }
    }
    /// Рисует панель часов с последними погодными данными, будильником и таймером.
    fn dashboard_face(&mut self, f: MaterialFrame, t: Theme) {
        self.face_date(f, 36, t);
        self.big_time(74, f.time_minutes, f.time_valid, t.on_surface);
        let weather = f.app.weather;
        for (i, symbol) in [Symbol::Weather, Symbol::Alarm, Symbol::Timer].into_iter().enumerate() {
            let x = 9 + i as i32 * 78;
            self.round_rect(x, 166, 70, 82, 22, t.surface);
            self.symbol(symbol, x + 35, 188, t.primary);
            let value = match i {
                0 if weather.valid => alloc::format!("{}°", (weather.temp + if weather.temp >= 0 { 5 } else { -5 }) / 10),
                1 if f.alarm_enabled => core::str::from_utf8(&Self::time_text(f.alarm_minutes)).unwrap_or("--:--").into(),
                2 if f.timer_secs > 0 => core::str::from_utf8(&Self::mmss_text(f.timer_secs)).unwrap_or("--:--").into(),
                _ => alloc::string::String::from("--"),
            };
            self.text_center_x(x + 35, 217, &value, 1, t.on_surface);
        }
        self.text_center(263, if weather.valid { "Погода последнего обновления" } else { "Обновите город в Погоде" }, 1, t.on_surface_variant);
    }
}
