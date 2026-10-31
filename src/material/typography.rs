//! typography Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn glyph(&mut self, x: i32, y: i32, ch: u8, scale: i32, c: u16) {
        let g = glyph(ch);
        let scale = scale.max(1);
        for (col, bits) in g.iter().enumerate() {
            for row in 0..7 {
                if (bits >> row) & 1 != 0 {
                    self.fill_rect(x + col as i32 * scale, y + row * scale, scale, scale, c);
                }
            }
        }
    }

    pub(super) fn text(&mut self, x: i32, y: i32, text: &str, scale: i32, c: u16) {
        let mut cx = x;
        for ch in text.bytes() { self.glyph(cx, y, ch, scale, c); cx += 6 * scale; }
    }

    pub(super) fn text_width(text: &str, scale: i32) -> i32 {
        if text.is_empty() { 0 } else { (text.len() as i32 * 6 - 1) * scale }
    }

    pub(super) fn text_center(&mut self, y: i32, text: &str, scale: i32, c: u16) {
        self.text((WIDTH as i32 - Self::text_width(text, scale)) / 2, y, text, scale, c);
    }

    pub(super) fn text_center_x(&mut self, cx: i32, y: i32, text: &str, scale: i32, c: u16) {
        self.text(cx - Self::text_width(text, scale) / 2, y, text, scale, c);
    }

    pub(super) fn time_text(mins: u16) -> [u8; 5] {
        let h = (mins / 60) % 24;
        let m = mins % 60;
        [b'0' + (h / 10) as u8, b'0' + (h % 10) as u8, b':', b'0' + (m / 10) as u8, b'0' + (m % 10) as u8]
    }

    pub(super) fn mmss_text(secs: u16) -> [u8; 5] {
        let m = (secs / 60).min(99);
        let s = secs % 60;
        [b'0' + (m / 10) as u8, b'0' + (m % 10) as u8, b':', b'0' + (s / 10) as u8, b'0' + (s % 10) as u8]
    }
}
