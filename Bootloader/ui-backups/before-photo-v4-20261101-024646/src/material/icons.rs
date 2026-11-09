//! icons Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn icon_wifi(&mut self, cx: i32, cy: i32, c: u16) {
        // Compact Wear-style Wi-Fi glyph built from three chevrons/dot.
        self.line(cx - 12, cy - 6, cx, cy - 12, 2, c);
        self.line(cx, cy - 12, cx + 12, cy - 6, 2, c);
        self.line(cx - 8, cy, cx, cy - 4, 2, c);
        self.line(cx, cy - 4, cx + 8, cy, 2, c);
        self.line(cx - 4, cy + 6, cx, cy + 4, 2, c);
        self.line(cx, cy + 4, cx + 4, cy + 6, 2, c);
        self.circle(cx, cy + 10, 2, c);
    }

    pub(super) fn icon_bt(&mut self, cx: i32, cy: i32, c: u16) {
        self.line(cx, cy - 13, cx, cy + 13, 2, c);
        self.line(cx, cy - 13, cx + 8, cy - 5, 2, c);
        self.line(cx + 8, cy - 5, cx - 7, cy + 7, 2, c);
        self.line(cx - 7, cy - 7, cx + 8, cy + 5, 2, c);
        self.line(cx + 8, cy + 5, cx, cy + 13, 2, c);
    }

    pub(super) fn icon_dnd(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 12, 3, c);
        self.round_rect(cx - 7, cy - 2, 14, 4, 2, c);
    }

    pub(super) fn icon_airplane(&mut self, cx: i32, cy: i32, c: u16) {
        self.line(cx - 13, cy, cx + 13, cy, 3, c);
        self.line(cx + 3, cy, cx - 3, cy - 9, 3, c);
        self.line(cx + 3, cy, cx - 3, cy + 9, 3, c);
        self.line(cx - 7, cy, cx - 12, cy - 5, 2, c);
        self.line(cx - 7, cy, cx - 12, cy + 5, 2, c);
    }

    pub(super) fn icon_sun(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 7, 2, c);
        for &(x0, y0, x1, y1) in &[
            (0, -14, 0, -10),
            (0, 10, 0, 14),
            (-14, 0, -10, 0),
            (10, 0, 14, 0),
            (-10, -10, -7, -7),
            (7, 7, 10, 10),
            (-10, 10, -7, 7),
            (7, -7, 10, -10),
        ] {
            self.line(cx + x0, cy + y0, cx + x1, cy + y1, 2, c);
        }
    }

    pub(super) fn icon_settings(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 9, 3, c);
        self.circle(cx, cy, 3, c);
        self.line(cx, cy - 15, cx, cy - 10, 3, c);
        self.line(cx, cy + 10, cx, cy + 15, 3, c);
        self.line(cx - 15, cy, cx - 10, cy, 3, c);
        self.line(cx + 10, cy, cx + 15, cy, 3, c);
    }

    pub(super) fn icon_bell(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 9, 3, c);
        self.fill_rect(cx - 10, cy + 1, 20, 8, c);
        self.circle(cx, cy + 12, 2, c);
    }

    pub(super) fn icon_clock(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 12, 2, c);
        self.line(cx, cy, cx, cy - 7, 2, c);
        self.line(cx, cy, cx + 6, cy + 3, 2, c);
    }

    pub(super) fn icon_media(&mut self, cx: i32, cy: i32, c: u16) {
        // Material-style play glyph with a compact media baseline.
        for i in 0..13 {
            let half = if i < 7 { i } else { 12 - i };
            self.line(cx - 6 + i, cy - half, cx - 6 + i, cy + half, 2, c);
        }
        self.line(cx - 11, cy + 13, cx + 11, cy + 13, 2, c);
    }

    pub(super) fn icon_info(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 12, 2, c);
        self.circle(cx, cy - 6, 2, c);
        self.round_rect(cx - 2, cy - 1, 4, 10, 2, c);
    }

    pub(super) fn icon_grid(&mut self, cx: i32, cy: i32, c: u16) {
        for yy in [-7, 0, 7] {
            for xx in [-7, 0, 7] {
                self.circle(cx + xx, cy + yy, 2, c);
            }
        }
    }

    pub(super) fn icon_assistant(&mut self, cx: i32, cy: i32, c: u16) {
        // Four-point sparkle: recognizable assistant affordance without
        // pretending to be a proprietary service logo.
        self.line(cx, cy - 13, cx, cy + 13, 2, c);
        self.line(cx - 13, cy, cx + 13, cy, 2, c);
        self.line(cx - 8, cy - 8, cx + 8, cy + 8, 2, c);
        self.line(cx - 8, cy + 8, cx + 8, cy - 8, 2, c);
        self.circle(cx, cy, 3, c);
    }

    pub(super) fn icon_connectivity(&mut self, cx: i32, cy: i32, c: u16) {
        self.icon_wifi(cx - 3, cy - 2, c);
        self.circle(cx + 10, cy + 10, 3, c);
    }

    pub(super) fn app_icon(&mut self, icon: u8, cx: i32, cy: i32, c: u16) {
        match icon {
            0 => self.icon_clock(cx, cy, c),
            1 => self.icon_media(cx, cy, c),
            2 => self.icon_settings(cx, cy, c),
            3 => self.icon_connectivity(cx, cy, c),
            4 => self.icon_info(cx, cy, c),
            5 => self.icon_assistant(cx, cy, c),
            _ => self.icon_grid(cx, cy, c),
        }
    }

    pub(super) fn app_accent(icon: u8) -> u16 {
        match icon {
            0 => rgb565(181, 232, 211), // clock mint
            1 => rgb565(255, 191, 171), // media coral
            2 => rgb565(174, 203, 255), // settings blue
            3 => rgb565(102, 221, 205), // connectivity teal
            4 => rgb565(218, 190, 255), // system lavender
            5 => rgb565(126, 225, 165), // assistant green
            _ => rgb565(50, 59, 54),
        }
    }

    pub(super) fn icon_back(&mut self, cx: i32, cy: i32, c: u16) {
        self.line(cx + 8, cy, cx - 8, cy, 3, c);
        self.line(cx - 8, cy, cx - 1, cy - 7, 3, c);
        self.line(cx - 8, cy, cx - 1, cy + 7, 3, c);
    }

    pub(super) fn icon_lock(&mut self, cx: i32, cy: i32, c: u16) {
        self.round_rect(cx - 9, cy - 1, 18, 15, 4, c);
        // Hollow the body center slightly using the surrounding black surface
        // is not possible generically here; keep the compact filled Wear glyph.
        self.ring(cx, cy - 5, 7, 2, c);
    }

    pub(super) fn icon_sound(&mut self, cx: i32, cy: i32, c: u16) {
        self.fill_rect(cx - 11, cy - 4, 6, 8, c);
        self.line(cx - 5, cy - 4, cx + 2, cy - 10, 3, c);
        self.line(cx - 5, cy + 4, cx + 2, cy + 10, 3, c);
        self.line(cx + 6, cy - 7, cx + 10, cy, 2, c);
        self.line(cx + 10, cy, cx + 6, cy + 7, 2, c);
    }

    pub(super) fn icon_person(&mut self, cx: i32, cy: i32, c: u16) {
        self.circle(cx, cy - 8, 4, c);
        self.line(cx, cy - 2, cx, cy + 10, 3, c);
        self.line(cx - 9, cy + 2, cx + 9, cy + 2, 3, c);
        self.line(cx, cy + 10, cx - 7, cy + 15, 3, c);
        self.line(cx, cy + 10, cx + 7, cy + 15, 3, c);
    }

    pub(super) fn row_icon(&mut self, label: &str, cx: i32, cy: i32, c: u16) {
        match label {
            "CONNECTIVITY" | "WI-FI" => self.icon_wifi(cx, cy, c),
            "BLUETOOTH" => self.icon_bt(cx, cy, c),
            "APPS & NOTIFS" | "NOTIFICATIONS" => self.icon_bell(cx, cy, c),
            "DISPLAY" | "BRIGHTNESS" => self.icon_sun(cx, cy, c),
            "SOUND & VIBRATION" => self.icon_sound(cx, cy, c),
            "ACCESSIBILITY" => self.icon_person(cx, cy, c),
            "SECURITY" | "SCREEN LOCK" => self.icon_lock(cx, cy, c),
            "DATE & TIME" | "AUTOMATIC TIME" | "SET HOUR" | "SET MINUTE" | "TIME ZONE" => {
                self.icon_clock(cx, cy, c)
            }
            "ABOUT" | "SYSTEM INFO" | "BUILD NUMBER" => self.icon_info(cx, cy, c),
            "DEVELOPER OPTIONS" | "SYSTEM" | "SETTINGS" => self.icon_settings(cx, cy, c),
            "MEDIA CONTROLS" => self.icon_media(cx, cy, c),
            "AIRPLANE MODE" => self.icon_airplane(cx, cy, c),
            "BACK" => self.icon_back(cx, cy, c),
            _ => self.circle(cx, cy, 3, c),
        }
    }

    pub(super) fn numeral_segment(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: u16) {
        self.line(x0, y0, x1, y1, 5, c);
    }

    pub(super) fn numeral(&mut self, x: i32, y: i32, ch: u8, c: u16) {
        // Rounded seven-stroke numeral used only for the hero clock. The rest
        // of SystemUI keeps the compact bitmap font to save flash/RAM.
        let seg = match ch {
            b'0' => 0b1110111,
            b'1' => 0b0100100,
            b'2' => 0b1011101,
            b'3' => 0b1101101,
            b'4' => 0b0101110,
            b'5' => 0b1101011,
            b'6' => 0b1111011,
            b'7' => 0b0100101,
            b'8' => 0b1111111,
            b'9' => 0b1101111,
            _ => 0,
        };
        // bit order: top, upper-right, lower-right, bottom, lower-left,
        // upper-left, middle.
        if seg & 0b0000001 != 0 {
            self.numeral_segment(x + 6, y, x + 28, y, c);
        }
        if seg & 0b0000010 != 0 {
            self.numeral_segment(x + 31, y + 5, x + 31, y + 25, c);
        }
        if seg & 0b0000100 != 0 {
            self.numeral_segment(x + 31, y + 33, x + 31, y + 53, c);
        }
        if seg & 0b0001000 != 0 {
            self.numeral_segment(x + 6, y + 58, x + 28, y + 58, c);
        }
        if seg & 0b0010000 != 0 {
            self.numeral_segment(x + 3, y + 33, x + 3, y + 53, c);
        }
        if seg & 0b0100000 != 0 {
            self.numeral_segment(x + 3, y + 5, x + 3, y + 25, c);
        }
        if seg & 0b1000000 != 0 {
            self.numeral_segment(x + 6, y + 29, x + 28, y + 29, c);
        }
    }

    pub(super) fn big_time(&mut self, y: i32, minutes: u16, _valid: bool, c: u16) {
        let h = (minutes / 60) % 24;
        let m = minutes % 60;
        let chars = [
            b'0' + (h / 10) as u8,
            b'0' + (h % 10) as u8,
            b'0' + (m / 10) as u8,
            b'0' + (m % 10) as u8,
        ];
        self.numeral(32, y, chars[0], c);
        self.numeral(72, y, chars[1], c);
        self.circle(120, y + 20, 3, c);
        self.circle(120, y + 40, 3, c);
        self.numeral(136, y, chars[2], c);
        self.numeral(176, y, chars[3], c);
    }
}
