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
    pub timer_secs: u16,
    pub stopwatch_secs: u16,
    pub alarm_enabled: bool,
    pub auto_time: bool,
    pub time_valid: bool,
    pub timezone_hours: i8,
    pub swap_pages: u8,
    pub wifi_ap_count: u8,
    pub wifi_ap_index: u8,
    pub wifi_ap_ssid: [u8; 32],
    pub wifi_ap_ssid_len: u8,
    pub wifi_ap_rssi: i8,
    pub wifi_password_len: u8,
    // On-device-only clear-text preview. Desktop/web serializers deliberately
    // omit these bytes; they exist only so the physical keyboard can show
    // exactly what was entered with the crown.
    pub wifi_password_preview: [u8; 16],
    pub wifi_password_preview_len: u8,
    pub wifi_editor_char: u8,
    pub wifi_link_state: u8,
}

impl MaterialFrame {
    pub fn parse(raw: &str) -> Option<Self> {
        let mut p = raw.split('|');
        if p.next()? != "M3" { return None; }
        let screen = p.next()?.parse::<u8>().ok()?.min(31);
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
        let timer_secs = p.next()?.parse::<u16>().ok()?.min(300);
        let stopwatch_secs = p.next()?.parse::<u16>().ok()?.min(3599);
        let alarm_enabled = p.next()? == "1";
        let auto_time = p.next()? == "1";
        let time_valid = p.next()? == "1";
        let timezone_hours = p.next()?.parse::<i8>().ok()?.clamp(-12, 14);
        if p.next().is_some() { return None; }
        Some(Self {
            screen, cursor, brightness, dnd, airplane, theme, locked,
            wifi, bt, adb, notes, time_minutes, timer_secs, stopwatch_secs,
            alarm_enabled, auto_time, time_valid, timezone_hours, swap_pages: 0,
            wifi_ap_count: 0, wifi_ap_index: 0, wifi_ap_ssid: [0; 32], wifi_ap_ssid_len: 0,
            wifi_ap_rssi: -127, wifi_password_len: 0, wifi_password_preview: [0; 16], wifi_password_preview_len: 0, wifi_editor_char: 0, wifi_link_state: 0,
        })
    }

    pub fn set_swap_pages(&mut self, pages: u8) { self.swap_pages = pages; }

    pub fn fallback(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self.screen {
            0 => ("LOCK", "WEAR OS", "CROWN TO UNLOCK", ""),
            1 => ("WATCH FACE", "", "CROWN: APPS", ""),
            2 => ("APPS", "CLOCK", "MEDIA CONTROLS", "SETTINGS"),
            3 => ("NOTIFICATIONS", "", "", ""),
            4 => ("QUICK SETTINGS", "WIFI / BT", "DND / AIRPLANE", "BRIGHTNESS"),
            5 => ("SETTINGS", "CONNECTIVITY", "DISPLAY", "SYSTEM"),
            6 => ("CLOCK", "ALARM", "TIMER", "STOPWATCH"),
            7 => ("CONNECTIVITY", "WIFI", "BLUETOOTH", "WIRELESS DEBUG"),
            8 => ("ABOUT", "AOSP WEAR", "ANDROID 17", "ESP32-S3"),
            9 => ("TILES", "", "", ""),
            10 => ("MEDIA CONTROLS", "NO ACTIVE SESSION", "", ""),
            11 => ("CLOCK", "TIMER", "STOPWATCH", "ALARM"),
            12 => ("WIFI", "NETWORK", "SAVED NETWORK", ""),
            13 => ("BLUETOOTH", "PAIR NEW DEVICE", "", ""),
            14 => ("DISPLAY", "BRIGHTNESS", "THEME", ""),
            15 => ("SYSTEM", "DATE & TIME", "ABOUT", "DEVELOPER OPTIONS"),
            16 => ("DATE & TIME", "AUTOMATIC TIME", "SET TIME", "TIME ZONE"),
            17 => ("SOUND & VIBRATION", "NO AUDIO HARDWARE", "", ""),
            18 => ("GESTURES", "CROWN NAVIGATION", "", ""),
            19 => ("ACCESSIBILITY", "CROWN-ONLY MODE", "", ""),
            20 => ("SECURITY", "SCREEN LOCK", "", ""),
            21 => ("APPS & NOTIFICATIONS", "NOTIFICATIONS", "APP INFO", ""),
            22 => ("DEVELOPER OPTIONS", "WIRELESS DEBUGGING", "BUILD", "SYSTEM"),
            23 => ("AVAILABLE NETWORKS", "SELECT NETWORK", "", ""),
            24 => ("WI-FI PASSWORD", "CROWN TO TYPE", "", ""),
            25 => ("WI-FI", "CONNECTING", "", ""),
            26 => ("APPS", "LIST VIEW", "", ""),
            27 => ("ASSISTANT", "READY", "", ""),
            _ => ("WEAR OS", "ANDROID 17", "", ""),
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

fn theme(index: u8) -> Theme {
    match index % 4 {
        1 => Theme {
            bg: rgb565(0, 0, 0), surface: rgb565(16, 21, 31), surface_high: rgb565(27, 34, 47),
            primary: rgb565(174, 203, 255), on_primary: rgb565(8, 48, 88),
            primary_container: rgb565(32, 69, 111), on_primary_container: rgb565(219, 230, 255),
            on_surface: rgb565(241, 242, 248), on_surface_variant: rgb565(191, 198, 211),
            outline: rgb565(118, 127, 143), error: rgb565(255, 180, 171),
        },
        2 => Theme {
            bg: rgb565(0, 0, 0), surface: rgb565(24, 18, 28), surface_high: rgb565(39, 29, 45),
            primary: rgb565(224, 187, 255), on_primary: rgb565(72, 31, 97),
            primary_container: rgb565(100, 55, 128), on_primary_container: rgb565(244, 220, 255),
            on_surface: rgb565(246, 239, 247), on_surface_variant: rgb565(207, 194, 211),
            outline: rgb565(142, 128, 147), error: rgb565(255, 180, 171),
        },
        3 => Theme {
            bg: rgb565(0, 0, 0), surface: rgb565(29, 19, 17), surface_high: rgb565(47, 31, 27),
            primary: rgb565(255, 184, 161), on_primary: rgb565(86, 33, 16),
            primary_container: rgb565(119, 55, 34), on_primary_container: rgb565(255, 221, 211),
            on_surface: rgb565(249, 238, 233), on_surface_variant: rgb565(216, 195, 188),
            outline: rgb565(150, 129, 123), error: rgb565(255, 180, 171),
        },
        _ => Theme {
            bg: rgb565(0, 0, 0), surface: rgb565(14, 24, 18), surface_high: rgb565(24, 39, 29),
            primary: rgb565(126, 225, 165), on_primary: rgb565(0, 57, 29),
            primary_container: rgb565(23, 80, 49), on_primary_container: rgb565(163, 249, 194),
            on_surface: rgb565(238, 244, 239), on_surface_variant: rgb565(190, 203, 194),
            outline: rgb565(126, 143, 132), error: rgb565(255, 180, 171),
        },
    }
}

pub struct Surface { pixels: Vec<u16> }

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
            8 => self.about(frame, t),
            9 => self.widgets(frame, t),
            10 => self.media(frame, t),
            11 => self.clock_tools(frame, t),
            12 => self.wifi_page(frame, t),
            13 => self.bluetooth_page(frame, t),
            14 => self.display_settings(frame, t),
            15 => self.system_page(frame, t),
            16 => self.date_time(frame, t),
            17 => self.sound_page(frame, t),
            18 => self.gestures_page(frame, t),
            19 => self.accessibility_page(frame, t),
            20 => self.security_page(frame, t),
            21 => self.apps_notifications_page(frame, t),
            22 => self.developer_page(frame, t),
            23 => self.wifi_networks(frame, t),
            24 => self.wifi_password(frame, t),
            25 => self.wifi_connecting(frame, t),
            26 => self.list_launcher(frame, t),
            27 => self.assistant_page(frame, t),
            _ => self.watchface(frame, t),
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
        if x1 <= x0 || y1 <= y0 { return; }
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

    fn ring(&mut self, cx: i32, cy: i32, r: i32, thickness: i32, c: u16) {
        let inner = (r - thickness).max(0);
        for y in -r..=r {
            for x in -r..=r {
                let d = x * x + y * y;
                if d <= r * r && d >= inner * inner { self.pixel(cx + x, cy + y, c); }
            }
        }
    }

    fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, thickness: i32, c: u16) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.circle(x0, y0, thickness.max(1) / 2, c);
            if x0 == x1 && y0 == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x0 += sx; }
            if e2 <= dx { err += dx; y0 += sy; }
        }
    }

    fn icon_wifi(&mut self, cx: i32, cy: i32, c: u16) {
        // Compact Wear-style Wi-Fi glyph built from three chevrons/dot.
        self.line(cx - 12, cy - 6, cx, cy - 12, 2, c);
        self.line(cx, cy - 12, cx + 12, cy - 6, 2, c);
        self.line(cx - 8, cy, cx, cy - 4, 2, c);
        self.line(cx, cy - 4, cx + 8, cy, 2, c);
        self.line(cx - 4, cy + 6, cx, cy + 4, 2, c);
        self.line(cx, cy + 4, cx + 4, cy + 6, 2, c);
        self.circle(cx, cy + 10, 2, c);
    }

    fn icon_bt(&mut self, cx: i32, cy: i32, c: u16) {
        self.line(cx, cy - 13, cx, cy + 13, 2, c);
        self.line(cx, cy - 13, cx + 8, cy - 5, 2, c);
        self.line(cx + 8, cy - 5, cx - 7, cy + 7, 2, c);
        self.line(cx - 7, cy - 7, cx + 8, cy + 5, 2, c);
        self.line(cx + 8, cy + 5, cx, cy + 13, 2, c);
    }

    fn icon_dnd(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 12, 3, c);
        self.round_rect(cx - 7, cy - 2, 14, 4, 2, c);
    }

    fn icon_airplane(&mut self, cx: i32, cy: i32, c: u16) {
        self.line(cx - 13, cy, cx + 13, cy, 3, c);
        self.line(cx + 3, cy, cx - 3, cy - 9, 3, c);
        self.line(cx + 3, cy, cx - 3, cy + 9, 3, c);
        self.line(cx - 7, cy, cx - 12, cy - 5, 2, c);
        self.line(cx - 7, cy, cx - 12, cy + 5, 2, c);
    }

    fn icon_sun(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 7, 2, c);
        for &(x0,y0,x1,y1) in &[
            (0,-14,0,-10),(0,10,0,14),(-14,0,-10,0),(10,0,14,0),
            (-10,-10,-7,-7),(7,7,10,10),(-10,10,-7,7),(7,-7,10,-10),
        ] { self.line(cx+x0,cy+y0,cx+x1,cy+y1,2,c); }
    }

    fn icon_settings(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 9, 3, c);
        self.circle(cx, cy, 3, c);
        self.line(cx, cy-15, cx, cy-10, 3, c);
        self.line(cx, cy+10, cx, cy+15, 3, c);
        self.line(cx-15, cy, cx-10, cy, 3, c);
        self.line(cx+10, cy, cx+15, cy, 3, c);
    }

    fn icon_bell(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 9, 3, c);
        self.fill_rect(cx - 10, cy + 1, 20, 8, c);
        self.circle(cx, cy + 12, 2, c);
    }

    fn icon_clock(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 12, 2, c);
        self.line(cx, cy, cx, cy - 7, 2, c);
        self.line(cx, cy, cx + 6, cy + 3, 2, c);
    }

    fn icon_media(&mut self, cx: i32, cy: i32, c: u16) {
        // Material-style play glyph with a compact media baseline.
        for i in 0..13 {
            let half = if i < 7 { i } else { 12 - i };
            self.line(cx - 6 + i, cy - half, cx - 6 + i, cy + half, 2, c);
        }
        self.line(cx - 11, cy + 13, cx + 11, cy + 13, 2, c);
    }

    fn icon_info(&mut self, cx: i32, cy: i32, c: u16) {
        self.ring(cx, cy, 12, 2, c);
        self.circle(cx, cy - 6, 2, c);
        self.round_rect(cx - 2, cy - 1, 4, 10, 2, c);
    }

    fn icon_grid(&mut self, cx: i32, cy: i32, c: u16) {
        for yy in [-7, 0, 7] { for xx in [-7, 0, 7] { self.circle(cx + xx, cy + yy, 2, c); } }
    }

    fn icon_assistant(&mut self, cx: i32, cy: i32, c: u16) {
        // Four-point sparkle: recognizable assistant affordance without
        // pretending to be a proprietary service logo.
        self.line(cx, cy - 13, cx, cy + 13, 2, c);
        self.line(cx - 13, cy, cx + 13, cy, 2, c);
        self.line(cx - 8, cy - 8, cx + 8, cy + 8, 2, c);
        self.line(cx - 8, cy + 8, cx + 8, cy - 8, 2, c);
        self.circle(cx, cy, 3, c);
    }

    fn icon_connectivity(&mut self, cx: i32, cy: i32, c: u16) {
        self.icon_wifi(cx - 3, cy - 2, c);
        self.circle(cx + 10, cy + 10, 3, c);
    }

    fn app_icon(&mut self, icon: u8, cx: i32, cy: i32, c: u16) {
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

    fn app_accent(icon: u8) -> u16 {
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

    fn icon_back(&mut self, cx: i32, cy: i32, c: u16) {
        self.line(cx + 8, cy, cx - 8, cy, 3, c);
        self.line(cx - 8, cy, cx - 1, cy - 7, 3, c);
        self.line(cx - 8, cy, cx - 1, cy + 7, 3, c);
    }

    fn icon_lock(&mut self, cx: i32, cy: i32, c: u16) {
        self.round_rect(cx - 9, cy - 1, 18, 15, 4, c);
        // Hollow the body center slightly using the surrounding black surface
        // is not possible generically here; keep the compact filled Wear glyph.
        self.ring(cx, cy - 5, 7, 2, c);
    }

    fn icon_sound(&mut self, cx: i32, cy: i32, c: u16) {
        self.fill_rect(cx - 11, cy - 4, 6, 8, c);
        self.line(cx - 5, cy - 4, cx + 2, cy - 10, 3, c);
        self.line(cx - 5, cy + 4, cx + 2, cy + 10, 3, c);
        self.line(cx + 6, cy - 7, cx + 10, cy, 2, c);
        self.line(cx + 10, cy, cx + 6, cy + 7, 2, c);
    }

    fn icon_person(&mut self, cx: i32, cy: i32, c: u16) {
        self.circle(cx, cy - 8, 4, c);
        self.line(cx, cy - 2, cx, cy + 10, 3, c);
        self.line(cx - 9, cy + 2, cx + 9, cy + 2, 3, c);
        self.line(cx, cy + 10, cx - 7, cy + 15, 3, c);
        self.line(cx, cy + 10, cx + 7, cy + 15, 3, c);
    }

    fn row_icon(&mut self, label: &str, cx: i32, cy: i32, c: u16) {
        match label {
            "CONNECTIVITY" | "WI-FI" => self.icon_wifi(cx, cy, c),
            "BLUETOOTH" => self.icon_bt(cx, cy, c),
            "APPS & NOTIFS" | "NOTIFICATIONS" => self.icon_bell(cx, cy, c),
            "DISPLAY" | "BRIGHTNESS" => self.icon_sun(cx, cy, c),
            "SOUND & VIBRATION" => self.icon_sound(cx, cy, c),
            "ACCESSIBILITY" => self.icon_person(cx, cy, c),
            "SECURITY" | "SCREEN LOCK" => self.icon_lock(cx, cy, c),
            "DATE & TIME" | "AUTOMATIC TIME" | "SET HOUR" | "SET MINUTE" | "TIME ZONE" => self.icon_clock(cx, cy, c),
            "ABOUT" | "SYSTEM INFO" | "BUILD NUMBER" => self.icon_info(cx, cy, c),
            "DEVELOPER OPTIONS" | "SYSTEM" | "SETTINGS" => self.icon_settings(cx, cy, c),
            "MEDIA CONTROLS" => self.icon_media(cx, cy, c),
            "AIRPLANE MODE" => self.icon_airplane(cx, cy, c),
            "BACK" => self.icon_back(cx, cy, c),
            _ => self.circle(cx, cy, 3, c),
        }
    }


    fn numeral_segment(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: u16) {
        self.line(x0, y0, x1, y1, 5, c);
    }

    fn numeral(&mut self, x: i32, y: i32, ch: u8, c: u16) {
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
        if seg & 0b0000001 != 0 { self.numeral_segment(x + 6, y, x + 28, y, c); }
        if seg & 0b0000010 != 0 { self.numeral_segment(x + 31, y + 5, x + 31, y + 25, c); }
        if seg & 0b0000100 != 0 { self.numeral_segment(x + 31, y + 33, x + 31, y + 53, c); }
        if seg & 0b0001000 != 0 { self.numeral_segment(x + 6, y + 58, x + 28, y + 58, c); }
        if seg & 0b0010000 != 0 { self.numeral_segment(x + 3, y + 33, x + 3, y + 53, c); }
        if seg & 0b0100000 != 0 { self.numeral_segment(x + 3, y + 5, x + 3, y + 25, c); }
        if seg & 0b1000000 != 0 { self.numeral_segment(x + 6, y + 29, x + 28, y + 29, c); }
    }

    fn big_time(&mut self, y: i32, minutes: u16, _valid: bool, c: u16) {
        let h = (minutes / 60) % 24;
        let m = minutes % 60;
        let chars = [b'0' + (h / 10) as u8, b'0' + (h % 10) as u8, b'0' + (m / 10) as u8, b'0' + (m % 10) as u8];
        self.numeral(32, y, chars[0], c);
        self.numeral(72, y, chars[1], c);
        self.circle(120, y + 20, 3, c); self.circle(120, y + 40, 3, c);
        self.numeral(136, y, chars[2], c);
        self.numeral(176, y, chars[3], c);
    }

    fn glyph(&mut self, x: i32, y: i32, ch: u8, scale: i32, c: u16) {
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

    fn text(&mut self, x: i32, y: i32, text: &str, scale: i32, c: u16) {
        let mut cx = x;
        for ch in text.bytes() { self.glyph(cx, y, ch, scale, c); cx += 6 * scale; }
    }

    fn text_width(text: &str, scale: i32) -> i32 {
        if text.is_empty() { 0 } else { (text.len() as i32 * 6 - 1) * scale }
    }

    fn text_center(&mut self, y: i32, text: &str, scale: i32, c: u16) {
        self.text((WIDTH as i32 - Self::text_width(text, scale)) / 2, y, text, scale, c);
    }

    fn text_center_x(&mut self, cx: i32, y: i32, text: &str, scale: i32, c: u16) {
        self.text(cx - Self::text_width(text, scale) / 2, y, text, scale, c);
    }

    fn time_text(mins: u16) -> [u8; 5] {
        let h = (mins / 60) % 24;
        let m = mins % 60;
        [b'0' + (h / 10) as u8, b'0' + (h % 10) as u8, b':', b'0' + (m / 10) as u8, b'0' + (m % 10) as u8]
    }

    fn mmss_text(secs: u16) -> [u8; 5] {
        let m = (secs / 60).min(99);
        let s = secs % 60;
        [b'0' + (m / 10) as u8, b'0' + (m % 10) as u8, b':', b'0' + (s / 10) as u8, b'0' + (s % 10) as u8]
    }

    fn status_strip(&mut self, f: MaterialFrame, t: Theme) {
        // System-facing status only. Development transports (USB/ADB) are not
        // exposed on normal Wear surfaces.
        let mut x = 18;
        if f.airplane { self.icon_airplane(x, 15, t.on_surface_variant); x += 29; }
        if f.wifi { self.icon_wifi(x, 15, t.primary); x += 31; }
        if f.bt { self.icon_bt(x, 15, t.primary); x += 25; }
        if f.dnd { self.icon_dnd(x, 15, t.on_surface_variant); }
        if f.notes > 0 { self.circle(218, 15, 4, t.primary); }
    }

    fn title(&mut self, title: &str, t: Theme) { self.text_center(15, title, 2, t.on_surface); }

    fn keyboard_key(&mut self, x: i32, y: i32, w: i32, h: i32, label: &str, selected: bool, t: Theme) {
        let bg = if selected { t.primary } else { t.surface_high };
        let fg = if selected { t.on_primary } else { t.on_surface };
        self.round_rect(x, y, w, h, 7, bg);
        self.text_center_x(x + w / 2, y + (h - 7) / 2, label, 1, fg);
    }

    fn wear_row(&mut self, y: i32, label: &str, selected: bool, t: Theme) {
        let (x, w, h, r, bg, fg, icon_r) = if selected {
            (5, 230, 50, 25, t.primary_container, t.on_primary_container, 12)
        } else {
            (17, 206, 42, 21, t.surface, t.on_surface, 9)
        };
        self.round_rect(x, y, w, h, r, bg);
        let icon_bg = if selected { t.primary } else { t.surface_high };
        let icon_fg = if selected { t.on_primary } else { t.on_surface_variant };
        self.circle(x + 25, y + h / 2, icon_r, icon_bg);
        self.row_icon(label, x + 25, y + h / 2, icon_fg);
        let scale = if label.len() > 11 { 1 } else { 2 };
        self.text(x + 46, y + (h - 7 * scale) / 2, label, scale, fg);
    }

    fn app_row(&mut self, y: i32, label: &str, selected: bool, icon: u8, t: Theme) {
        let (x, w, h, r, cx, icon_r, bg, fg) = if selected {
            (8, 224, 48, 24, 35, 15, t.primary_container, t.on_primary_container)
        } else {
            (17, 206, 42, 21, 42, 12, t.surface, t.on_surface)
        };
        self.round_rect(x, y, w, h, r, bg);
        self.circle(cx, y + h / 2, icon_r, Self::app_accent(icon));
        let icon_fg = rgb565(10, 34, 24);
        self.app_icon(icon, cx, y + h / 2, icon_fg);
        let scale = if label.len() > 12 { 1 } else { 2 };
        self.text(if selected { 64 } else { 70 }, y + (h - 7 * scale) / 2, label, scale, fg);
    }

    fn toggle(&mut self, x: i32, y: i32, on: bool, t: Theme) {
        let bg = if on { t.primary } else { t.surface_high };
        self.round_rect(x, y, 42, 24, 12, bg);
        self.circle(if on { x + 30 } else { x + 12 }, y + 12, 8, if on { t.on_primary } else { t.on_surface_variant });
    }

    fn slider(&mut self, x: i32, y: i32, w: i32, v: u8, t: Theme) {
        self.round_rect(x, y, w, 10, 5, t.surface_high);
        let active = ((w - 10) * v as i32 / 100).max(0);
        self.round_rect(x, y, active + 10, 10, 5, t.primary);
        self.circle(x + 5 + active, y + 5, 8, t.primary);
    }

    fn quick_button(&mut self, cx: i32, cy: i32, label: &str, on: bool, selected: bool, t: Theme) {
        // Wear OS 6 / Material 3 Expressive: grouped pill buttons expand and
        // shape-morph around the focused item.
        let w = if selected { 92 } else { 82 };
        let h = if selected { 54 } else { 48 };
        let bg = if on { t.primary } else if selected { t.primary_container } else { t.surface_high };
        let fg = if on { t.on_primary } else if selected { t.on_primary_container } else { t.on_surface };
        self.round_rect(cx - w / 2, cy - h / 2, w, h, h / 2, bg);
        let ix = cx - 22;
        match label {
            "WIFI" => self.icon_wifi(ix, cy, fg),
            "BT" => self.icon_bt(ix, cy, fg),
            "DND" => self.icon_dnd(ix, cy, fg),
            "AIR" => self.icon_airplane(ix, cy, fg),
            "LIGHT" => self.icon_sun(ix, cy, fg),
            "SET" => self.icon_settings(ix, cy, fg),
            _ => self.circle(ix, cy, 7, fg),
        }
        self.text(cx - 3, cy - 4, label, 1, fg);
    }

    fn info_card(&mut self, y: i32, title: &str, value: &str, active: bool, t: Theme) {
        self.round_rect(16, y, 208, 52, 26, if active { t.primary_container } else { t.surface });
        self.circle(42, y + 26, 10, if active { t.primary } else { t.surface_high });
        self.text(64, y + 10, title, 1, if active { t.on_primary_container } else { t.on_surface_variant });
        self.text(64, y + 28, value, 1, if active { t.primary } else { t.on_surface });
    }


    fn lock_screen(&mut self, f: MaterialFrame, t: Theme) {
        self.big_time(55, f.time_minutes, f.time_valid, t.on_surface);
        self.ring(120, 145, 17, 3, t.surface_high);
        self.round_rect(114, 142, 12, 12, 3, t.on_surface_variant);
        if f.notes > 0 { self.circle(120, 196, 5, t.primary); }
        self.text_center(226, "UNLOCK", 1, t.on_surface_variant);
    }


    fn watchface(&mut self, f: MaterialFrame, t: Theme) {
        // Watch face intentionally contains no Android/debug branding. Wear
        // surfaces prioritize glanceable time, status, and complications.
        self.status_strip(f, t);
        self.big_time(43, f.time_minutes, f.time_valid, t.on_surface);
        if !f.time_valid {
            self.text_center(112, if f.auto_time { "SYNCING TIME" } else { "SET TIME IN SETTINGS" }, 1, t.on_surface_variant);
        }

        // Three small complication slots. They expose only real state that the
        // board actually knows, not development transport state.
        let y = 177;
        for cx in [54, 120, 186] { self.circle(cx, y, 24, t.surface); }
        if f.wifi { self.icon_wifi(54, y, t.primary); } else { self.icon_wifi(54, y, t.outline); }
        if f.alarm_enabled { self.icon_clock(120, y, t.primary); } else { self.icon_clock(120, y, t.outline); }
        if f.dnd { self.icon_dnd(186, y, t.primary); } else { self.icon_dnd(186, y, t.outline); }

        if f.notes > 0 {
            self.circle(120, 231, 6, t.primary);
            self.circle(120, 231, 2, t.on_primary);
        }
    }


    fn launcher(&mut self, f: MaterialFrame, t: Theme) {
        // Honeycomb/grid launcher adapted from modern Wear OS.  The selected
        // bubble grows slightly so the encoder always has an obvious focus.
        let labels = ["CLOCK", "MEDIA", "SETTINGS", "CONNECTIVITY", "SYSTEM INFO", "ASSISTANT", "LIST VIEW"];
        let cur = f.cursor.min(6) as usize;
        let pos = [
            (52, 70), (120, 58), (188, 70),
            (52, 140), (120, 128), (188, 140),
            (120, 208),
        ];
        for i in 0..7 {
            let (cx, cy) = pos[i];
            let selected = i == cur;
            let r = if selected { 33 } else { if i == 6 { 26 } else { 28 } };
            let bg = if i == 6 { t.surface_high } else { Self::app_accent(i as u8) };
            let fg = if i == 6 { t.on_surface_variant } else { rgb565(10, 34, 24) };
            if selected { self.ring(cx, cy, r + 3, 2, t.on_surface); }
            self.circle(cx, cy, r, bg);
            self.app_icon(i as u8, cx, cy, fg);
        }
        // Only the focused app gets a label, keeping the grid glanceable like
        // the round-screen launcher while still being usable with one encoder.
        self.round_rect(31, 244, 178, 27, 13, t.surface);
        self.text_center(254, labels[cur], 1, t.on_surface);
    }

    fn list_launcher(&mut self, f: MaterialFrame, t: Theme) {
        let labels = ["CLOCK", "MEDIA", "SETTINGS", "CONNECTIVITY", "SYSTEM INFO", "ASSISTANT", "GRID VIEW"];
        let cur = f.cursor.min(6) as usize;
        let start = cur.saturating_sub(1).min(4);
        for row in 0..3 {
            let i = (start + row).min(6);
            let y = 44 + row as i32 * 66;
            let selected = i == cur;
            if i == 6 {
                let (x,w,h) = if selected { (22,196,54) } else { (34,172,46) };
                self.round_rect(x, y, w, h, h/2, if selected { t.primary } else { t.surface_high });
                self.icon_grid(x + 31, y + h/2, if selected { t.on_primary } else { t.on_surface_variant });
                self.text(x + 58, y + (h-14)/2, "GRID VIEW", 2, if selected { t.on_primary } else { t.on_surface });
            } else {
                self.app_row(y, labels[i], selected, i as u8, t);
            }
        }
        self.circle(228, 72 + (cur.saturating_sub(start) as i32 * 66), 3, t.primary);
    }

    fn assistant_page(&mut self, _f: MaterialFrame, t: Theme) {
        // Assistant shell is ready for the future phone companion.  The UI is
        // intentionally service-neutral: it mirrors the Wear assistant surface
        // without claiming a Google/Gemini backend that does not exist here.
        self.ring(120, 116, 70, 2, t.surface_high);
        self.icon_assistant(120, 91, t.primary);
        self.text_center(126, "ASK ASSISTANT", 2, t.on_surface);
        self.text_center(158, "COMPANION APP REQUIRED", 1, t.on_surface_variant);
        self.line(79, 183, 101, 183, 3, rgb565(80, 190, 255));
        self.line(101, 183, 123, 183, 3, rgb565(126, 225, 165));
        self.line(123, 183, 145, 183, 3, rgb565(224, 187, 255));
        self.line(145, 183, 167, 183, 3, rgb565(255, 184, 161));
        self.round_rect(51, 224, 138, 38, 19, t.surface_high);
        self.text_center(236, "PRESS TO RETURN", 1, t.on_surface_variant);
    }


    fn notifications(&mut self, f: MaterialFrame, t: Theme) {
        let tm = Self::time_text(f.time_minutes);
        let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(10, ts, 1, t.on_surface_variant);
        if f.notes == 0 {
            self.circle(120, 113, 32, t.surface);
            self.icon_bell(120, 113, t.on_surface_variant);
            self.text_center(161, "NO NOTIFICATIONS", 1, t.on_surface_variant);
        } else {
            let selected = f.cursor == 0;
            self.round_rect(12, 50, 216, 112, 35, if selected { t.primary_container } else { t.surface });
            self.circle(44, 82, 14, t.primary);
            self.icon_bell(44, 82, t.on_primary);
            self.text(67, 67, "SYSTEM", 1, t.on_surface_variant);
            self.text(67, 88, "NOTIFICATION", 2, if selected { t.on_primary_container } else { t.on_surface });
            self.text(67, 119, "PRESS TO DISMISS", 1, t.on_surface_variant);
            if f.notes > 1 {
                self.round_rect(27, 175, 186, 48, 24, t.surface);
                self.text_center(191, "MORE NOTIFICATIONS", 1, t.on_surface_variant);
            }
        }
    }


    fn quick_settings(&mut self, f: MaterialFrame, t: Theme) {
        let tm = Self::time_text(f.time_minutes);
        let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(9, ts, 2, t.on_surface);
        self.quick_button(66, 68, "WIFI", f.wifi, f.cursor == 0, t);
        self.quick_button(174, 68, "BT", f.bt, f.cursor == 1, t);
        self.quick_button(66, 132, "DND", f.dnd, f.cursor == 2, t);
        self.quick_button(174, 132, "AIR", f.airplane, f.cursor == 3, t);
        self.quick_button(66, 196, "LIGHT", true, f.cursor == 4, t);
        self.quick_button(174, 196, "SET", false, f.cursor == 5, t);
        self.icon_sun(25, 250, t.on_surface_variant);
        self.slider(43, 246, 165, f.brightness, t);
    }


    fn settings(&mut self, f: MaterialFrame, t: Theme) {
        self.title("SETTINGS", t);
        let labels = [
            "CONNECTIVITY", "APPS & NOTIFS", "DISPLAY", "SOUND & VIBRATION",
            "GESTURES", "ACCESSIBILITY", "SECURITY", "SYSTEM", "BACK"
        ];
        let cur = f.cursor.min(8) as usize;
        let start = cur.saturating_sub(1).min(6);
        for row in 0..3 {
            let i = (start + row).min(8);
            self.wear_row(55 + row as i32 * 64, labels[i], i == cur, t);
        }
    }


    fn clock(&mut self, f: MaterialFrame, t: Theme) {
        self.big_time(34, f.time_minutes, f.time_valid, t.on_surface);
        self.text_center(98, "CLOCK", 1, t.on_surface_variant);
        let labels = ["TIMER", "STOPWATCH", "ALARM"];
        for i in 0..3 {
            let selected = f.cursor as usize == i;
            let cx = 48 + i as i32 * 72;
            self.circle(cx, 165, if selected { 28 } else { 23 }, if selected { t.primary } else { t.surface_high });
            self.icon_clock(cx, 165, if selected { t.on_primary } else { t.on_surface });
            self.text_center_x(cx, 202, labels[i], 1, t.on_surface_variant);
        }
    }


    fn connectivity(&mut self, f: MaterialFrame, t: Theme) {
        self.title("CONNECTIVITY", t);
        let labels = ["WI-FI", "BLUETOOTH", "AIRPLANE MODE", "BACK"];
        let vals = [f.wifi, f.bt, f.airplane, false];
        let cur = f.cursor.min(3) as usize;
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let i = (start + row).min(3);
            let y = 57 + row as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i < 3 { self.toggle(174, y + 13, vals[i], t); }
        }
    }


    fn about(&mut self, f: MaterialFrame, t: Theme) {
        self.title("ABOUT", t);
        self.circle(120, 72, 30, t.primary_container);
        self.text_center(57, "A", 4, t.primary);
        self.text_center(116, "AOSP WEAR", 2, t.on_surface);
        self.text_center(144, "ANDROID 17", 1, t.primary);
        self.round_rect(22, 168, 196, 36, 18, t.surface);
        self.text_center(180, "BUILD 17.2.1", 1, t.on_surface);
        self.text_center(216, "ESP32-S3-ZERO-N4R2", 1, t.on_surface_variant);
        self.text_center(238, if f.swap_pages > 0 { "SYSTEM STORAGE ACTIVE" } else { "SYSTEM STORAGE READY" }, 1, t.outline);
    }


    fn widgets(&mut self, f: MaterialFrame, t: Theme) {
        let cur = f.cursor.min(2);
        // Edge-to-edge Wear tile card.  Each tile gets its own accent family
        // instead of looking like another Settings page.
        let (bg, fg, sub) = if cur == 0 {
            (rgb565(58, 113, 94), rgb565(228, 255, 241), if f.wifi { "WI-FI CONNECTED" } else { "WI-FI OFF" })
        } else if cur == 1 {
            (rgb565(47, 78, 126), rgb565(226, 236, 255), if f.timer_secs > 0 { "TIMER RUNNING" } else { "5 MINUTE TIMER" })
        } else {
            (rgb565(92, 67, 119), rgb565(247, 231, 255), if f.alarm_enabled { "ALARM ON" } else { "ALARM OFF" })
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
            let timer = Self::mmss_text(if f.timer_secs == 0 { 300 } else { f.timer_secs });
            let timer_s = core::str::from_utf8(&timer).unwrap_or("05:00");
            self.text(27, 153, timer_s, 2, fg);
        } else {
            self.icon_clock(52, 66, fg);
            self.text(82, 55, "ALARM", 1, fg);
            self.text(27, 112, sub, 2, fg);
            self.text(27, 153, "PRESS TO OPEN", 1, fg);
        }
        // Carousel dots deliberately sit outside the card like Wear tiles.
        for i in 0..3 { self.circle(108 + i * 12, 254, if i as u8 == cur { 4 } else { 2 }, if i as u8 == cur { t.primary } else { t.outline }); }
    }


    fn media(&mut self, f: MaterialFrame, t: Theme) {
        let tm = Self::time_text(f.time_minutes);
        let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(9, ts, 1, t.on_surface_variant);
        self.circle(120, 78, 38, t.surface_high);
        self.text_center(61, "M", 4, t.primary);
        self.text_center(128, "NO MEDIA PLAYING", 1, t.on_surface);
        self.text_center(149, "START MEDIA ON PHONE", 1, t.on_surface_variant);
        let labels = ["<", "||", ">"];
        for i in 0..3 {
            let cx = 55 + i as i32 * 65;
            let selected = f.cursor as usize == i;
            self.circle(cx, 204, if selected { 26 } else { 21 }, if selected { t.primary } else { t.surface_high });
            self.text_center_x(cx, 198, labels[i], 2, if selected { t.on_primary } else { t.on_surface });
        }
    }


    fn clock_tools(&mut self, f: MaterialFrame, t: Theme) {
        self.title("CLOCK", t);
        let cur = f.cursor.min(3) as usize;
        let timer = Self::mmss_text(if f.timer_secs == 0 { 300 } else { f.timer_secs });
        let stopwatch = Self::mmss_text(f.stopwatch_secs);
        let timer_s = core::str::from_utf8(&timer).unwrap_or("05:00");
        let stopwatch_s = core::str::from_utf8(&stopwatch).unwrap_or("00:00");
        let labels = ["TIMER", "STOPWATCH", "ALARM", "BACK"];
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let i = (start + row).min(3);
            let y = 57 + row as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i == 0 { self.text(154, y + 19, timer_s, 1, if f.timer_secs > 0 { t.primary } else { t.on_surface_variant }); }
            if i == 1 { self.text(154, y + 19, stopwatch_s, 1, t.on_surface_variant); }
            if i == 2 { self.text(174, y + 19, if f.alarm_enabled { "ON" } else { "OFF" }, 1, if f.alarm_enabled { t.primary } else { t.on_surface_variant }); }
        }
    }


    fn wifi_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("WI-FI", t);
        self.round_rect(18, 52, 204, 58, 29, if f.cursor == 0 { t.primary_container } else { t.surface });
        self.icon_wifi(47, 81, if f.wifi { t.primary } else { t.outline });
        self.text(72, 66, "USE WI-FI", 1, t.on_surface);
        self.text(72, 84, if f.wifi { "ON" } else { "OFF" }, 1, t.on_surface_variant);
        self.toggle(171, 69, f.wifi, t);
        self.info_card(124, "NETWORKS", if f.wifi { "SCAN / SAVED" } else { "WI-FI IS OFF" }, f.cursor == 1, t);
        self.info_card(188, "BACK", "CONNECTIVITY", f.cursor == 2, t);
    }



    fn wifi_ssid<'a>(f: &'a MaterialFrame) -> &'a str {
        let n = (f.wifi_ap_ssid_len as usize).min(32);
        core::str::from_utf8(&f.wifi_ap_ssid[..n]).unwrap_or("UNKNOWN")
    }

    fn wifi_networks(&mut self, f: MaterialFrame, t: Theme) {
        self.title("AVAILABLE NETWORKS", t);
        if f.wifi_ap_count == 0 {
            self.circle(120, 104, 34, t.surface);
            self.icon_wifi(120, 104, t.outline);
            self.text_center(158, "SCANNING...", 2, t.on_surface);
            self.text_center(184, "ROTATE CROWN AFTER SCAN", 1, t.on_surface_variant);
            self.round_rect(48, 222, 144, 38, 19, t.surface_high);
            self.text_center(234, "BACK", 1, t.on_surface);
            return;
        }
        let cur = f.cursor.min(f.wifi_ap_count);
        if cur >= f.wifi_ap_count {
            self.round_rect(30, 87, 180, 94, 36, t.primary_container);
            self.text_center(113, "BACK", 2, t.on_primary_container);
            self.text_center(146, "RETURN TO WI-FI", 1, t.on_surface_variant);
        } else {
            self.round_rect(16, 55, 208, 145, 38, t.surface);
            self.icon_wifi(120, 91, if f.wifi_ap_rssi > -70 { t.primary } else { t.outline });
            self.text_center(124, Self::wifi_ssid(&f), 2, t.on_surface);
            let sig = if f.wifi_ap_rssi > -55 { "EXCELLENT" } else if f.wifi_ap_rssi > -70 { "GOOD" } else { "WEAK" };
            self.text_center(151, sig, 1, t.on_surface_variant);
            self.text_center(176, "PRESS TO CONNECT", 1, t.primary);
        }
        for i in 0..f.wifi_ap_count.min(7) {
            let x = 120 - ((f.wifi_ap_count.min(7) as i32 - 1) * 6) + i as i32 * 12;
            self.circle(x, 231, if i == f.wifi_ap_index { 4 } else { 2 }, if i == f.wifi_ap_index { t.primary } else { t.outline });
        }
    }

    fn wifi_password(&mut self, f: MaterialFrame, t: Theme) {
        const ALPHA: &[u8] = b"qwertyuiopasdfghjklzxcvbnm";
        const SYMBOLS_1: &[u8] = b"1234567890!@#$%^&*()_+-=[]";
        const SYMBOLS_2: &[u8] = b"{}.,:;?'\"/\\|<>`~";

        let page = (f.wifi_editor_char >> 6).min(3);
        let selected = (f.wifi_editor_char & 0x3f) as usize;
        let chars = match page { 2 => SYMBOLS_1, 3 => SYMBOLS_2, _ => ALPHA };

        // Compact Gboard-like password editor: no suggestion strip, only the
        // field and keyboard.  It is laid out for one-dimensional encoder
        // focus, so the selected key lifts/enlarges instead of relying on touch.
        self.text_center(5, Self::wifi_ssid(&f), 1, t.on_surface_variant);
        self.round_rect(12, 21, 216, 40, 18, t.surface);
        if f.wifi_password_len == 0 {
            self.text(26, 37, "PASSWORD", 1, t.outline);
        } else {
            // Crown-only input is easy to overshoot by one detent. Show the
            // actual tail of the password on the physical watch so a wrong
            // neighbouring key is visible before CONNECT is pressed. The
            // preview is intentionally not serialized to desktop/web remotes.
            let n = f.wifi_password_preview_len.min(16) as usize;
            let visible = core::str::from_utf8(&f.wifi_password_preview[..n]).unwrap_or("?");
            self.text(25, 35, visible, 1, t.on_surface);
            if f.wifi_password_len as usize > n { self.text(15, 35, "~", 1, t.outline); }
        }
        // Small lock-like dot shows that this is a password field.
        self.circle(211, 41, 5, t.primary_container);
        self.circle(211, 41, 2, t.primary);

        let draw_char_key = |this: &mut Surface, x: i32, y: i32, w: i32, h: i32, ch: u8, idx: usize| {
            let is_sel = selected == idx;
            let yy = if is_sel { y - 3 } else { y };
            let hh = if is_sel { h + 6 } else { h };
            let bg = if is_sel { t.primary } else { t.surface_high };
            let fg = if is_sel { t.on_primary } else { t.on_surface };
            this.round_rect(x, yy, w, hh, 8, bg);
            let one = [if page == 1 { ch.to_ascii_uppercase() } else { ch }];
            let label = core::str::from_utf8(&one).unwrap_or("?");
            let scale = if is_sel { 2 } else { 1 };
            this.text_center_x(x + w/2, yy + (hh - 7*scale)/2, label, scale, fg);
        };

        if page <= 1 {
            // QWERTY rows matching a phone keyboard silhouette.
            let rows = [10usize, 9usize, 7usize];
            let mut base = 0usize;
            for (r, count) in rows.into_iter().enumerate() {
                let y = 76 + r as i32 * 37;
                if r < 2 {
                    let kw = 20; let gap = 2;
                    let row_w = count as i32 * kw + (count as i32 - 1) * gap;
                    let mut x = (WIDTH as i32 - row_w) / 2;
                    for pos in 0..count {
                        let idx = base + pos; let ch = chars[idx];
                        draw_char_key(self, x, y, kw, 29, ch, idx);
                        x += kw + gap;
                    }
                } else {
                    // Third row is inset like Gboard; Shift/Delete live on the
                    // service row below, so all letters remain large enough.
                    let kw = 22; let gap = 3;
                    let row_w = count as i32 * kw + (count as i32 - 1) * gap;
                    let mut x = (WIDTH as i32 - row_w) / 2;
                    for pos in 0..count {
                        let idx = base + pos; let ch = chars[idx];
                        draw_char_key(self, x, y, kw, 29, ch, idx);
                        x += kw + gap;
                    }
                }
                base += count;
            }
        } else {
            let rows: &[usize] = if page == 2 { &[10, 10, 6] } else { &[9, 8] };
            let mut base = 0usize;
            for (r, &count) in rows.iter().enumerate() {
                let y = 76 + r as i32 * 37;
                let kw = if count >= 10 { 20 } else { 22 }; let gap = 2;
                let row_w = count as i32 * kw + (count as i32 - 1) * gap;
                let mut x = (WIDTH as i32 - row_w) / 2;
                for pos in 0..count {
                    let idx = base + pos; if idx >= chars.len() { break; }
                    draw_char_key(self, x, y, kw, 29, chars[idx], idx);
                    x += kw + gap;
                }
                base += count;
            }
        }

        let n = chars.len();
        let (page_key, alpha_key) = match page {
            2 => ("#+=", "ABC"),
            3 => ("123", "ABC"),
            _ => (if page == 1 { "abc" } else { "SHIFT" }, "123"),
        };
        // Service row mirrors a phone keyboard, but every key remains a simple
        // crown target in the same linear focus order.
        self.keyboard_key(6, 188, 50, 31, page_key, selected == n, t);
        self.keyboard_key(60, 188, 42, 31, alpha_key, selected == n + 1, t);
        self.keyboard_key(106, 188, 74, 31, "SPACE", selected == n + 2, t);
        self.keyboard_key(184, 188, 50, 31, "DEL", selected == n + 3, t);

        self.keyboard_key(6, 226, 64, 34, "BACK", selected == n + 5, t);
        self.keyboard_key(76, 226, 158, 34, "CONNECT", selected == n + 4, t);

        // Make the encoder model discoverable without a permanent instruction
        // banner: selected action is obvious from the raised accent key.
        self.text_center(266, "ROTATE  /  PRESS", 1, t.outline);
    }


    fn wifi_connecting(&mut self, f: MaterialFrame, t: Theme) {
        self.title("WI-FI", t);
        self.circle(120, 102, 38, t.surface);
        self.icon_wifi(120, 102, if f.wifi_link_state == 4 { t.primary } else { t.outline });
        let (title, sub) = match f.wifi_link_state {
            4 => ("CONNECTED", "TIME SYNC STARTED"),
            3 => ("GETTING ADDRESS", "DHCP"),
            2 => ("LINK READY", "STARTING NETWORK"),
            1 => ("CONNECTING", "ASSOCIATING"),
            _ => ("NOT CONNECTED", "PRESS TO RETURN"),
        };
        self.text_center(159, title, 2, if f.wifi_link_state == 4 { t.primary } else { t.on_surface });
        self.text_center(187, sub, 1, t.on_surface_variant);
        self.round_rect(48, 222, 144, 38, 19, t.surface_high);
        self.text_center(234, "DONE", 1, t.on_surface);
    }

    fn bluetooth_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("BLUETOOTH", t);
        self.round_rect(18, 52, 204, 58, 29, if f.cursor == 0 { t.primary_container } else { t.surface });
        self.icon_bt(47, 81, if f.bt { t.primary } else { t.outline });
        self.text(72, 66, "BLUETOOTH", 1, t.on_surface);
        self.text(72, 84, if f.bt { "ON" } else { "OFF" }, 1, t.on_surface_variant);
        self.toggle(171, 69, f.bt, t);
        self.info_card(124, "PAIR NEW DEVICE", if f.bt { "READY TO SCAN" } else { "BLUETOOTH OFF" }, f.cursor == 1, t);
        self.info_card(188, "WATCH NAME", "AOSP WEAR", f.cursor == 2, t);
    }


    fn display_settings(&mut self, f: MaterialFrame, t: Theme) {
        self.title("DISPLAY", t);
        self.round_rect(16, 55, 208, 66, 33, if f.cursor == 0 { t.primary_container } else { t.surface });
        self.icon_sun(42, 86, t.primary);
        self.text(66, 68, "BRIGHTNESS", 1, t.on_surface_variant);
        self.slider(66, 94, 132, f.brightness, t);
        self.round_rect(16, 135, 208, 58, 29, if f.cursor == 1 { t.primary_container } else { t.surface });
        self.text(38, 148, "DYNAMIC COLOR", 1, t.on_surface_variant);
        let color = match f.theme { 1 => "BLUE", 2 => "PURPLE", 3 => "CORAL", _ => "GREEN" };
        self.text(38, 168, color, 2, t.primary);
        self.round_rect(16, 207, 208, 42, 21, if f.cursor == 2 { t.primary_container } else { t.surface });
        self.text_center(219, "BACK", 1, t.on_surface);
    }


    fn system_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("SYSTEM", t);
        let labels = ["DATE & TIME", "ABOUT", "DEVELOPER OPTIONS", "BACK"];
        let cur = f.cursor.min(3) as usize;
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let i = (start + row).min(3);
            self.wear_row(55 + row as i32 * 64, labels[i], i == cur, t);
        }
    }

    fn date_time(&mut self, f: MaterialFrame, t: Theme) {
        self.title("DATE & TIME", t);
        let labels = ["AUTOMATIC TIME", "SET HOUR", "SET MINUTE", "TIME ZONE", "BACK"];
        let cur = f.cursor.min(4) as usize;
        let start = cur.saturating_sub(1).min(2);
        for row in 0..3 {
            let i = (start + row).min(4);
            let y = 56 + row as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i == 0 { self.toggle(174, y + 13, f.auto_time, t); }
            if i == 1 || i == 2 {
                let tm = Self::time_text(f.time_minutes);
                let ts = core::str::from_utf8(&tm).unwrap_or("00:00");
                self.text(167, y + 19, ts, 1, if f.auto_time { t.outline } else { t.primary });
            }
            if i == 3 {
                if f.timezone_hours == 0 { self.text(178, y + 19, "UTC", 1, t.on_surface_variant); }
                else if f.timezone_hours > 0 { self.text(166, y + 19, "UTC+", 1, t.on_surface_variant); }
                else { self.text(166, y + 19, "UTC-", 1, t.on_surface_variant); }
            }
        }
        if f.auto_time && !f.time_valid { self.text_center(251, "SYNCING OVER WI-FI / PHONE", 1, t.primary); }
    }

    fn unavailable_page(&mut self, title: &str, icon: &str, body: &str, t: Theme) {
        self.title(title, t);
        self.circle(120, 103, 37, t.surface);
        self.text_center(88, icon, 3, t.primary);
        self.text_center(157, body, 1, t.on_surface);
        self.text_center(180, "NOT PRESENT ON THIS BOARD", 1, t.on_surface_variant);
        self.round_rect(45, 217, 150, 38, 19, t.surface_high);
        self.text_center(229, "BACK", 1, t.on_surface);
    }

    fn sound_page(&mut self, _f: MaterialFrame, t: Theme) {
        self.unavailable_page("SOUND & VIBRATION", "S", "AUDIO HARDWARE", t);
    }

    fn gestures_page(&mut self, _f: MaterialFrame, t: Theme) {
        self.title("GESTURES", t);
        self.info_card(65, "CROWN", "ROTATE TO SCROLL", true, t);
        self.info_card(132, "CROWN PRESS", "SELECT / APPS", false, t);
        self.info_card(199, "HOME", "RETURN TO WATCH FACE", false, t);
    }

    fn accessibility_page(&mut self, _f: MaterialFrame, t: Theme) {
        self.title("ACCESSIBILITY", t);
        self.info_card(68, "INPUT", "CROWN-ONLY MODE", true, t);
        self.info_card(135, "CONTRAST", "DARK WATCH UI", false, t);
        self.round_rect(45, 213, 150, 40, 20, t.surface_high);
        self.text_center(226, "BACK", 1, t.on_surface);
    }

    fn security_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("SECURITY", t);
        self.round_rect(18, 65, 204, 64, 32, if f.cursor == 0 { t.primary_container } else { t.surface });
        self.text(42, 78, "SCREEN LOCK", 1, t.on_surface);
        self.text(42, 99, if f.locked { "ENABLED" } else { "NONE" }, 1, t.on_surface_variant);
        self.toggle(169, 85, f.locked, t);
        self.round_rect(45, 194, 150, 42, 21, if f.cursor == 1 { t.primary_container } else { t.surface });
        self.text_center(207, "BACK", 1, t.on_surface);
    }

    fn apps_notifications_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("APPS & NOTIFS", t);
        let labels = ["NOTIFICATIONS", "APP INFO", "BACK"];
        let cur = f.cursor.min(2) as usize;
        for i in 0..3 {
            let y = 57 + i as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i == 0 { self.text(176, y + 19, if f.notes > 0 { "NEW" } else { "0" }, 1, t.on_surface_variant); }
            if i == 1 { self.text(176, y + 19, "6", 1, t.on_surface_variant); }
        }
    }

    fn developer_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("DEVELOPER OPTIONS", t);
        let labels = ["WIRELESS DEBUG", "BUILD NUMBER", "SYSTEM", "BACK"];
        let cur = f.cursor.min(3) as usize;
        let start = cur.saturating_sub(1).min(1);
        for row in 0..3 {
            let i = (start + row).min(3);
            let y = 57 + row as i32 * 64;
            self.wear_row(y, labels[i], i == cur, t);
            if i == 0 { self.toggle(174, y + 13, f.adb, t); }
            if i == 1 { self.text(157, y + 19, "17.2.1", 1, t.primary); }
        }
        if f.adb { self.text_center(253, "WIRELESS ADB IS A DEVELOPER FEATURE", 1, t.error); }
    }

}
