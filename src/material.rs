extern crate alloc;

use alloc::{vec, vec::Vec};

use crate::font::glyph;

pub const WIDTH: usize = 240;
pub const HEIGHT: usize = 280;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterialFrame {
    pub screen: u8,
    pub cursor: u8,
    pub brightness: u8,
    pub dnd: bool,
    pub airplane: bool,
    pub theme: u8,
    pub locked: bool,
    pub wifi: bool,
    pub bt: bool,
    pub adb: bool,
    pub notes: u8,
    pub time_minutes: u16,
    pub swap_pages: u8,
}

impl MaterialFrame {
    pub fn parse(raw: &str) -> Option<Self> {
        let mut p = raw.split('|');
        if p.next()? != "M3" { return None; }
        let screen = p.next()?.parse::<u8>().ok()?.min(8);
        let cursor = p.next()?.parse::<u8>().ok()?.min(15);
        let brightness = p.next()?.parse::<u8>().ok()?.min(100);
        let dnd = p.next()? == "1";
        let airplane = p.next()? == "1";
        let theme = p.next()?.parse::<u8>().ok()? % 4;
        let locked = p.next()? == "1";
        let wifi = p.next()? == "1";
        let bt = p.next()? == "1";
        let adb = p.next()? == "1";
        let notes = p.next()?.parse::<u8>().ok()?.min(7);
        let time_minutes = p.next()?.parse::<u16>().ok()? % 1440;
        if p.next().is_some() { return None; }
        Some(Self {
            screen, cursor, brightness, dnd, airplane, theme, locked,
            wifi, bt, adb, notes, time_minutes, swap_pages: 0,
        })
    }

    pub fn set_swap_pages(&mut self, pages: u8) { self.swap_pages = pages; }

    pub fn fallback(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self.screen {
            0 => ("LOCKED", "AOSP WEAR OS", "PRESS CROWN", ""),
            1 => ("WATCHFACE", "CLOCK", "MESSAGES", "QUICK SETTINGS"),
            2 => ("APPS", "MESSAGES", "SETTINGS", "CLOCK / CONNECT"),
            3 => ("NOTIFICATIONS", "SYSTEM", "PHONE LINK", "BACK"),
            4 => ("QUICK SETTINGS", "WIFI / BT", "ADB / DND", "BRIGHTNESS"),
            5 => ("SETTINGS", "DISPLAY", "THEME", "CONNECTIVITY"),
            6 => ("CLOCK", "TIME", "RTC/NTP NEXT", "BACK"),
            7 => ("CONNECTIVITY", "WIFI", "BLUETOOTH", "WADB"),
            _ => ("SYSTEM", "AOSP WEAR OS", "ANDROID 17 QPR1", "ESP32-S3"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }
impl Rect {
    pub const FULL: Self = Self { x: 0, y: 0, w: WIDTH as i32, h: HEIGHT as i32 };
    pub fn x1(self) -> i32 { self.x + self.w - 1 }
    pub fn y1(self) -> i32 { self.y + self.h - 1 }
}

#[derive(Clone, Copy)]
struct Theme {
    bg: u16,
    surface: u16,
    surface_high: u16,
    primary: u16,
    on_primary: u16,
    primary_container: u16,
    on_primary_container: u16,
    on_surface: u16,
    on_surface_variant: u16,
    outline: u16,
    error: u16,
}

const fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    (((r as u16) & 0xF8) << 8) | (((g as u16) & 0xFC) << 3) | ((b as u16) >> 3)
}

/// Wear OS 6 / Pixel Watch inspired AMOLED palettes.  The background stays
/// true black while the chosen watch-face accent colors the whole system.
fn theme(index: u8) -> Theme {
    match index % 4 {
        1 => Theme {
            bg: rgb565(0, 0, 0), surface: rgb565(16, 21, 31), surface_high: rgb565(27, 34, 47),
            primary: rgb565(174, 203, 255), on_primary: rgb565(8, 48, 88),
            primary_container: rgb565(32, 69, 111), on_primary_container: rgb565(219, 230, 255),
            on_surface: rgb565(241, 242, 248),
            on_surface_variant: rgb565(191, 198, 211), outline: rgb565(118, 127, 143), error: rgb565(255, 180, 171),
        },
        2 => Theme {
            bg: rgb565(0, 0, 0), surface: rgb565(24, 18, 28), surface_high: rgb565(39, 29, 45),
            primary: rgb565(224, 187, 255), on_primary: rgb565(72, 31, 97),
            primary_container: rgb565(100, 55, 128), on_primary_container: rgb565(244, 220, 255),
            on_surface: rgb565(246, 239, 247),
            on_surface_variant: rgb565(207, 194, 211), outline: rgb565(142, 128, 147), error: rgb565(255, 180, 171),
        },
        3 => Theme {
            bg: rgb565(0, 0, 0), surface: rgb565(29, 19, 17), surface_high: rgb565(47, 31, 27),
            primary: rgb565(255, 184, 161), on_primary: rgb565(86, 33, 16),
            primary_container: rgb565(119, 55, 34), on_primary_container: rgb565(255, 221, 211),
            on_surface: rgb565(249, 238, 233),
            on_surface_variant: rgb565(216, 195, 188), outline: rgb565(150, 129, 123), error: rgb565(255, 180, 171),
        },
        _ => Theme {
            bg: rgb565(0, 0, 0), surface: rgb565(14, 24, 18), surface_high: rgb565(24, 39, 29),
            primary: rgb565(126, 225, 165), on_primary: rgb565(0, 57, 29),
            primary_container: rgb565(23, 80, 49), on_primary_container: rgb565(163, 249, 194),
            on_surface: rgb565(238, 244, 239),
            on_surface_variant: rgb565(190, 203, 194), outline: rgb565(126, 143, 132), error: rgb565(255, 180, 171),
        },
    }
}

pub struct Surface {
    pixels: Vec<u16>,
}

impl Surface {
    pub fn new() -> Self { Self { pixels: vec![0; WIDTH * HEIGHT] } }
    pub fn pixels(&self) -> &[u16] { &self.pixels }

    pub fn render(&mut self, frame: MaterialFrame) -> Rect {
        let t = theme(frame.theme);
        self.fill_rect(0, 0, WIDTH as i32, HEIGHT as i32, t.bg);
        match frame.screen {
            0 => self.lock_screen(frame, t),
            1 => self.watchface(frame, t),
            2 => self.launcher(frame, t),
            3 => self.notifications(frame, t),
            4 => self.quick_settings(frame, t),
            5 => self.settings(frame, t),
            6 => self.clock(frame, t),
            7 => self.connectivity(frame, t),
            _ => self.about(frame, t),
        }
        Rect::FULL
    }

    fn pixel(&mut self, x: i32, y: i32, c: u16) {
        if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
            self.pixels[y as usize * WIDTH + x as usize] = c;
        }
    }

    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u16) {
        if w <= 0 || h <= 0 { return; }
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = (x + w).min(WIDTH as i32).max(0) as usize;
        let y1 = (y + h).min(HEIGHT as i32).max(0) as usize;
        for yy in y0..y1 { self.pixels[yy * WIDTH + x0..yy * WIDTH + x1].fill(c); }
    }

    fn round_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: i32, c: u16) {
        if w <= 0 || h <= 0 { return; }
        let r = r.max(0).min(w / 2).min(h / 2);
        self.fill_rect(x + r, y, w - 2 * r, h, c);
        self.fill_rect(x, y + r, w, h - 2 * r, c);
        for dy in 0..r {
            for dx in 0..r {
                let ox = r - dx;
                let oy = r - dy;
                if ox * ox + oy * oy <= r * r {
                    self.pixel(x + dx, y + dy, c);
                    self.pixel(x + w - 1 - dx, y + dy, c);
                    self.pixel(x + dx, y + h - 1 - dy, c);
                    self.pixel(x + w - 1 - dx, y + h - 1 - dy, c);
                }
            }
        }
    }

    fn circle(&mut self, cx: i32, cy: i32, r: i32, c: u16) {
        for y in -r..=r {
            for x in -r..=r {
                if x * x + y * y <= r * r { self.pixel(cx + x, cy + y, c); }
            }
        }
    }

    fn glyph(&mut self, x: i32, y: i32, ch: u8, scale: i32, c: u16) {
        let g = glyph(ch.to_ascii_uppercase());
        let scale = scale.max(1);
        for (col, bits) in g.iter().enumerate() {
            for row in 0..7 {
                if (bits >> row) & 1 != 0 {
                    self.fill_rect(x + col as i32 * scale, y + row * scale, scale, scale, c);
                }
            }
        }
    }

    fn text(&mut self, x: i32, y: i32, text: &str, scale: i32, c: u16) {
        let mut cx = x;
        for ch in text.bytes() {
            self.glyph(cx, y, ch, scale, c);
            cx += 6 * scale;
        }
    }

    fn text_width(text: &str, scale: i32) -> i32 {
        if text.is_empty() { 0 } else { (text.len() as i32 * 6 - 1) * scale }
    }

    fn text_center(&mut self, y: i32, text: &str, scale: i32, c: u16) {
        self.text((WIDTH as i32 - Self::text_width(text, scale)) / 2, y, text, scale, c);
    }

    fn time_text(mins: u16) -> [u8; 5] {
        let h = (mins / 60) % 24;
        let m = mins % 60;
        [b'0' + (h / 10) as u8, b'0' + (h % 10) as u8, b':', b'0' + (m / 10) as u8, b'0' + (m % 10) as u8]
    }

    fn text_center_x(&mut self, cx: i32, y: i32, text: &str, scale: i32, c: u16) {
        self.text(cx - Self::text_width(text, scale) / 2, y, text, scale, c);
    }

    fn status_strip(&mut self, f: MaterialFrame, t: Theme) {
        // Wear OS keeps status chrome deliberately tiny so the watch face remains dominant.
        let mut x = 18;
        if f.wifi { self.circle(x, 16, 4, t.primary); x += 13; }
        if f.bt { self.circle(x, 16, 3, t.primary); x += 12; }
        if f.dnd { self.circle(x, 16, 3, t.on_surface_variant); x += 12; }
        if f.adb {
            self.round_rect(x - 1, 9, 28, 14, 7, t.primary_container);
            self.text(x + 4, 12, "ADB", 1, t.on_primary_container);
        }
        self.round_rect(192, 9, 30, 14, 7, t.surface_high);
        self.text(197, 12, "USB", 1, t.on_surface_variant);
    }

    fn title(&mut self, title: &str, t: Theme) {
        self.text_center(15, title, 2, t.on_surface);
    }

    fn wear_row(&mut self, y: i32, label: &str, selected: bool, t: Theme) {
        // Center item expands like the curved Wear OS launcher/settings lists.
        let (x, w, h, r, bg, fg, icon_r) = if selected {
            (5, 230, 50, 25, t.primary_container, t.on_primary_container, 12)
        } else {
            (17, 206, 42, 21, t.surface, t.on_surface, 9)
        };
        self.round_rect(x, y, w, h, r, bg);
        self.circle(x + 25, y + h / 2, icon_r, if selected { t.primary } else { t.surface_high });
        let scale = if label.len() > 10 { 1 } else { 2 };
        let glyph_h = 7 * scale;
        self.text(x + 46, y + (h - glyph_h) / 2, label, scale, fg);
    }

    fn toggle(&mut self, x: i32, y: i32, on: bool, t: Theme) {
        let bg = if on { t.primary } else { t.surface_high };
        self.round_rect(x, y, 42, 24, 12, bg);
        self.circle(if on { x + 30 } else { x + 12 }, y + 12, 8, if on { t.on_primary } else { t.on_surface_variant });
    }

    fn slider(&mut self, x: i32, y: i32, w: i32, v: u8, t: Theme) {
        self.round_rect(x, y, w, 8, 4, t.surface_high);
        let active = ((w - 8) * v as i32 / 100).max(0);
        self.round_rect(x, y, active + 8, 8, 4, t.primary);
        self.circle(x + 4 + active, y + 4, 7, t.primary);
    }

    fn quick_button(&mut self, cx: i32, cy: i32, label: &str, on: bool, selected: bool, t: Theme) {
        if selected { self.circle(cx, cy, 31, t.primary_container); }
        self.circle(cx, cy, if selected { 25 } else { 24 }, if on { t.primary } else { t.surface_high });
        let icon_color = if on { t.on_primary } else { t.on_surface };
        // Tiny geometric icon: enough to read as a Wear OS quick-settings glyph on 240 px.
        self.circle(cx, cy - 2, 7, icon_color);
        self.text_center_x(cx, cy + 34, label, 1, if selected { t.on_primary_container } else { t.on_surface_variant });
    }

    fn lock_screen(&mut self, f: MaterialFrame, t: Theme) {
        self.status_strip(f, t);
        let tm = Self::time_text(f.time_minutes);
        let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(61, ts, 6, t.on_surface);
        self.text_center(112, "AOSP WEAR OS", 1, t.on_surface_variant);
        // Small lock puck, matching the unobtrusive locked-state cue used on watches.
        self.circle(120, 145, 14, t.surface_high);
        self.round_rect(115, 140, 10, 10, 3, t.on_surface_variant);
        if f.notes > 0 {
            self.round_rect(31, 176, 178, 47, 23, t.surface);
            self.circle(55, 199, 10, t.primary_container);
            self.text(76, 188, "ANDROID SYSTEM", 1, t.on_surface);
            self.text(76, 204, "1 NOTIFICATION", 1, t.on_surface_variant);
        } else {
            self.text_center(192, "NO NOTIFICATIONS", 1, t.outline);
        }
        self.text_center(251, "PRESS CROWN", 1, t.outline);
    }

    fn watchface(&mut self, f: MaterialFrame, t: Theme) {
        self.status_strip(f, t);
        let tm = Self::time_text(f.time_minutes);
        let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(54, ts, 6, t.on_surface);
        self.text_center(105, "ANDROID 17 QPR1", 1, t.on_surface_variant);

        // Complication row: rounded, glanceable, and almost entirely icon-driven.
        self.round_rect(19, 145, 62, 54, 27, t.surface);
        self.circle(50, 163, 8, if f.wifi { t.primary } else { t.surface_high });
        self.text_center_x(50, 181, if f.wifi { "WIFI" } else { "OFF" }, 1, if f.wifi { t.primary } else { t.on_surface_variant });

        self.round_rect(89, 145, 62, 54, 27, t.surface);
        self.circle(120, 163, 8, if f.bt { t.primary } else { t.surface_high });
        self.text_center_x(120, 181, if f.bt { "BT" } else { "OFF" }, 1, if f.bt { t.primary } else { t.on_surface_variant });

        self.round_rect(159, 145, 62, 54, 27, t.surface);
        self.circle(190, 163, 8, if f.adb { t.primary } else { t.surface_high });
        self.text_center_x(190, 181, if f.adb { "ADB" } else { "OFF" }, 1, if f.adb { t.primary } else { t.on_surface_variant });

        if f.notes > 0 {
            self.circle(120, 224, 6, t.primary);
            self.circle(120, 224, 2, t.on_primary);
        }
        self.text_center(252, if f.dnd { "DND  CROWN FOR APPS" } else { "CROWN FOR APPS" }, 1, t.outline);
    }

    fn launcher(&mut self, f: MaterialFrame, t: Theme) {
        self.text_center(12, "APPS", 1, t.on_surface_variant);
        let labels = ["MESSAGES", "SETTINGS", "CLOCK", "CONNECT", "SYSTEM"];
        let cur = f.cursor.min(4) as usize;
        let start = if cur >= 2 { (cur - 2).min(2) } else { 0 };
        for row in 0..3 {
            let i = (start + row).min(4);
            let y = 49 + row as i32 * 67;
            self.wear_row(y, labels[i], i == cur, t);
        }
        self.text_center(258, "ROTATE  PRESS", 1, t.outline);
    }

    fn notifications(&mut self, f: MaterialFrame, t: Theme) {
        self.title("NOTIFICATIONS", t);
        if f.notes == 0 {
            self.circle(120, 119, 28, t.surface);
            self.circle(120, 119, 8, t.primary_container);
            self.text_center(162, "ALL CAUGHT UP", 1, t.on_surface_variant);
        } else {
            self.round_rect(12, 55, 216, 104, 34, t.surface);
            self.circle(43, 84, 13, t.primary_container);
            self.text(66, 72, "ANDROID SYSTEM", 1, t.on_surface_variant);
            self.text(66, 91, "DEVICE IS READY", 2, t.on_surface);
            self.text(66, 119, "ANDROID 17 QPR1", 1, t.primary);
            self.round_rect(20, 172, 200, 58, 29, t.surface);
            self.circle(47, 201, 10, if f.adb { t.primary } else { t.surface_high });
            self.text(69, 189, "WIRELESS DEBUG", 1, t.on_surface);
            self.text(69, 205, if f.adb { "ADB AVAILABLE" } else { "ADB OFF" }, 1, t.on_surface_variant);
        }
        self.text_center(258, "CROWN  BACK", 1, t.outline);
    }

    fn quick_settings(&mut self, f: MaterialFrame, t: Theme) {
        let tm = Self::time_text(f.time_minutes);
        let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(8, ts, 2, t.on_surface);
        self.quick_button(58, 72, "WIFI", f.wifi, f.cursor == 0, t);
        self.quick_button(182, 72, "BT", f.bt, f.cursor == 1, t);
        self.quick_button(58, 157, "ADB", f.adb, f.cursor == 2, t);
        self.quick_button(182, 157, "DND", f.dnd, f.cursor == 3, t);
        self.quick_button(58, 225, "AIR", f.airplane, f.cursor == 4, t);
        self.quick_button(182, 225, "LIGHT", true, f.cursor == 5, t);
        // Brightness value is encoded as a tiny progress line under the LIGHT tile.
        let active = (42 * f.brightness as i32 / 100).max(3);
        self.round_rect(161, 269, 42, 4, 2, t.surface_high);
        self.round_rect(161, 269, active, 4, 2, t.primary);
    }

    fn settings(&mut self, f: MaterialFrame, t: Theme) {
        self.title("SETTINGS", t);
        let labels = ["DISPLAY", "THEME", "CONNECTIVITY", "SYSTEM", "BACK"];
        let cur = f.cursor.min(4) as usize;
        let start = if cur >= 3 { cur - 2 } else { 0 };
        for row in 0..3 {
            let i = (start + row).min(4);
            let y = 57 + row as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i == 0 { self.slider(145, y + 22, 60, f.brightness, t); }
            if i == 1 {
                let n = match f.theme { 1 => "BLUE", 2 => "PURPLE", 3 => "CORAL", _ => "GREEN" };
                self.text(159, y + 20, n, 1, t.on_surface_variant);
            }
        }
        self.text_center(258, "ROTATE  PRESS", 1, t.outline);
    }

    fn clock(&mut self, f: MaterialFrame, t: Theme) {
        let tm = Self::time_text(f.time_minutes);
        let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(57, ts, 6, t.on_surface);
        self.text_center(111, "CLOCK", 1, t.on_surface_variant);
        self.round_rect(36, 151, 168, 55, 27, t.surface);
        self.circle(61, 178, 9, t.primary_container);
        self.text(82, 166, "TIME SOURCE", 1, t.on_surface_variant);
        self.text(82, 184, "RTC / NTP READY", 1, t.primary);
        self.text_center(258, "CROWN  BACK", 1, t.outline);
    }

    fn connectivity(&mut self, f: MaterialFrame, t: Theme) {
        self.title("CONNECTIVITY", t);
        let labels = ["WIFI", "BLUETOOTH", "WIRELESS ADB", "AIRPLANE", "BACK"];
        let vals = [f.wifi, f.bt, f.adb, f.airplane, false];
        let cur = f.cursor.min(4) as usize;
        let start = if cur >= 3 { cur - 2 } else { 0 };
        for row in 0..3 {
            let i = (start + row).min(4);
            let y = 57 + row as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i < 4 { self.toggle(174, y + 13, vals[i], t); }
        }
        self.text_center(258, "ROTATE  PRESS", 1, t.outline);
    }

    fn about(&mut self, f: MaterialFrame, t: Theme) {
        self.title("SYSTEM", t);
        self.circle(120, 75, 30, t.primary_container);
        self.text_center(61, "A", 4, t.primary);
        self.text_center(116, "AOSP WEAR OS", 2, t.on_surface);
        self.text_center(143, "ANDROID 17 QPR1", 1, t.primary);
        self.round_rect(25, 166, 190, 34, 17, t.surface);
        self.text_center(177, "ESP32-S3 / API 37", 1, t.on_surface);
        self.text_center(214, "PSRAM 2M / SWAP 32M", 1, t.on_surface_variant);
        self.text_center(234, if f.swap_pages > 0 { "PAGER ACTIVE" } else { "PAGER READY" }, 1, if f.swap_pages > 0 { t.primary } else { t.outline });
        self.text_center(255, "USERDEBUG / AVB ORANGE", 1, t.error);
    }

}
