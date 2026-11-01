extern crate alloc;

use alloc::{
    format,
    string::{String, ToString},
};
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use esp_hal::time::Instant;
use heapless::{String as HString, Vec as HVec};

#[path = "runtime/clock.rs"]
mod clock;
#[path = "runtime/idle.rs"]
mod idle;
#[path = "runtime/security.rs"]
mod security;
use clock::{clock_action, clock_snapshot};

use crate::material::MaterialFrame;

#[cfg(feature = "rhai-runtime")]
use esp_println::println;
#[cfg(feature = "rhai-runtime")]
use rhai::{AST, CallFnOptions, Engine, ImmutableString, Scope};

// Compact full QWERTY keyboard used by the crown-only Wi-Fi password editor.
// The visible rows mirror a phone/watch keyboard instead of the old single-
// character carousel.  Password bytes never leave this Runtime object.
const WIFI_KB_ALPHA: &[u8] = b"qwertyuiopasdfghjklzxcvbnm";
// Together these two symbol pages plus SPACE cover the complete printable
// ASCII set used by WPA/WPA2 passphrases (0x20..=0x7e).
const WIFI_KB_SYMBOLS_1: &[u8] = b"1234567890!@#$%^&*()_+-=[]";
const WIFI_KB_SYMBOLS_2: &[u8] = b"{}.,:;?'\"/\\|<>`~";
const WIFI_KB_LOWER: u8 = 0;
const WIFI_KB_UPPER: u8 = 1;
const WIFI_KB_SYMBOL_1: u8 = 2;
const WIFI_KB_SYMBOL_2: u8 = 3;

fn wifi_keyboard_chars(page: u8) -> &'static [u8] {
    match page {
        WIFI_KB_SYMBOL_1 => WIFI_KB_SYMBOLS_1,
        WIFI_KB_SYMBOL_2 => WIFI_KB_SYMBOLS_2,
        _ => WIFI_KB_ALPHA,
    }
}
fn wifi_keyboard_char_count(page: u8) -> usize {
    wifi_keyboard_chars(page).len()
}
fn wifi_keyboard_total(page: u8) -> usize {
    wifi_keyboard_char_count(page) + 6
}

#[derive(Clone, Debug)]
struct WifiUiAp {
    ssid: HString<32>,
    rssi: i8,
}

#[derive(Clone, Debug)]
struct WifiUi {
    aps: HVec<WifiUiAp, 10>,
    selected: u8,
    password: HString<64>,
    editor_char: u8,
    keyboard_page: u8,
    pending_scan: bool,
    pending_connect: bool,
    link_state: u8,
}
impl WifiUi {
    fn new() -> Self {
        Self {
            aps: HVec::new(),
            selected: 0,
            password: HString::new(),
            editor_char: 0,
            keyboard_page: WIFI_KB_LOWER,
            pending_scan: false,
            pending_connect: false,
            link_state: 0,
        }
    }
}

pub enum WifiUiAction {
    Scan,
    Connect {
        ssid: HString<32>,
        password: HString<64>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BluetoothUiAction {
    Pair,
    Confirm(bool),
    Disconnect,
    Forget,
    Media(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    Script,
    NonZero,
    Protocol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEvent {
    Sync,
    Home,
    Back,
    Rotate(i32),
    Swipe(i32),
    Press,
}

#[derive(Clone, Debug)]
pub struct LegacyFrame {
    pub title: String,
    pub line1: String,
    pub line2: String,
    pub line3: String,
}

#[derive(Clone, Debug)]
pub enum RuntimeFrame {
    Material(MaterialFrame),
    Legacy(LegacyFrame),
}

impl RuntimeFrame {
    pub fn set_swap_pages(&mut self, pages: u8) {
        if let Self::Material(frame) = self {
            frame.set_swap_pages(pages);
        }
    }

    fn parse(raw: &str) -> Result<Self, RuntimeError> {
        if let Some(frame) = MaterialFrame::parse(raw) {
            return Ok(Self::Material(frame));
        }
        let mut parts = raw.split('|');
        let title = parts.next().ok_or(RuntimeError::Protocol)?;
        let line1 = parts.next().ok_or(RuntimeError::Protocol)?;
        let line2 = parts.next().ok_or(RuntimeError::Protocol)?;
        let line3 = parts.next().ok_or(RuntimeError::Protocol)?;
        if parts.next().is_some() {
            return Err(RuntimeError::Protocol);
        }
        if title.len() > 22 || line1.len() > 28 || line2.len() > 28 || line3.len() > 28 {
            return Err(RuntimeError::Protocol);
        }
        Ok(Self::Legacy(LegacyFrame {
            title: title.to_string(),
            line1: line1.to_string(),
            line2: line2.to_string(),
            line3: line3.to_string(),
        }))
    }
}

#[cfg(feature = "rhai-runtime")]
pub struct Runtime {
    engine: Engine,
    scope: Scope<'static>,
    ast: AST,
    /// Firmware-owned persistent state.  Rhai functions are intentionally
    /// pure with respect to outer Scope variables, so state crosses the ABI
    /// explicitly instead of relying on closure/global-variable semantics.
    state: i32,
    wifi_ui: WifiUi,
    flashlight_since_secs: Option<u64>,
    bluetooth_action: Option<BluetoothUiAction>,
    bluetooth_status: u8,
    bluetooth_pin: Option<u32>,
    wifi_ip: Option<[u8; 4]>,
    timer_editor: u8,
    security: security::Security,
    security_saved: [u8; 58],
    crown_sensitivity: u8,
    crown_reverse: bool,
    high_contrast: bool,
    last_input_ms: u64,
    timeout_parent: u8,
    transition_started_ms: u64,
}

#[cfg(feature = "rhai-runtime")]
impl Runtime {
    pub fn start(script: String) -> Result<(Self, RuntimeFrame), RuntimeError> {
        let mut engine = Engine::new();
        // Firmware is trusted/AVB-verified userspace. Keep a bounded parser, but
        // allow normal Material/SystemUI expressions inside function bodies.
        engine.set_max_expr_depths(64, 32);
        engine.set_max_operations(60_000);
        engine.set_max_call_levels(16);

        engine.register_fn("system_locale", || {
            crate::material::DEFAULT_LOCALE.to_string()
        });
        engine.register_fn("boot_version", || 2_i32);
        engine.register_fn("android_api_level", || 37_i32);
        engine.register_fn("runtime_abi", || {
            "aosp-wear-android17-qpr1-rhai-v8-m3e".to_string()
        });
        engine.register_fn("clock_action", |action: i32| clock_action(action));
        engine.register_fn("time_action", |action: i32| crate::rtc::action(action));
        engine.register_fn(
            "m3_scene",
            |screen: i32,
             cursor: i32,
             brightness: i32,
             dnd: i32,
             airplane: i32,
             theme: i32,
             locked: i32,
             wifi: i32,
             bt: i32,
             adb: i32,
             notes: i32| {
                // Wall clock advances from the ESP monotonic timer, but is marked invalid
                // until an explicit Wear companion/manual source sets it. USB debug
                // transport never seeds wall time implicitly.
                let mins = crate::rtc::local_minutes();
                let (timer_secs, stopwatch_secs, alarm_enabled) = clock_snapshot();
                format!(
                    "M3|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                    screen.clamp(0, 31),
                    cursor.clamp(0, 15),
                    brightness.clamp(0, 100),
                    if dnd != 0 { 1 } else { 0 },
                    if airplane != 0 { 1 } else { 0 },
                    theme.rem_euclid(4),
                    if locked != 0 { 1 } else { 0 },
                    if wifi != 0 { 1 } else { 0 },
                    if bt != 0 { 1 } else { 0 },
                    if adb != 0 { 1 } else { 0 },
                    notes.clamp(0, 7),
                    mins,
                    timer_secs,
                    stopwatch_secs,
                    alarm_enabled,
                    if crate::rtc::automatic() { 1 } else { 0 },
                    if crate::rtc::valid() { 1 } else { 0 },
                    crate::rtc::timezone_hours() as i32
                )
            },
        );
        #[cfg(feature = "display-ssd1306")]
        {
            engine.register_fn("device_width", || 128_i32);
            engine.register_fn("device_height", || 64_i32);
        }
        #[cfg(feature = "display-st7789")]
        {
            engine.register_fn("device_width", || 240_i32);
            engine.register_fn("device_height", || 280_i32);
        }

        let ast = engine.compile(script.as_str()).map_err(|err| {
            println!("rhai compile error: {:?}", err);
            RuntimeError::Script
        })?;
        drop(script);

        let mut scope = Scope::new();
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let state = engine
            .call_fn_with_options::<i32>(options, &mut scope, &ast, "init", ())
            .map_err(|err| {
                println!("rhai init error: {:?}", err);
                RuntimeError::Script
            })?;
        if state < 0 {
            return Err(RuntimeError::NonZero);
        }

        let mut runtime = Self {
            engine,
            scope,
            ast,
            state,
            wifi_ui: WifiUi::new(),
            flashlight_since_secs: None,
            bluetooth_action: None,
            bluetooth_status: 0,
            bluetooth_pin: None,
            wifi_ip: None,
            timer_editor: 0,
            security: security::Security::new(Instant::now().duration_since_epoch().as_secs()),
            security_saved: security::Security::new(0).encode(),
            crown_sensitivity: 1,
            crown_reverse: false,
            high_contrast: false,
            last_input_ms: Instant::now().duration_since_epoch().as_millis() as u64,
            timeout_parent: 20,
            transition_started_ms: 0,
        };
        let frame = runtime.render()?;
        Ok((runtime, frame))
    }

    pub fn render(&mut self) -> Result<RuntimeFrame, RuntimeError> {
        if self.security_saved[5] != 0 || self.security.encode() == self.security_saved {
            self.security
                .tick(Instant::now().duration_since_epoch().as_secs());
        }
        if self.security.locked {
            self.state &= !0x1ff;
        }
        if self.security.has_pin {
            self.state |= 1 << 20;
        } else {
            self.state &= !(1 << 20);
        }
        if self.state & 31 != 11 {
            self.timer_editor = 0;
        }
        // The flashlight drives an already retention-prone LCD fully white.
        // Always return to Quick Settings after 45 seconds, even without input.
        if (self.state & 31) == 29 {
            let now = Instant::now().duration_since_epoch().as_secs();
            if let Some(start) = self.flashlight_since_secs {
                if now.saturating_sub(start) >= 45 {
                    self.state = (self.state & -512) | 4 | (4 << 5);
                    self.flashlight_since_secs = None;
                }
            } else {
                self.flashlight_since_secs = Some(now);
            }
        } else {
            self.flashlight_since_secs = None;
        }
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let raw = self
            .engine
            .call_fn_with_options::<ImmutableString>(
                options,
                &mut self.scope,
                &self.ast,
                "render",
                (self.state,),
            )
            .map_err(|err| {
                println!("rhai render error: {:?}", err);
                RuntimeError::Script
            })?;
        let mut frame = RuntimeFrame::parse(raw.as_str())?;
        self.enrich_wifi_frame(&mut frame);
        if let RuntimeFrame::Material(m) = &mut frame {
            m.bt_status = self.bluetooth_status;
            m.bt_pin = self.bluetooth_pin;
            m.alarm_minutes = clock::alarm_minutes();
            m.clock_alert = clock::alert();
            m.wifi_ip = self.wifi_ip;
            m.timer_editor = self.timer_editor;
            m.timer_duration_secs = clock::timer_duration();
            m.crown_sensitivity = self.crown_sensitivity;
            m.crown_reverse = self.crown_reverse;
            m.high_contrast = self.high_contrast;
            m.has_pin = self.security.has_pin;
            m.security_pending = self.security.encode() != self.security_saved;
            m.security_view = self.security.view;
            m.pin_count = self.security.count;
            m.pin_digit = self.security.digit;
            m.security_message = self.security.message;
            m.lockout_secs = self
                .security
                .remaining(Instant::now().duration_since_epoch().as_secs());
            m.lock_timeout_secs = self.security.timeout;
            m.timeout_is_display = self.timeout_parent == 14;
            let now_ms = Instant::now().duration_since_epoch().as_millis() as u64;
            m.display_brightness = Some(self.idle_brightness(now_ms));
            m.display_sleep = m.display_brightness == Some(0);
            m.transition_alpha =
                (now_ms.saturating_sub(self.transition_started_ms) * 255 / 180).min(255) as u8;
            m.animation_phase = (now_ms % 1000 * 255 / 1000) as u8;
        }
        Ok(frame)
    }

    pub fn state(&self) -> i32 {
        self.state
    }
    fn idle_brightness(&self, now_ms: u64) -> u8 {
        idle::level(
            ((self.state >> 9) & 127) as u8,
            now_ms.saturating_sub(self.last_input_ms),
            self.security.timeout,
            clock::alert() != 0 || self.bluetooth_pin.is_some() || (self.state & 31) == 29,
        )
    }
    fn display_should_sleep(&self, now_ms: u64) -> bool {
        self.idle_brightness(now_ms) == 0
    }
    pub fn crown_sensitivity(&self) -> u8 {
        self.crown_sensitivity
    }
    pub fn preference_config(&self) -> [u8; 4] {
        [
            1,
            self.crown_sensitivity,
            u8::from(self.crown_reverse),
            u8::from(self.high_contrast),
        ]
    }
    pub fn restore_preferences(&mut self, raw: &[u8]) {
        if raw.len() == 4 && raw[0] == 1 && raw[1] <= 2 && raw[2] <= 1 && raw[3] <= 1 {
            self.crown_sensitivity = raw[1];
            self.crown_reverse = raw[2] != 0;
            self.high_contrast = raw[3] != 0;
        }
    }
    pub fn security_saved(&mut self) {
        self.security_saved = self.security.encode();
    }
    pub fn security_config(&self) -> [u8; 58] {
        self.security.encode()
    }
    pub fn restore_security(&mut self, raw: &[u8]) {
        if self
            .security
            .restore(raw, Instant::now().duration_since_epoch().as_secs())
        {
            self.security_saved = self.security.encode();
        } else {
            self.security_storage_error();
        }
    }
    pub fn security_storage_error(&mut self) {
        self.security.has_pin = true;
        self.security.lock();
        self.security.view = 7;
    }
    pub fn set_security_storage(&mut self, available: bool, entropy: [u8; 16]) {
        self.security.storage = available;
        self.security.entropy(entropy);
    }

    pub fn clock_config(&self) -> i32 {
        clock::config()
    }
    pub fn restore_clock_config(&mut self, value: i32) {
        clock::restore_config(value);
    }

    pub fn take_bluetooth_action(&mut self) -> Option<BluetoothUiAction> {
        self.bluetooth_action.take()
    }

    pub fn set_bluetooth_status(&mut self, status: u8, pin: Option<u32>) -> bool {
        let changed = self.bluetooth_status != status || self.bluetooth_pin != pin;
        self.bluetooth_status = status;
        self.bluetooth_pin = pin;
        changed
    }

    pub fn set_wifi_ip(&mut self, ip: Option<[u8; 4]>) -> bool {
        let changed = self.wifi_ip != ip;
        self.wifi_ip = ip;
        changed
    }

    pub fn clear_wifi_scan(&mut self) {
        self.wifi_ui.aps.clear();
        self.wifi_ui.selected = 0;
    }
    pub fn push_wifi_ap(&mut self, ssid: &str, rssi: i8) {
        if self.wifi_ui.aps.len() >= 10 {
            return;
        }
        let mut name = HString::<32>::new();
        for ch in ssid.chars() {
            if name.push(ch).is_err() {
                break;
            }
        }
        if !name.is_empty() {
            let _ = self.wifi_ui.aps.push(WifiUiAp { ssid: name, rssi });
        }
    }
    pub fn set_wifi_link_state(&mut self, state: u8) -> bool {
        let next = state.min(4);
        let changed = self.wifi_ui.link_state != next;
        self.wifi_ui.link_state = next;
        changed
    }
    pub fn take_wifi_ui_action(&mut self) -> Option<WifiUiAction> {
        if self.wifi_ui.pending_scan {
            self.wifi_ui.pending_scan = false;
            return Some(WifiUiAction::Scan);
        }
        if self.wifi_ui.pending_connect {
            self.wifi_ui.pending_connect = false;
            let idx = self.wifi_ui.selected as usize;
            if let Some(ap) = self.wifi_ui.aps.get(idx) {
                return Some(WifiUiAction::Connect {
                    ssid: ap.ssid.clone(),
                    password: self.wifi_ui.password.clone(),
                });
            }
        }
        None
    }
    fn enrich_wifi_frame(&self, frame: &mut RuntimeFrame) {
        let RuntimeFrame::Material(m) = frame else {
            return;
        };
        m.wifi_ap_count = self.wifi_ui.aps.len() as u8;
        m.wifi_ap_index = self.wifi_ui.selected;
        m.wifi_ap_ssid = [0; 32];
        m.wifi_ap_ssid_len = 0;
        m.wifi_ap_rssi = -127;
        if let Some(ap) = self.wifi_ui.aps.get(self.wifi_ui.selected as usize) {
            let bytes = ap.ssid.as_bytes();
            let n = bytes.len().min(32);
            m.wifi_ap_ssid[..n].copy_from_slice(&bytes[..n]);
            m.wifi_ap_ssid_len = n as u8;
            m.wifi_ap_rssi = ap.rssi;
        }
        m.wifi_password_len = self.wifi_ui.password.len() as u8;
        m.wifi_password_preview = [0; 16];
        m.wifi_password_preview_len = 0;
        let password = self.wifi_ui.password.as_bytes();
        let start = password.len().saturating_sub(16);
        let tail = &password[start..];
        m.wifi_password_preview[..tail.len()].copy_from_slice(tail);
        m.wifi_password_preview_len = tail.len() as u8;
        // High two bits carry the keyboard page; low six bits are the selected key.
        // This keeps the viewer protocol compatible while allowing a real full keyboard.
        m.wifi_editor_char = (self.wifi_ui.keyboard_page << 6) | (self.wifi_ui.editor_char & 0x3f);
        m.wifi_link_state = self.wifi_ui.link_state;
    }

    pub fn replace_state(&mut self, state: i32) -> Result<RuntimeFrame, RuntimeError> {
        if state < 0 {
            return Err(RuntimeError::NonZero);
        }
        self.state = state;
        self.render()
    }

    pub fn dispatch(&mut self, event: RuntimeEvent) -> Result<RuntimeFrame, RuntimeError> {
        let now_ms = Instant::now().duration_since_epoch().as_millis() as u64;
        let now = now_ms / 1000;
        if self.security_saved[5] != 0 || self.security.encode() == self.security_saved {
            self.security.tick(now);
        }
        if event != RuntimeEvent::Sync {
            let asleep = self.display_should_sleep(now_ms);
            self.last_input_ms = now_ms;
            self.transition_started_ms = now_ms;
            self.security.activity(now);
            if asleep {
                return self.render();
            }
        }
        let event = match event {
            RuntimeEvent::Rotate(d) if self.crown_reverse => {
                RuntimeEvent::Rotate(d.saturating_neg())
            }
            other => other,
        };
        if self.security.locked {
            // A timer/alarm may be dismissed without unlocking private screens.
            if clock::alert() != 0 && event == RuntimeEvent::Press {
                clock_action(6);
                return self.render();
            }
            self.security.event(event, now);
            if !self.security.locked {
                self.state = (self.state & !0x1ff) | 1;
            }
            return self.render();
        }
        let screen = (self.state & 0x1f) as u8;
        let was_bt_enabled = self.state & (1 << 22) != 0;
        let cursor = ((self.state >> 5) & 15) as u8;
        if screen == 14 {
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + d).rem_euclid(4) << 5)
                }
                RuntimeEvent::Press => match cursor {
                    0 => self.state = (self.state & !0x1ff) | 28,
                    1 => {
                        let next = (((self.state >> 18) & 3) + 1) % 4;
                        self.state = (self.state & !(3 << 18)) | (next << 18);
                    }
                    2 => {
                        self.timeout_parent = 14;
                        self.security.begin(5);
                        self.state = (self.state & !0x1ff) | 20;
                    }
                    _ => self.state = (self.state & !0x1ff) | 5 | (2 << 5),
                },
                RuntimeEvent::Back => self.state = (self.state & !0x1ff) | 5 | (2 << 5),
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
            return self.render();
        }
        if screen == 20 {
            if self.security.view != 0 {
                let was_timeout = self.security.view == 5;
                self.security.event(event, now);
                if was_timeout && self.security.view == 0 && self.timeout_parent == 14 {
                    self.state = (self.state & !0x1ff) | 14 | (2 << 5);
                    self.timeout_parent = 20;
                }
                if event == RuntimeEvent::Home {
                    self.state = (self.state & !0x1ff) | 1;
                }
                return self.render();
            }
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + d).rem_euclid(5) << 5)
                }
                RuntimeEvent::Press => match cursor {
                    0 => self
                        .security
                        .begin(if self.security.has_pin { 4 } else { 1 }),
                    1 => {
                        self.timeout_parent = 20;
                        self.security.begin(5);
                    }
                    2 => {
                        if self.security_saved[5] == 0
                            && self.security.encode() != self.security_saved
                        {
                            self.security.message = 7;
                        } else {
                            self.security.lock();
                        }
                    }
                    3 => {
                        if self.security.has_pin {
                            self.security.begin(6);
                        } else {
                            self.security.message = 6;
                        }
                    }
                    _ => self.state = (self.state & !0x1ff) | 5 | (6 << 5),
                },
                RuntimeEvent::Back => self.state = (self.state & !0x1ff) | 5 | (6 << 5),
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
            return self.render();
        }
        if screen == 18 || screen == 19 {
            let count = if screen == 18 { 3 } else { 2 };
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + d).rem_euclid(count) << 5)
                }
                RuntimeEvent::Press => {
                    if screen == 18 && cursor == 0 {
                        self.crown_sensitivity = (self.crown_sensitivity + 1) % 3;
                    } else if screen == 18 && cursor == 1 {
                        self.crown_reverse = !self.crown_reverse;
                    } else if screen == 19 && cursor == 0 {
                        self.high_contrast = !self.high_contrast;
                    } else {
                        self.state =
                            (self.state & !0x1ff) | 5 | ((if screen == 18 { 4 } else { 5 }) << 5);
                    }
                }
                RuntimeEvent::Back => {
                    self.state =
                        (self.state & !0x1ff) | 5 | ((if screen == 18 { 4 } else { 5 }) << 5)
                }
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
            return self.render();
        }
        if clock::alert() != 0 && event == RuntimeEvent::Press {
            clock_action(6);
            return self.render();
        }
        if screen == 11 && self.timer_editor != 0 {
            match event {
                RuntimeEvent::Rotate(delta) if self.timer_editor == 2 => {
                    clock::set_timer_duration(
                        i32::from(clock::timer_duration()).saturating_add(delta.saturating_mul(60)),
                    );
                }
                RuntimeEvent::Rotate(delta) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + delta).rem_euclid(5) << 5);
                }
                RuntimeEvent::Press => {
                    if self.timer_editor == 2 {
                        clock::start_timer();
                        self.timer_editor = 1;
                    } else if cursor < 4 {
                        clock::set_timer_duration([300, 600, 900, 1800][cursor as usize]);
                        clock::start_timer();
                    } else {
                        self.timer_editor = 2;
                    }
                }
                RuntimeEvent::Back => {
                    self.timer_editor = if self.timer_editor == 2 { 1 } else { 0 };
                    self.state &= !0x1e0;
                }
                RuntimeEvent::Home => {
                    self.state = (self.state & !0x1ff) | 1;
                }
                _ => {}
            }
            return self.render();
        }
        if screen == 11 && cursor == 0 && event == RuntimeEvent::Press {
            self.timer_editor = 1;
            return self.render();
        }
        if screen == 11 && cursor == 2 && event == RuntimeEvent::Press {
            self.state = (self.state & !0x1ff) | 31;
            return self.render();
        }
        if screen == 31 {
            match event {
                RuntimeEvent::Rotate(delta) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + delta).rem_euclid(4) << 5);
                }
                RuntimeEvent::Press => {
                    if cursor < 3 {
                        clock_action([3, 4, 5][cursor as usize]);
                    } else {
                        self.state = (self.state & !0x1ff) | 11 | (2 << 5);
                    }
                }
                RuntimeEvent::Back => {
                    self.state = (self.state & !0x1ff) | 11 | (2 << 5);
                }
                RuntimeEvent::Home => {
                    self.state = (self.state & !0x1ff) | 1;
                }
                _ => {}
            }
            return self.render();
        }
        if screen == 30 {
            match event {
                RuntimeEvent::Rotate(delta) => {
                    let next = (i32::from(cursor) + delta).rem_euclid(3);
                    self.state = (self.state & !0x1e0) | (next << 5);
                }
                RuntimeEvent::Press => {
                    self.bluetooth_action = Some(if self.bluetooth_pin.is_some() {
                        BluetoothUiAction::Confirm(cursor == 0)
                    } else {
                        match cursor {
                            0 => BluetoothUiAction::Pair,
                            1 => BluetoothUiAction::Disconnect,
                            _ => BluetoothUiAction::Forget,
                        }
                    });
                    if cursor == 0 {
                        self.state = (self.state | (1 << 22)) & !(1 << 17);
                    }
                }
                RuntimeEvent::Back | RuntimeEvent::Home => {
                    if self.bluetooth_pin.is_some() {
                        self.bluetooth_action = Some(BluetoothUiAction::Confirm(false));
                    }
                    self.state =
                        (self.state & !0x1ff) | if event == RuntimeEvent::Home { 1 } else { 13 };
                }
                _ => {}
            }
            return self.render();
        }
        if screen == 10 && event == RuntimeEvent::Press {
            if let Some(usage) = [0x00B6, 0x00CD, 0x00B5, 0x00EA, 0x00E9].get(cursor as usize) {
                self.bluetooth_action = Some(BluetoothUiAction::Media(*usage));
                return self.render();
            }
        }
        if screen == 23 || screen == 24 || screen == 25 {
            if self.dispatch_wifi_ui(event) {
                return self.render();
            }
        }
        let (kind, value) = match event {
            RuntimeEvent::Sync => ("sync", 0),
            RuntimeEvent::Home => ("home", 0),
            RuntimeEvent::Back => ("back", 0),
            RuntimeEvent::Rotate(delta) => ("rotate", delta),
            RuntimeEvent::Swipe(direction) => ("swipe", direction),
            RuntimeEvent::Press => ("press", 0),
        };
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let new_state = self
            .engine
            .call_fn_with_options::<i32>(
                options,
                &mut self.scope,
                &self.ast,
                "on_event",
                (self.state, kind.to_string(), value),
            )
            .map_err(|err| {
                println!("rhai event error: {:?}", err);
                RuntimeError::Script
            })?;
        if new_state < 0 {
            return Err(RuntimeError::NonZero);
        }
        let old_screen = screen;
        self.state = new_state;
        if ((new_state & 31) == 30 && old_screen != 30)
            || (event == RuntimeEvent::Press
                && !was_bt_enabled
                && new_state & (1 << 22) != 0
                && ((screen == 13 && cursor == 0) || (screen == 4 && cursor == 1)))
        {
            self.bluetooth_action = Some(BluetoothUiAction::Pair);
        }
        if (new_state & 0x1f) as u8 == 23 && old_screen != 23 {
            self.wifi_ui.pending_scan = true;
            self.wifi_ui.aps.clear();
            self.wifi_ui.selected = 0;
        }
        self.render()
    }

    fn dispatch_wifi_ui(&mut self, event: RuntimeEvent) -> bool {
        let screen = (self.state & 0x1f) as u8;
        let set_sc = |state: i32, sc: u8, cur: u8| -> i32 {
            (state & !0x1ff) | sc as i32 | ((cur as i32) << 5)
        };
        match screen {
            23 => match event {
                RuntimeEvent::Rotate(d) => {
                    let count = self.wifi_ui.aps.len() as i32 + 1; // + Back
                    let cur = ((self.state >> 5) & 0x0f) as i32;
                    let next = (cur + d).rem_euclid(count.max(1));
                    self.state = set_sc(self.state, 23, next as u8);
                    if next < self.wifi_ui.aps.len() as i32 {
                        self.wifi_ui.selected = next as u8;
                    }
                    true
                }
                RuntimeEvent::Press => {
                    let cur = ((self.state >> 5) & 0x0f) as usize;
                    if cur < self.wifi_ui.aps.len() {
                        self.wifi_ui.selected = cur as u8;
                        self.wifi_ui.password.clear();
                        self.wifi_ui.editor_char = 0;
                        self.wifi_ui.keyboard_page = WIFI_KB_LOWER;
                        self.state = set_sc(self.state, 24, 0);
                    } else {
                        self.state = set_sc(self.state, 12, 1);
                    }
                    true
                }
                RuntimeEvent::Swipe(2) | RuntimeEvent::Back => {
                    self.state = set_sc(self.state, 12, 1);
                    true
                }
                RuntimeEvent::Sync => true,
                _ => false,
            },
            24 => match event {
                RuntimeEvent::Rotate(d) => {
                    let total = wifi_keyboard_total(self.wifi_ui.keyboard_page) as i32;
                    self.wifi_ui.editor_char =
                        (self.wifi_ui.editor_char as i32 + d).rem_euclid(total.max(1)) as u8;
                    true
                }
                RuntimeEvent::Press => {
                    let page = self.wifi_ui.keyboard_page;
                    let i = self.wifi_ui.editor_char as usize;
                    let chars = wifi_keyboard_chars(page);
                    if i < chars.len() {
                        if self.wifi_ui.password.len() < 64 {
                            let mut ch = chars[i] as char;
                            if page == WIFI_KB_UPPER {
                                ch = ch.to_ascii_uppercase();
                            }
                            let _ = self.wifi_ui.password.push(ch);
                        }
                    } else {
                        // Six fixed actions are kept on every page so crown
                        // navigation is predictable: page switch, ABC/123,
                        // SPACE, DEL, CONNECT, BACK.
                        let action = i - chars.len();
                        match page {
                            WIFI_KB_LOWER | WIFI_KB_UPPER => match action {
                                0 => {
                                    self.wifi_ui.keyboard_page = if page == WIFI_KB_UPPER {
                                        WIFI_KB_LOWER
                                    } else {
                                        WIFI_KB_UPPER
                                    };
                                    self.wifi_ui.editor_char = 0;
                                }
                                1 => {
                                    self.wifi_ui.keyboard_page = WIFI_KB_SYMBOL_1;
                                    self.wifi_ui.editor_char = 0;
                                }
                                2 => {
                                    if self.wifi_ui.password.len() < 64 {
                                        let _ = self.wifi_ui.password.push(' ');
                                    }
                                }
                                3 => {
                                    let _ = self.wifi_ui.password.pop();
                                }
                                4 => {
                                    self.wifi_ui.pending_connect = true;
                                    self.wifi_ui.link_state = 1;
                                    self.state = set_sc(self.state, 25, 0);
                                }
                                _ => {
                                    self.state = set_sc(self.state, 23, self.wifi_ui.selected);
                                }
                            },
                            WIFI_KB_SYMBOL_1 => match action {
                                0 => {
                                    self.wifi_ui.keyboard_page = WIFI_KB_SYMBOL_2;
                                    self.wifi_ui.editor_char = 0;
                                } // #+=
                                1 => {
                                    self.wifi_ui.keyboard_page = WIFI_KB_LOWER;
                                    self.wifi_ui.editor_char = 0;
                                }
                                2 => {
                                    if self.wifi_ui.password.len() < 64 {
                                        let _ = self.wifi_ui.password.push(' ');
                                    }
                                }
                                3 => {
                                    let _ = self.wifi_ui.password.pop();
                                }
                                4 => {
                                    self.wifi_ui.pending_connect = true;
                                    self.wifi_ui.link_state = 1;
                                    self.state = set_sc(self.state, 25, 0);
                                }
                                _ => {
                                    self.state = set_sc(self.state, 23, self.wifi_ui.selected);
                                }
                            },
                            _ => match action {
                                0 => {
                                    self.wifi_ui.keyboard_page = WIFI_KB_SYMBOL_1;
                                    self.wifi_ui.editor_char = 0;
                                } // 123
                                1 => {
                                    self.wifi_ui.keyboard_page = WIFI_KB_LOWER;
                                    self.wifi_ui.editor_char = 0;
                                }
                                2 => {
                                    if self.wifi_ui.password.len() < 64 {
                                        let _ = self.wifi_ui.password.push(' ');
                                    }
                                }
                                3 => {
                                    let _ = self.wifi_ui.password.pop();
                                }
                                4 => {
                                    self.wifi_ui.pending_connect = true;
                                    self.wifi_ui.link_state = 1;
                                    self.state = set_sc(self.state, 25, 0);
                                }
                                _ => {
                                    self.state = set_sc(self.state, 23, self.wifi_ui.selected);
                                }
                            },
                        }
                    }
                    true
                }
                RuntimeEvent::Swipe(2) | RuntimeEvent::Back => {
                    self.state = set_sc(self.state, 23, self.wifi_ui.selected);
                    true
                }
                RuntimeEvent::Home => {
                    self.state = set_sc(self.state, 1, 0);
                    true
                }
                RuntimeEvent::Sync => true,
                _ => false,
            },
            25 => match event {
                RuntimeEvent::Press | RuntimeEvent::Swipe(2) | RuntimeEvent::Back => {
                    self.state = set_sc(self.state, 12, 1);
                    true
                }
                RuntimeEvent::Home => {
                    self.state = set_sc(self.state, 1, 0);
                    true
                }
                RuntimeEvent::Sync => true,
                _ => false,
            },
            _ => false,
        }
    }
}

#[cfg(not(feature = "rhai-runtime"))]
pub struct Runtime;

#[cfg(not(feature = "rhai-runtime"))]
impl Runtime {
    pub fn start(_script: String) -> Result<(Self, RuntimeFrame), RuntimeError> {
        Ok((
            Self,
            RuntimeFrame::Legacy(LegacyFrame {
                title: "ANDROID".to_string(),
                line1: "RHAI DISABLED".to_string(),
                line2: "".to_string(),
                line3: "".to_string(),
            }),
        ))
    }
    pub fn render(&mut self) -> Result<RuntimeFrame, RuntimeError> {
        Self::start(String::new()).map(|v| v.1)
    }
    pub fn dispatch(&mut self, _event: RuntimeEvent) -> Result<RuntimeFrame, RuntimeError> {
        self.render()
    }
    pub fn crown_sensitivity(&self) -> u8 {
        1
    }
    pub fn preference_config(&self) -> [u8; 4] {
        [1, 1, 0, 0]
    }
    pub fn restore_preferences(&mut self, _raw: &[u8]) {}
    pub fn security_saved(&mut self) {}
    pub fn security_config(&self) -> [u8; 58] {
        [0; 58]
    }
    pub fn restore_security(&mut self, _raw: &[u8]) {}
    pub fn security_storage_error(&mut self) {}
    pub fn set_security_storage(&mut self, _available: bool, _entropy: [u8; 16]) {}
    pub fn state(&self) -> i32 {
        0
    }
    pub fn replace_state(&mut self, _state: i32) -> Result<RuntimeFrame, RuntimeError> {
        self.render()
    }
    pub fn clear_wifi_scan(&mut self) {}
    pub fn push_wifi_ap(&mut self, _ssid: &str, _rssi: i8) {}
    pub fn set_wifi_link_state(&mut self, _state: u8) -> bool {
        false
    }
    pub fn take_wifi_ui_action(&mut self) -> Option<WifiUiAction> {
        None
    }
    pub fn take_bluetooth_action(&mut self) -> Option<BluetoothUiAction> {
        None
    }
    pub fn set_bluetooth_status(&mut self, _status: u8, _pin: Option<u32>) -> bool {
        false
    }
    pub fn clock_config(&self) -> i32 {
        0
    }
    pub fn restore_clock_config(&mut self, _value: i32) {}
    pub fn set_wifi_ip(&mut self, _ip: Option<[u8; 4]>) -> bool {
        false
    }
}
