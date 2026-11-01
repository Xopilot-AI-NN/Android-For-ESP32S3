extern crate alloc;

use alloc::{vec, vec::Vec};

#[path = "material/localization.rs"]
mod localization;
pub const DEFAULT_LOCALE: &str = localization::DEFAULT_LOCALE;

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
    /// Live HID state: off, advertising, connected, ready, confirmation, error.
    pub bt_status: u8,
    pub bt_pin: Option<u32>,
    pub alarm_minutes: u16,
    pub clock_alert: u8,
    pub wifi_ip: Option<[u8; 4]>,
    pub timer_editor: u8,
    pub timer_duration_secs: u16,
}

impl MaterialFrame {
    pub fn parse(raw: &str) -> Option<Self> {
        let mut p = raw.split('|');
        if p.next()? != "M3" {
            return None;
        }
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
        let timer_secs = p.next()?.parse::<u16>().ok()?.min(3600);
        let stopwatch_secs = p.next()?.parse::<u16>().ok()?.min(3599);
        let alarm_enabled = p.next()? == "1";
        let auto_time = p.next()? == "1";
        let time_valid = p.next()? == "1";
        let timezone_hours = p.next()?.parse::<i8>().ok()?.clamp(-12, 14);
        if p.next().is_some() {
            return None;
        }
        Some(Self {
            screen,
            cursor,
            brightness,
            dnd,
            airplane,
            theme,
            locked,
            wifi,
            bt,
            adb,
            notes,
            time_minutes,
            timer_secs,
            stopwatch_secs,
            alarm_enabled,
            auto_time,
            time_valid,
            timezone_hours,
            swap_pages: 0,
            wifi_ap_count: 0,
            wifi_ap_index: 0,
            wifi_ap_ssid: [0; 32],
            wifi_ap_ssid_len: 0,
            wifi_ap_rssi: -127,
            wifi_password_len: 0,
            wifi_password_preview: [0; 16],
            wifi_password_preview_len: 0,
            wifi_editor_char: 0,
            wifi_link_state: 0,
            bt_status: 0,
            bt_pin: None,
            alarm_minutes: 420,
            clock_alert: 0,
            wifi_ip: None,
            timer_editor: 0,
            timer_duration_secs: 300,
        })
    }

    pub fn set_swap_pages(&mut self, pages: u8) {
        self.swap_pages = pages;
    }

    pub fn fallback(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self.screen {
            0 => ("LOCK", "WEAR OS", "CROWN TO UNLOCK", ""),
            1 => ("WATCH FACE", "", "CROWN: APPS", ""),
            2 => ("APPS", "CLOCK", "MEDIA CONTROLS", "SETTINGS"),
            3 => ("NOTIFICATIONS", "", "", ""),
            4 => (
                "QUICK SETTINGS",
                "WIFI / BT",
                "DND / AIRPLANE",
                "BRIGHTNESS",
            ),
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
            28 => ("BRIGHTNESS", "CROWN TO ADJUST", "", ""),
            29 => ("FLASHLIGHT", "WHITE SCREEN", "", ""),
            30 => ("BLUETOOTH", "PAIR NEW DEVICE", "", ""),
            _ => ("WEAR OS", "ANDROID 17", "", ""),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Rect {
    pub const FULL: Self = Self {
        x: 0,
        y: 0,
        w: WIDTH as i32,
        h: HEIGHT as i32,
    };
    pub fn x1(self) -> i32 {
        self.x + self.w - 1
    }
    pub fn y1(self) -> i32 {
        self.y + self.h - 1
    }
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
            bg: rgb565(0, 0, 0),
            surface: rgb565(16, 21, 31),
            surface_high: rgb565(27, 34, 47),
            primary: rgb565(174, 203, 255),
            on_primary: rgb565(8, 48, 88),
            primary_container: rgb565(32, 69, 111),
            on_primary_container: rgb565(219, 230, 255),
            on_surface: rgb565(241, 242, 248),
            on_surface_variant: rgb565(191, 198, 211),
            outline: rgb565(118, 127, 143),
            error: rgb565(255, 180, 171),
        },
        2 => Theme {
            bg: rgb565(0, 0, 0),
            surface: rgb565(24, 18, 28),
            surface_high: rgb565(39, 29, 45),
            primary: rgb565(224, 187, 255),
            on_primary: rgb565(72, 31, 97),
            primary_container: rgb565(100, 55, 128),
            on_primary_container: rgb565(244, 220, 255),
            on_surface: rgb565(246, 239, 247),
            on_surface_variant: rgb565(207, 194, 211),
            outline: rgb565(142, 128, 147),
            error: rgb565(255, 180, 171),
        },
        3 => Theme {
            bg: rgb565(0, 0, 0),
            surface: rgb565(29, 19, 17),
            surface_high: rgb565(47, 31, 27),
            primary: rgb565(255, 184, 161),
            on_primary: rgb565(86, 33, 16),
            primary_container: rgb565(119, 55, 34),
            on_primary_container: rgb565(255, 221, 211),
            on_surface: rgb565(249, 238, 233),
            on_surface_variant: rgb565(216, 195, 188),
            outline: rgb565(150, 129, 123),
            error: rgb565(255, 180, 171),
        },
        _ => Theme {
            bg: rgb565(0, 0, 0),
            surface: rgb565(27, 46, 62),
            surface_high: rgb565(54, 83, 109),
            primary: rgb565(166, 222, 239),
            on_primary: rgb565(4, 40, 50),
            primary_container: rgb565(32, 72, 85),
            on_primary_container: rgb565(245, 254, 254),
            on_surface: rgb565(243, 250, 255),
            on_surface_variant: rgb565(183, 204, 221),
            outline: rgb565(134, 161, 180),
            error: rgb565(255, 180, 171),
        },
    }
}

pub struct Surface {
    pixels: Vec<u16>,
}

impl Surface {
    pub fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT],
        }
    }
    pub fn pixels(&self) -> &[u16] {
        &self.pixels
    }

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
            28 => self.brightness_editor(frame, t),
            29 => self.fill_rect(0, 0, WIDTH as i32, HEIGHT as i32, rgb565(255, 255, 255)),
            30 => self.bluetooth_pairing(frame, t),
            31 => self.alarm_editor(frame, t),
            _ => self.watchface(frame, t),
        }
        if frame.clock_alert != 0 {
            self.round_rect(8, 90, 224, 108, 30, t.primary);
            self.text_center(
                113,
                if frame.clock_alert == 2 {
                    "Alarm"
                } else {
                    "Timer finished"
                },
                2,
                t.on_primary,
            );
            self.text_center(157, "PRESS TO DISMISS", 1, t.on_primary);
        }
        Rect::FULL
    }
}

#[path = "material/components.rs"]
mod components;
#[path = "material/icons.rs"]
mod icons;
#[path = "material/pages_bluetooth.rs"]
mod pages_bluetooth;
#[path = "material/pages_main.rs"]
mod pages_main;
#[path = "material/pages_settings.rs"]
mod pages_settings;
#[path = "material/pages_wifi.rs"]
mod pages_wifi;
#[path = "material/primitives.rs"]
mod primitives;
#[path = "material/typography.rs"]
mod typography;

#[path = "material/symbols/mod.rs"]
mod symbols;
use symbols::Symbol;
