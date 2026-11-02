//! Watch faces use actual clock/radio state; unavailable health data is not fabricated.
use super::*;
impl Surface {
    pub(super) fn watchface(&mut self, f: MaterialFrame, t: Theme) {
        match f.watch_face {
            1 => self.analog_face(f, t),
            2 => self.complication_face(f, t),
            _ => self.digital_face(f, t),
        }
    }
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
    fn analog_face(&mut self, f: MaterialFrame, t: Theme) {
        self.face_date(f, 17, t);
        self.ring(120, 144, 101, 1, t.surface_high);
        for hour in 0..12 {
            let phase = hour * 300;
            let a = super::dial::point(phase, 94);
            let b = super::dial::point(phase, if hour % 3 == 0 { 83 } else { 88 });
            self.line(
                120 + a.0,
                144 + a.1,
                120 + b.0,
                144 + b.1,
                if hour % 3 == 0 { 3 } else { 2 },
                t.on_surface_variant,
            );
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
        self.line(120, 144, 120 + h.0, 144 + h.1, 5, t.on_surface);
        self.line(120, 144, 120 + m.0, 144 + m.1, 3, t.primary);
        if f.show_seconds {
            self.line(
                120 - s.0 / 5,
                144 - s.1 / 5,
                120 + s.0,
                144 + s.1,
                1,
                t.error,
            );
        }
        self.circle(120, 144, 4, t.primary);
    }
}
