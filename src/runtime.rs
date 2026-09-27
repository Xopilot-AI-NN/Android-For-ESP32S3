extern crate alloc;

use alloc::{format, string::{String, ToString}};
use heapless::{String as HString, Vec as HVec};
use esp_hal::time::Instant;
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

// Wall-clock time is owned by crate::rtc. Timer/stopwatch use only the
// monotonic ESP counter below and therefore remain independent of wall time.
static STOPWATCH_RUNNING: AtomicBool = AtomicBool::new(false);
static STOPWATCH_START_SECONDS: AtomicI32 = AtomicI32::new(0);
static STOPWATCH_ACCUM_SECONDS: AtomicI32 = AtomicI32::new(0);
static TIMER_RUNNING: AtomicBool = AtomicBool::new(false);
static TIMER_END_SECONDS: AtomicI32 = AtomicI32::new(0);
static ALARM_ENABLED: AtomicBool = AtomicBool::new(false);

fn uptime_seconds() -> i32 {
    (Instant::now().duration_since_epoch().as_secs() & 0x7fff_ffff) as i32
}

fn clock_action(action: i32) -> i32 {
    let now = uptime_seconds();
    match action {
        0 => {
            if STOPWATCH_RUNNING.load(Ordering::Relaxed) {
                let start = STOPWATCH_START_SECONDS.load(Ordering::Relaxed);
                let elapsed = now.saturating_sub(start);
                let accum = STOPWATCH_ACCUM_SECONDS.load(Ordering::Relaxed).saturating_add(elapsed);
                STOPWATCH_ACCUM_SECONDS.store(accum.min(3599), Ordering::Relaxed);
                STOPWATCH_RUNNING.store(false, Ordering::Relaxed);
            } else {
                STOPWATCH_START_SECONDS.store(now, Ordering::Relaxed);
                STOPWATCH_RUNNING.store(true, Ordering::Relaxed);
            }
        }
        1 => {
            STOPWATCH_RUNNING.store(false, Ordering::Relaxed);
            STOPWATCH_ACCUM_SECONDS.store(0, Ordering::Relaxed);
        }
        2 => {
            if TIMER_RUNNING.load(Ordering::Relaxed) {
                TIMER_RUNNING.store(false, Ordering::Relaxed);
            } else {
                TIMER_END_SECONDS.store(now.saturating_add(300), Ordering::Relaxed);
                TIMER_RUNNING.store(true, Ordering::Relaxed);
            }
        }
        3 => {
            let next = !ALARM_ENABLED.load(Ordering::Relaxed);
            ALARM_ENABLED.store(next, Ordering::Relaxed);
        }
        _ => {}
    }
    1
}

fn clock_snapshot() -> (i32, i32, i32) {
    let now = uptime_seconds();
    let mut stopwatch = STOPWATCH_ACCUM_SECONDS.load(Ordering::Relaxed);
    if STOPWATCH_RUNNING.load(Ordering::Relaxed) {
        stopwatch = stopwatch.saturating_add(now.saturating_sub(STOPWATCH_START_SECONDS.load(Ordering::Relaxed)));
    }
    stopwatch = stopwatch.clamp(0, 3599);

    let mut timer = 0;
    if TIMER_RUNNING.load(Ordering::Relaxed) {
        let end = TIMER_END_SECONDS.load(Ordering::Relaxed);
        timer = end.saturating_sub(now);
        if timer <= 0 {
            TIMER_RUNNING.store(false, Ordering::Relaxed);
            timer = 0;
        }
    }
    (timer.clamp(0, 300), stopwatch, if ALARM_ENABLED.load(Ordering::Relaxed) { 1 } else { 0 })
}

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
fn wifi_keyboard_char_count(page: u8) -> usize { wifi_keyboard_chars(page).len() }
fn wifi_keyboard_total(page: u8) -> usize { wifi_keyboard_char_count(page) + 6 }

#[derive(Clone, Debug)]
struct WifiUiAp { ssid: HString<32>, rssi: i8 }

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
    fn new() -> Self { Self { aps: HVec::new(), selected: 0, password: HString::new(), editor_char: 0, keyboard_page: WIFI_KB_LOWER, pending_scan: false, pending_connect: false, link_state: 0 } }
}

pub enum WifiUiAction {
    Scan,
    Connect { ssid: HString<32>, password: HString<64> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeError { Script, NonZero, Protocol }

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
        if let Self::Material(frame) = self { frame.set_swap_pages(pages); }
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
        if parts.next().is_some() { return Err(RuntimeError::Protocol); }
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

        engine.register_fn("boot_version", || 2_i32);
        engine.register_fn("android_api_level", || 37_i32);
        engine.register_fn("runtime_abi", || "aosp-wear-android17-qpr1-rhai-v8-m3e".to_string());
        engine.register_fn("clock_action", |action: i32| clock_action(action));
        engine.register_fn("time_action", |action: i32| crate::rtc::action(action));
        engine.register_fn("m3_scene", |
            screen: i32, cursor: i32, brightness: i32, dnd: i32, airplane: i32,
            theme: i32, locked: i32, wifi: i32, bt: i32, adb: i32, notes: i32
        | {
            // Wall clock advances from the ESP monotonic timer, but is marked invalid
            // until an explicit Wear companion/manual source sets it. USB debug
            // transport never seeds wall time implicitly.
            let mins = crate::rtc::local_minutes();
            let (timer_secs, stopwatch_secs, alarm_enabled) = clock_snapshot();
            format!(
                "M3|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                screen.clamp(0, 31), cursor.clamp(0, 15), brightness.clamp(0, 100),
                if dnd != 0 { 1 } else { 0 }, if airplane != 0 { 1 } else { 0 },
                theme.rem_euclid(4), if locked != 0 { 1 } else { 0 },
                if wifi != 0 { 1 } else { 0 }, if bt != 0 { 1 } else { 0 },
                if adb != 0 { 1 } else { 0 }, notes.clamp(0, 7), mins,
                timer_secs, stopwatch_secs, alarm_enabled,
                if crate::rtc::automatic() { 1 } else { 0 },
                if crate::rtc::valid() { 1 } else { 0 },
                crate::rtc::timezone_hours() as i32
            )
        });
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
        if state < 0 { return Err(RuntimeError::NonZero); }

        let mut runtime = Self { engine, scope, ast, state, wifi_ui: WifiUi::new() };
        let frame = runtime.render()?;
        Ok((runtime, frame))
    }

    pub fn render(&mut self) -> Result<RuntimeFrame, RuntimeError> {
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let raw = self.engine
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
        Ok(frame)
    }

    pub fn state(&self) -> i32 { self.state }

    pub fn clear_wifi_scan(&mut self) { self.wifi_ui.aps.clear(); self.wifi_ui.selected = 0; }
    pub fn push_wifi_ap(&mut self, ssid: &str, rssi: i8) {
        if self.wifi_ui.aps.len() >= 10 { return; }
        let mut name = HString::<32>::new();
        for ch in ssid.chars() {
            if ch.is_ascii() { let _ = name.push(ch); }
            if name.len() >= 32 { break; }
        }
        if !name.is_empty() { let _ = self.wifi_ui.aps.push(WifiUiAp { ssid: name, rssi }); }
    }
    pub fn set_wifi_link_state(&mut self, state: u8) -> bool { let next=state.min(4); let changed=self.wifi_ui.link_state!=next; self.wifi_ui.link_state=next; changed }
    pub fn take_wifi_ui_action(&mut self) -> Option<WifiUiAction> {
        if self.wifi_ui.pending_scan { self.wifi_ui.pending_scan = false; return Some(WifiUiAction::Scan); }
        if self.wifi_ui.pending_connect {
            self.wifi_ui.pending_connect = false;
            let idx = self.wifi_ui.selected as usize;
            if let Some(ap) = self.wifi_ui.aps.get(idx) {
                return Some(WifiUiAction::Connect { ssid: ap.ssid.clone(), password: self.wifi_ui.password.clone() });
            }
        }
        None
    }
    fn enrich_wifi_frame(&self, frame: &mut RuntimeFrame) {
        let RuntimeFrame::Material(m) = frame else { return; };
        m.wifi_ap_count = self.wifi_ui.aps.len() as u8;
        m.wifi_ap_index = self.wifi_ui.selected;
        m.wifi_ap_ssid = [0; 32]; m.wifi_ap_ssid_len = 0; m.wifi_ap_rssi = -127;
        if let Some(ap) = self.wifi_ui.aps.get(self.wifi_ui.selected as usize) {
            let bytes = ap.ssid.as_bytes(); let n = bytes.len().min(32);
            m.wifi_ap_ssid[..n].copy_from_slice(&bytes[..n]); m.wifi_ap_ssid_len = n as u8; m.wifi_ap_rssi = ap.rssi;
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
        if state < 0 { return Err(RuntimeError::NonZero); }
        self.state = state;
        self.render()
    }

    pub fn dispatch(&mut self, event: RuntimeEvent) -> Result<RuntimeFrame, RuntimeError> {
        let screen = (self.state & 0x1f) as u8;
        if screen == 23 || screen == 24 || screen == 25 {
            if self.dispatch_wifi_ui(event) { return self.render(); }
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
        let new_state = self.engine
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
        if new_state < 0 { return Err(RuntimeError::NonZero); }
        let old_screen = screen;
        self.state = new_state;
        if (new_state & 0x1f) as u8 == 23 && old_screen != 23 {
            self.wifi_ui.pending_scan = true;
            self.wifi_ui.aps.clear();
            self.wifi_ui.selected = 0;
        }
        self.render()
    }

    fn dispatch_wifi_ui(&mut self, event: RuntimeEvent) -> bool {
        let screen = (self.state & 0x1f) as u8;
        let set_sc = |state: i32, sc: u8, cur: u8| -> i32 { (state & !0x1ff) | sc as i32 | ((cur as i32) << 5) };
        match screen {
            23 => match event {
                RuntimeEvent::Rotate(d) => {
                    let count = self.wifi_ui.aps.len() as i32 + 1; // + Back
                    let cur = ((self.state >> 5) & 0x0f) as i32;
                    let next = (cur + d).rem_euclid(count.max(1));
                    self.state = set_sc(self.state, 23, next as u8);
                    if next < self.wifi_ui.aps.len() as i32 { self.wifi_ui.selected = next as u8; }
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
                    } else { self.state = set_sc(self.state, 12, 1); }
                    true
                }
                RuntimeEvent::Swipe(2) | RuntimeEvent::Back => { self.state = set_sc(self.state, 12, 1); true }
                RuntimeEvent::Sync => true,
                _ => false,
            },
            24 => match event {
                RuntimeEvent::Rotate(d) => {
                    let total = wifi_keyboard_total(self.wifi_ui.keyboard_page) as i32;
                    self.wifi_ui.editor_char = (self.wifi_ui.editor_char as i32 + d).rem_euclid(total.max(1)) as u8;
                    true
                }
                RuntimeEvent::Press => {
                    let page = self.wifi_ui.keyboard_page;
                    let i = self.wifi_ui.editor_char as usize;
                    let chars = wifi_keyboard_chars(page);
                    if i < chars.len() {
                        if self.wifi_ui.password.len() < 64 {
                            let mut ch = chars[i] as char;
                            if page == WIFI_KB_UPPER { ch = ch.to_ascii_uppercase(); }
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
                                    self.wifi_ui.keyboard_page = if page == WIFI_KB_UPPER { WIFI_KB_LOWER } else { WIFI_KB_UPPER };
                                    self.wifi_ui.editor_char = 0;
                                }
                                1 => { self.wifi_ui.keyboard_page = WIFI_KB_SYMBOL_1; self.wifi_ui.editor_char = 0; }
                                2 => { if self.wifi_ui.password.len() < 64 { let _ = self.wifi_ui.password.push(' '); } }
                                3 => { let _ = self.wifi_ui.password.pop(); }
                                4 => { self.wifi_ui.pending_connect = true; self.wifi_ui.link_state = 1; self.state = set_sc(self.state,25,0); }
                                _ => { self.state = set_sc(self.state,23,self.wifi_ui.selected); }
                            },
                            WIFI_KB_SYMBOL_1 => match action {
                                0 => { self.wifi_ui.keyboard_page = WIFI_KB_SYMBOL_2; self.wifi_ui.editor_char = 0; } // #+=
                                1 => { self.wifi_ui.keyboard_page = WIFI_KB_LOWER; self.wifi_ui.editor_char = 0; }
                                2 => { if self.wifi_ui.password.len() < 64 { let _ = self.wifi_ui.password.push(' '); } }
                                3 => { let _ = self.wifi_ui.password.pop(); }
                                4 => { self.wifi_ui.pending_connect = true; self.wifi_ui.link_state = 1; self.state = set_sc(self.state,25,0); }
                                _ => { self.state = set_sc(self.state,23,self.wifi_ui.selected); }
                            },
                            _ => match action {
                                0 => { self.wifi_ui.keyboard_page = WIFI_KB_SYMBOL_1; self.wifi_ui.editor_char = 0; } // 123
                                1 => { self.wifi_ui.keyboard_page = WIFI_KB_LOWER; self.wifi_ui.editor_char = 0; }
                                2 => { if self.wifi_ui.password.len() < 64 { let _ = self.wifi_ui.password.push(' '); } }
                                3 => { let _ = self.wifi_ui.password.pop(); }
                                4 => { self.wifi_ui.pending_connect = true; self.wifi_ui.link_state = 1; self.state = set_sc(self.state,25,0); }
                                _ => { self.state = set_sc(self.state,23,self.wifi_ui.selected); }
                            },
                        }
                    }
                    true
                }
                RuntimeEvent::Swipe(2) | RuntimeEvent::Back => { self.state = set_sc(self.state,23,self.wifi_ui.selected); true }
                RuntimeEvent::Home => { self.state = set_sc(self.state,1,0); true }
                RuntimeEvent::Sync => true,
                _ => false,
            },
            25 => match event {
                RuntimeEvent::Press | RuntimeEvent::Swipe(2) | RuntimeEvent::Back => { self.state = set_sc(self.state,12,1); true }
                RuntimeEvent::Home => { self.state = set_sc(self.state,1,0); true }
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
        Ok((Self, RuntimeFrame::Legacy(LegacyFrame {
            title: "ANDROID".to_string(),
            line1: "RHAI DISABLED".to_string(),
            line2: "".to_string(),
            line3: "".to_string(),
        })))
    }
    pub fn render(&mut self) -> Result<RuntimeFrame, RuntimeError> {
        Self::start(String::new()).map(|v| v.1)
    }
    pub fn dispatch(&mut self, _event: RuntimeEvent) -> Result<RuntimeFrame, RuntimeError> {
        self.render()
    }
    pub fn state(&self) -> i32 { 0 }
    pub fn replace_state(&mut self, _state: i32) -> Result<RuntimeFrame, RuntimeError> { self.render() }
    pub fn clear_wifi_scan(&mut self) {}
    pub fn push_wifi_ap(&mut self, _ssid: &str, _rssi: i8) {}
    pub fn set_wifi_link_state(&mut self, _state: u8) -> bool { false }
    pub fn take_wifi_ui_action(&mut self) -> Option<WifiUiAction> { None }
}
