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
#[path = "runtime/scene_cache.rs"]
mod scene_cache;
#[path = "runtime/security.rs"]
mod security;
use clock::{clock_action, clock_snapshot};
pub use scene_cache::ScriptStats;

use crate::material::MaterialFrame;

#[cfg(feature = "rhai-runtime")]
use crate::watch_println as println;
#[cfg(feature = "rhai-runtime")]
use rhai::{AST, CallFnOptions, Engine, ImmutableString, Scope};

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
    keyboard: crate::text_input::Keyboard,
    keyboard_error: u8,
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
            keyboard: crate::text_input::Keyboard::english(),
            keyboard_error: 0,
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
    Power,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SystemAction {
    Reboot = 1,
    Fastboot = 2,
    Recovery = 3,
    PowerOff = 4,
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
    name_editor: crate::device_name::Editor,
    scene_cache: scene_cache::SceneCache,
    engine: Engine,
    scope: Scope<'static>,
    ast: AST,
    /// Firmware-owned persistent state.  Rhai functions are intentionally
    /// pure with respect to outer Scope variables, so state crosses the ABI
    /// explicitly instead of relying on closure/global-variable semantics.
    state: i32,
    native_app: crate::apps::State,
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
    watch_face: u8,
    launcher_grid: bool,
    aod_enabled: bool,
    wallpaper_index: u8,
    wallpaper_level: u8,
    show_seconds: bool,
    wallpaper_enabled: bool,
    has_wallpaper: bool,
    playback: Option<crate::media::Playback>,
    power_menu: bool,
    power_cursor: u8,
    system_action: Option<SystemAction>,
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
            name_editor: crate::device_name::Editor::default(),
            scene_cache: scene_cache::SceneCache::default(),
            engine,
            scope,
            ast,
            state,
            native_app: crate::apps::State::default(),
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
            watch_face: 0,
            launcher_grid: false,
            aod_enabled: false,
            wallpaper_index: 1,
            wallpaper_level: 85,
            show_seconds: true,
            wallpaper_enabled: true,
            has_wallpaper: true,
            playback: None,
            power_menu: false,
            power_cursor: 0,
            system_action: None,
        };
        let frame = runtime.render()?;
        Ok((runtime, frame))
    }

    pub fn render(&mut self) -> Result<RuntimeFrame, RuntimeError> {
        if matches!(self.state & 31, 2 | 26) {
            self.state = (self.state & !31) | if self.launcher_grid { 26 } else { 2 };
        }
        if self.security_saved[5] != 0 || self.security.encode() == self.security_saved {
            self.security
                .tick(Instant::now().duration_since_epoch().as_secs());
        }
        if self.security.locked {
            self.name_editor.active = false;
            self.native_app.close();
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
        let mut frame =
            self.scene_cache
                .render(&self.engine, &mut self.scope, &self.ast, self.state)?;
        self.enrich_wifi_frame(&mut frame);
        if let RuntimeFrame::Material(m) = &mut frame {
            let name = crate::device_name::snapshot();
            m.device_name_len = name.len() as u8;
            m.device_name[..name.len()].copy_from_slice(name.as_bytes());
            m.resource_revision = crate::ui_resources::revision();
            m.app = self.native_app.view();
            if m.app.id == crate::apps::DIAGNOSTICS {
                m.app.uptime = Instant::now().duration_since_epoch().as_secs();
                m.app.free_kib = (esp_alloc::HEAP.free() / 1024) as u32;
                m.app.internal_kib = (esp_alloc::HEAP
                    .free_caps(esp_alloc::MemoryCapability::Internal.into())
                    / 1024) as u32;
                m.app.script_us = self.scene_cache.stats.last_us;
                m.app.cache_hits = self.scene_cache.stats.cache_hits;
            }
            m.name_editing = self.name_editor.active;
            if m.name_editing {
                m.keyboard = self.name_editor.keyboard.view;
                m.keyboard_error = self.name_editor.error;
            }
            m.name_draft_len = self.name_editor.draft.len() as u8;
            m.name_draft[..self.name_editor.draft.len()]
                .copy_from_slice(self.name_editor.draft.as_bytes());
            // The cached ABI scene describes state, never frozen clock data.
            let (timer, stopwatch, alarm) = clock_snapshot();
            m.time_minutes = crate::rtc::local_minutes() as u16;
            m.timer_secs = timer as u16;
            m.stopwatch_secs = stopwatch as u16;
            m.alarm_enabled = alarm != 0;
            m.auto_time = crate::rtc::automatic();
            m.time_valid = crate::rtc::valid();
            m.timezone_hours = crate::rtc::timezone_hours();
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
            m.aod_enabled = self.aod_enabled;
            m.aod = self.aod_enabled && m.display_sleep;
            if m.aod {
                m.display_sleep = false;
                m.display_brightness = Some(5);
            }
            m.wallpaper_level = self.wallpaper_level;
            m.show_seconds = self.show_seconds;
            m.wallpaper_index = self.wallpaper_index;
            // Prefer immediate updates on this MCU over full-screen fades.
            m.transition_alpha = 255;
            m.watch_face = self.watch_face;
            m.wallpaper_enabled = self.wallpaper_enabled;
            m.wallpaper_available = self.has_wallpaper;
            m.stopwatch_running = clock::stopwatch_running();
            m.power_menu = self.power_menu;
            if self.power_menu {
                m.cursor = self.power_cursor;
            }
            m.time_seconds = crate::rtc::local_seconds();
            if let Some((day, month)) = crate::rtc::calendar() {
                m.date_day = day;
                m.date_month = month;
            }
            if matches!(self.bluetooth_status, 2 | 3) {
                if let Some((playing, progress)) = self.playback.and_then(|p| p.progress(now_ms)) {
                    m.media_known = true;
                    m.media_playing = playing;
                    m.media_progress = progress;
                }
            }
            // No decorative motion; the analog second hand still uses RTC
            // seconds and real media progress remains live.
            m.animation_phase = 0;
        }
        Ok(frame)
    }

    pub fn state(&self) -> i32 {
        self.state
    }
    pub fn diagnostics_active(&self) -> bool {
        self.native_app.view().id == crate::apps::DIAGNOSTICS
    }

    pub fn script_stats(&self) -> ScriptStats {
        self.scene_cache.stats
    }
    pub fn next_background_refresh_ms(&self, now_ms: u64) -> u64 {
        idle::next_refresh(now_ms, self.last_input_ms, self.security.timeout)
    }
    pub fn take_system_action(&mut self) -> Option<SystemAction> {
        self.system_action.take()
    }
    pub fn is_locked(&self) -> bool {
        self.security.locked
    }
    pub fn request_system_action(&mut self, action: SystemAction) -> bool {
        if self.is_locked() {
            return false;
        }
        self.system_action = Some(action);
        true
    }
    pub fn wallpaper_available(&mut self, available: bool) {
        self.has_wallpaper = available || crate::wallpaper::builtin_count() != 0;
    }
    pub fn use_uploaded_wallpaper(&mut self) {
        self.wallpaper_index = 0;
        self.wallpaper_enabled = true;
        self.has_wallpaper = true;
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
    pub fn crown_reversed(&self) -> bool {
        self.crown_reverse
    }
    pub fn preference_config(&self) -> [u8; 12] {
        [
            4,
            self.crown_sensitivity,
            u8::from(self.crown_reverse),
            u8::from(self.high_contrast),
            self.watch_face,
            u8::from(self.wallpaper_enabled),
            u8::from(self.launcher_grid),
            u8::from(self.aod_enabled),
            self.wallpaper_index,
            self.wallpaper_level,
            u8::from(self.show_seconds),
            0,
        ]
    }
    pub fn restore_preferences(&mut self, raw: &[u8]) {
        if ((raw.len() == 4 && raw[0] == 1)
            || (raw.len() == 8 && raw[0] == 2)
            || (raw.len() == 12 && matches!(raw[0], 3 | 4)))
            && raw[1] <= 2
            && raw[2] <= 1
            && raw[3] <= 1
        {
            self.crown_sensitivity = raw[1];
            self.crown_reverse = raw[2] != 0;
            self.high_contrast = raw[3] != 0;
            if raw.len() >= 8 {
                self.watch_face = raw[4].min(2);
                self.wallpaper_enabled = raw[5] != 0;
            }
            if raw.len() == 12 {
                self.launcher_grid = raw[6] != 0;
                self.aod_enabled = raw[7] != 0;
                self.wallpaper_index = raw[8].min(crate::wallpaper::builtin_count());
                if raw[0] == 4 {
                    self.wallpaper_level = raw[9].clamp(40, 100);
                    self.show_seconds = raw[10] != 0;
                }
            }
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

    pub fn set_playback(&mut self, playback: crate::media::Playback) {
        self.playback = Some(playback);
    }
    pub fn set_bluetooth_status(&mut self, status: u8, pin: Option<u32>) -> bool {
        let changed = self.bluetooth_status != status || self.bluetooth_pin != pin;
        self.bluetooth_status = status;
        if !matches!(status, 2 | 3) {
            self.playback = None;
        }
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
        let mut start = password.len().saturating_sub(16);
        while !self.wifi_ui.password.is_char_boundary(start) {
            start += 1;
        }
        let tail = &password[start..];
        m.wifi_password_preview[..tail.len()].copy_from_slice(tail);
        m.wifi_password_preview_len = tail.len() as u8;
        // High two bits carry the keyboard page; low six bits are the selected key.
        // This keeps the viewer protocol compatible while allowing a real full keyboard.
        let keyboard = self.wifi_ui.keyboard.view;
        let legacy_page = if keyboard.page == 0 {
            u8::from(keyboard.upper)
        } else {
            keyboard.page + 1
        };
        m.wifi_editor_char = (legacy_page << 6) | (keyboard.focus & 0x3f);
        m.keyboard = keyboard;
        m.keyboard_error = self.wifi_ui.keyboard_error;
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
        if event == RuntimeEvent::Power {
            self.power_menu = !self.power_menu;
            self.power_cursor = 0;
            self.last_input_ms = now_ms;
            return self.render();
        }
        if self.power_menu {
            self.last_input_ms = now_ms;
            let count = if self.security.locked { 3 } else { 6 };
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.power_cursor = (i32::from(self.power_cursor) + d).rem_euclid(count) as u8
                }
                RuntimeEvent::Press => {
                    self.system_action = match self.power_cursor {
                        0 => Some(SystemAction::PowerOff),
                        1 => Some(SystemAction::Reboot),
                        3 if !self.security.locked => Some(SystemAction::Recovery),
                        4 if !self.security.locked => Some(SystemAction::Fastboot),
                        _ => None,
                    };
                    if self.power_cursor == 2 && !self.security.locked {
                        self.native_app.close();
                        self.state = (self.state & !0x1ff) | 1;
                    }
                    self.power_menu = false;
                }
                RuntimeEvent::Back | RuntimeEvent::Home => self.power_menu = false,
                _ => {}
            }
            return self.render();
        }
        if self.security_saved[5] != 0 || self.security.encode() == self.security_saved {
            self.security.tick(now);
        }
        if event != RuntimeEvent::Sync {
            let asleep = self.display_should_sleep(now_ms);
            self.last_input_ms = now_ms;
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
        // Native applications share one bounded router; the old SD script ABI stays intact.
        if self.native_app.active() {
            if matches!(
                event,
                RuntimeEvent::Home | RuntimeEvent::Back | RuntimeEvent::Swipe(2)
            ) {
                self.native_app.close();
                if event == RuntimeEvent::Home {
                    self.state = (self.state & !0x1ff) | 1;
                }
            } else if let Some(index) = self.native_app.event(event) {
                self.wallpaper_index = index;
                self.wallpaper_enabled = true;
            }
            return self.render();
        }
        if matches!(screen, 2 | 26) {
            match event {
                RuntimeEvent::Rotate(delta) => {
                    let count = i32::from(crate::apps::COUNT) + 1;
                    self.state = (self.state & !0x1e0)
                        | ((i32::from(cursor) + delta.rem_euclid(count)).rem_euclid(count) << 5);
                }
                RuntimeEvent::Swipe(direction @ (-1 | 1)) => {
                    let step = if screen == 26 { 3 } else { 1 };
                    let next = (i32::from(cursor) - direction * step)
                        .clamp(0, i32::from(crate::apps::COUNT));
                    self.state = (self.state & !0x1e0) | (next << 5);
                }
                RuntimeEvent::Press if cursor == crate::apps::COUNT => {
                    self.launcher_grid = screen == 2;
                    self.state = (self.state & !0x1ff) | if self.launcher_grid { 26 } else { 2 };
                }
                RuntimeEvent::Press => {
                    if let Some(route) = crate::apps::route(cursor) {
                        self.state = (self.state & !0x1ff) | i32::from(route);
                    } else {
                        self.native_app.open(cursor);
                    }
                }
                RuntimeEvent::Home | RuntimeEvent::Back | RuntimeEvent::Swipe(2) => {
                    self.state = (self.state & !0x1ff) | 1
                }
                _ => {}
            }
            return self.render();
        }
        if self.name_editor.active {
            match event {
                RuntimeEvent::Rotate(delta) => self.name_editor.rotate(delta),
                RuntimeEvent::Press => self.name_editor.press(),
                RuntimeEvent::Back | RuntimeEvent::Home => {
                    self.name_editor.active = false;
                    if event == RuntimeEvent::Home {
                        self.state = (self.state & !0x1ff) | 1;
                    }
                }
                RuntimeEvent::Swipe(2) => self.name_editor.active = false,
                _ => {}
            }
            return self.render();
        }
        if screen == 15 {
            match event {
                RuntimeEvent::Rotate(delta) => {
                    self.state = (self.state & !0x1e0)
                        | ((i32::from(cursor) + delta.rem_euclid(7)).rem_euclid(7) << 5)
                }
                RuntimeEvent::Press => match cursor {
                    0 => self.state = (self.state & !0x1ff) | 16,
                    1 => self.state = (self.state & !0x1ff) | 8,
                    2 => self.state = (self.state & !0x1ff) | 22,
                    3 => self.name_editor.begin(),
                    4 => self.native_app.open(crate::apps::DIAGNOSTICS),
                    5 => {
                        self.power_menu = true;
                        self.power_cursor = 0;
                    }
                    _ => self.state = (self.state & !0x1ff) | 5 | (7 << 5),
                },
                RuntimeEvent::Back => self.state = (self.state & !0x1ff) | 5 | (7 << 5),
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
            return self.render();
        }
        if screen == 14 {
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.state = (self.state & !0x1e0)
                        | ((i32::from(cursor) + d.rem_euclid(10)).rem_euclid(10) << 5)
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
                    3 => self.watch_face = (self.watch_face + 1) % 3,
                    4 => self.wallpaper_enabled = !self.wallpaper_enabled,
                    5 => {
                        self.wallpaper_index =
                            (self.wallpaper_index + 1) % (crate::wallpaper::builtin_count() + 1);
                        self.wallpaper_enabled = true;
                    }
                    6 => self.aod_enabled = !self.aod_enabled,
                    7 => {
                        self.wallpaper_level = match self.wallpaper_level {
                            0..=59 => 60,
                            60..=74 => 75,
                            75..=84 => 85,
                            85..=99 => 100,
                            _ => 40,
                        }
                    }
                    8 => self.show_seconds = !self.show_seconds,
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
        if screen == 6 && event == RuntimeEvent::Press {
            if cursor == 0 {
                self.state = (self.state & !0x1ff) | 11;
                self.timer_editor = 1;
            } else if cursor == 1 {
                self.state = (self.state & !0x1ff) | 11;
                self.timer_editor = 3;
            } else if cursor == 2 {
                self.state = (self.state & !0x1ff) | 31;
            } else {
                self.state = (self.state & !0x1ff) | 2;
            }
            return self.render();
        }
        if screen == 11 && self.timer_editor == 3 {
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + d).rem_euclid(3) << 5)
                }
                RuntimeEvent::Press => match cursor {
                    0 => {
                        clock_action(0);
                    }
                    1 => {
                        clock_action(1);
                    }
                    _ => {
                        self.state = (self.state & !0x1ff) | 6 | (1 << 5);
                    }
                },
                RuntimeEvent::Back => self.state = (self.state & !0x1ff) | 6 | (1 << 5),
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
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
                    if self.timer_editor == 0 {
                        self.state = (self.state & !0x1ff) | 6;
                    }
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
                        self.state = (self.state & !0x1ff) | 6 | (2 << 5);
                    }
                }
                RuntimeEvent::Back => {
                    self.state = (self.state & !0x1ff) | 6 | (2 << 5);
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
        if screen == 27 && event == RuntimeEvent::Press {
            if self.bluetooth_status == 3 {
                self.bluetooth_action = Some(BluetoothUiAction::Media(0x00CF));
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
            RuntimeEvent::Power => unreachable!(),
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
        if old_screen == 9 && (new_state & 31) == 11 {
            self.timer_editor = if ((new_state >> 5) & 15) == 1 { 3 } else { 1 };
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
                        self.wifi_ui.keyboard = crate::text_input::Keyboard::english();
                        self.wifi_ui.keyboard_error = 0;
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
                RuntimeEvent::Rotate(delta) => {
                    self.wifi_ui.keyboard.rotate(delta);
                    true
                }
                RuntimeEvent::Press => {
                    use crate::text_input::Action;
                    self.wifi_ui.keyboard_error = 0;
                    match self.wifi_ui.keyboard.press() {
                        Action::Character(ch) => {
                            if self.wifi_ui.password.push(ch).is_err() {
                                self.wifi_ui.keyboard_error = 1;
                            }
                        }
                        Action::Backspace => {
                            self.wifi_ui.password.pop();
                        }
                        Action::Submit => {
                            self.wifi_ui.pending_connect = true;
                            self.wifi_ui.link_state = 1;
                            self.state = set_sc(self.state, 25, 0);
                        }
                        Action::Cancel => {
                            self.state = set_sc(self.state, 23, self.wifi_ui.selected)
                        }
                        Action::None => {}
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
    pub fn diagnostics_active(&self) -> bool {
        false
    }
    pub fn script_stats(&self) -> ScriptStats {
        ScriptStats::default()
    }
    pub fn next_background_refresh_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_add(1000)
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
    pub fn crown_reversed(&self) -> bool {
        false
    }
    pub fn preference_config(&self) -> [u8; 12] {
        [3, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0]
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
    pub fn take_system_action(&mut self) -> Option<SystemAction> {
        None
    }
    pub fn is_locked(&self) -> bool {
        false
    }
    pub fn request_system_action(&mut self, _action: SystemAction) -> bool {
        false
    }
    pub fn wallpaper_available(&mut self, _available: bool) {}
    pub fn use_uploaded_wallpaper(&mut self) {}
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
    pub fn set_playback(&mut self, _playback: crate::media::Playback) {}
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
