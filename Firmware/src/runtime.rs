// Модуль Rhai runtime и нативного интерфейса.
extern crate alloc;

use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
};
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use esp_hal::time::Instant;
use heapless::{String as HString, Vec as HVec};

#[path = "../../App/system/app/Clock/controller.rs"]
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
/// Структура данных компонента Rhai runtime и нативного интерфейса.
struct WifiUiAp {
    /// Имя Wi-Fi-сети.
    ssid: HString<32>,
    /// Измеренный уровень сигнала, dBm.
    rssi: i8,
}

#[derive(Clone, Debug)]
/// Структура данных компонента Rhai runtime и нативного интерфейса.
struct WifiUi {
    /// Список результатов сканирования Wi-Fi.
    aps: HVec<WifiUiAp, 10>,
    /// Выбранный элемент либо признак его выделения.
    selected: u8,
    /// Ограниченный буфер пароля сети; не является системным именем.
    password: HString<64>,
    /// Состояние общей RU/EN-клавиатуры.
    keyboard: crate::text_input::Keyboard,
    /// Код ошибки проверки пользовательского ввода.
    keyboard_error: u8,
    /// Необработанный запрос Wi-Fi-сканирования.
    pending_scan: bool,
    /// Запрос подключения с введённым профилем.
    pending_connect: bool,
    /// Этап подключения сети.
    link_state: u8,
}
impl WifiUi {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
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

/// Структура данных компонента Rhai runtime и нативного интерфейса.
pub enum WifiUiAction {
    Scan,
    Connect {
        ssid: HString<32>,
        password: HString<64>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Структура данных компонента Rhai runtime и нативного интерфейса.
pub enum BluetoothUiAction {
    Pair,
    Confirm(bool),
    Disconnect,
    Forget,
    Media(u16),
    Keyboard([u8; 8]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Ошибки операций компонента Rhai runtime и нативного интерфейса.
pub enum RuntimeError {
    Script,
    NonZero,
    Protocol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Типизированное событие ввода или фонового обновления компонента Rhai runtime и нативного интерфейса.
pub enum RuntimeEvent {
    Sync,
    Home,
    Back,
    Rotate(i32),
    Swipe(i32),
    Press,
    Click(u8),
    Power,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
/// Отложенное аппаратное действие питания компонента Rhai runtime и нативного интерфейса.
pub enum SystemAction {
    Reboot = 1,
    Fastboot = 2,
    Recovery = 3,
    PowerOff = 4,
}

#[derive(Clone, Debug)]
/// Структура данных компонента Rhai runtime и нативного интерфейса.
pub struct LegacyFrame {
    /// Заголовок текущего экрана или документа.
    pub title: String,
    pub line1: String,
    pub line2: String,
    pub line3: String,
}

#[derive(Clone, Debug)]
/// Кадр Material либо резервного текстового интерфейса компонента Rhai runtime и нативного интерфейса.
pub enum RuntimeFrame {
    Material(alloc::boxed::Box<MaterialFrame>),
    Legacy(LegacyFrame),
}

impl RuntimeFrame {
    /// Обновляет отображаемое число сохранённых страниц состояния.
    pub fn set_swap_pages(&mut self, pages: u8) {
        if let Self::Material(frame) = self {
            frame.set_swap_pages(pages);
        }
    }

    /// Разбирает и проверяет входное представление данных.
    fn parse(raw: &str) -> Result<Self, RuntimeError> {
        if let Some(frame) = MaterialFrame::parse(raw) {
            return Ok(Self::Material(alloc::boxed::Box::new(frame)));
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
/// Владелец Rhai-движка и нативного состояния интерфейса компонента Rhai runtime и нативного интерфейса.
pub struct Runtime {
    /// Контроллер черновика редактируемого имени устройства.
    name_editor: crate::device_name::Editor,
    /// Кэш последнего кадра, зависящего от скриптового состояния.
    scene_cache: scene_cache::SceneCache,
    /// Rhai-движок с зарегистрированными нативными функциями.
    engine: Engine,
    /// Область переменных Rhai, используемая при вызовах скрипта.
    scope: Scope<'static>,
    /// Скомпилированное дерево Rhai-скрипта.
    ast: AST,
    /// Firmware-owned persistent state.  Rhai functions are intentionally
    /// pure with respect to outer Scope variables, so state crosses the ABI
    /// explicitly instead of relying on closure/global-variable semantics.
    state: i32,
    /// Контроллер нативных приложений поверх старого ABI экранов.
    native_app: Box<crate::apps::State>,
    /// Состояние списка сетей, пароля и ожидающих действий Wi-Fi.
    wifi_ui: WifiUi,
    flashlight_since_secs: Option<u64>,
    /// Команда BLE, ожидающая выполнения аппаратным слоем.
    bluetooth_action: Option<BluetoothUiAction>,
    /// Код состояния BLE для интерфейса.
    bluetooth_status: u8,
    /// Код ожидающего подтверждения BLE, если он имеется.
    bluetooth_pin: Option<u32>,
    /// Текущий IP-адрес Wi-Fi либо отсутствие адреса.
    wifi_ip: Option<[u8; 4]>,
    /// Подрежим выбора длительности таймера/секундомера.
    timer_editor: u8,
    bedtime_override: Option<bool>,
    bedtime_previous_auto: bool,
    bedtime_auto: bool,
    bedtime_start: u16,
    bedtime_end: u16,
    bedtime_brightness: u8,
    bedtime_editor: u8,
    bedtime_cursor: u8,
    /// Локальный PIN-гейт и защитные сроки.
    security: security::Security,
    /// Последняя подтверждённая запись параметров PIN.
    security_saved: [u8; 58],
    /// Выбранный уровень чувствительности колёсика.
    crown_sensitivity: u8,
    /// Признак обратного направления колёсика.
    crown_reverse: bool,
    /// Признак повышенной контрастности интерфейса.
    high_contrast: bool,
    /// Монотонное время/интервал last_input в миллисекундах.
    last_input_ms: u64,
    timeout_parent: u8,
    /// Номер выбранного сохраняемого циферблата.
    watch_face: u8,
    /// Выбор сетки вместо списка приложений.
    launcher_grid: bool,
    /// Полный индекс меню, не ограниченный четырьмя битами старого ABI.
    launcher_cursor: u8,
    /// Пользовательское разрешение режима Always-on Display.
    aod_enabled: bool,
    /// Номер обоев; ноль соответствует пользовательскому изображению.
    wallpaper_index: u8,
    /// Масштаб яркости обоев, проценты.
    wallpaper_level: u8,
    /// Разрешение показа секунд на поддерживающих их циферблатах.
    show_seconds: bool,
    /// Разрешение отображать выбранные обои.
    wallpaper_enabled: bool,
    /// Признак подготовленных пользовательских обоев.
    has_wallpaper: bool,
    /// Последнее реальное состояние медиаплеера.
    playback: Option<crate::media::Playback>,
    /// Признак открытого меню питания.
    power_menu: bool,
    /// Выбранный пункт меню питания.
    power_cursor: u8,
    /// Отложенная аппаратная команда, которую выполняет основной цикл.
    system_action: Option<SystemAction>,
}

#[cfg(feature = "rhai-runtime")]
impl Runtime {
    /// Подготавливает запуск компонента и его исходное состояние.
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
            native_app: Box::new(crate::apps::State::default()),
            wifi_ui: WifiUi::new(),
            flashlight_since_secs: None,
            bluetooth_action: None,
            bluetooth_status: 0,
            bluetooth_pin: None,
            wifi_ip: None,
            timer_editor: 0,
            bedtime_override: None,
            bedtime_previous_auto: false,
            bedtime_auto: false,
            bedtime_start: 23 * 60,
            bedtime_end: 7 * 60,
            bedtime_brightness: 35,
            bedtime_editor: 0,
            bedtime_cursor: 0,
            security: security::Security::new(Instant::now().duration_since_epoch().as_secs()),
            security_saved: security::Security::new(0).encode(),
            crown_sensitivity: 1,
            crown_reverse: false,
            high_contrast: false,
            last_input_ms: Instant::now().duration_since_epoch().as_millis() as u64,
            timeout_parent: 20,
            watch_face: 0,
            launcher_grid: false,
            launcher_cursor: 0,
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

    /// Формирует или выводит кадр текущего состояния.
    pub fn render(&mut self) -> Result<RuntimeFrame, RuntimeError> {
        let scheduled = self.bedtime_scheduled();
        if scheduled != self.bedtime_previous_auto {
            // A manual override lasts until the next schedule boundary.
            self.bedtime_override = None;
            self.bedtime_previous_auto = scheduled;
        }
        crate::weather::widget_tick(
            self.watch_face == 4 && !self.security.locked,
            Instant::now().duration_since_epoch().as_millis() as u64,
        );
        self.native_app.poll();
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
        crate::companion::set_allowed(!self.security.locked);
        crate::file_store::set_external_access(!self.security.locked);
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
        self.enrich_frame(&mut frame);
        Ok(frame)
    }

    // Keep scene parsing and app snapshots in separate stack frames. Both
    // contain large by-value views; inlining their temporaries overflows the
    // hardware stack during navigation even when host tests pass.
    #[inline(never)]
    fn enrich_frame(&mut self, frame: &mut RuntimeFrame) {
        self.enrich_wifi_frame(frame);
        if let RuntimeFrame::Material(m) = frame {
            let name = crate::device_name::snapshot();
            m.device_name_len = name.len() as u8;
            m.device_name[..name.len()].copy_from_slice(name.as_bytes());
            m.resource_revision = crate::ui_resources::revision();
            if crate::startup::active() && self.wallpaper_enabled && self.wallpaper_index > 0 {
                let _ = crate::ui_resources::wallpaper(self.wallpaper_index);
            }
            m.startup =
                crate::startup::phase(crate::ui_resources::pending(), self.bluetooth_status == 7);
            m.app = self.native_app.view();
            m.bedtime_active = self.bedtime_active();
            m.bedtime_auto = self.bedtime_auto;
            m.bedtime_start = self.bedtime_start;
            m.bedtime_end = self.bedtime_end;
            m.bedtime_brightness = self.bedtime_brightness;
            m.bedtime_editor = self.bedtime_editor;
            m.bedtime_cursor = self.bedtime_cursor;
            if m.screen == 1 && self.watch_face == 4 {
                m.app.weather = crate::weather::snapshot();
            }
            if matches!(m.screen, 2 | 26) {
                m.cursor = self.launcher_cursor.min(crate::apps::COUNT);
            }
            if matches!(m.app.id, crate::apps::FILES | crate::apps::EDITOR)
                && matches!(m.app.files.mode, 2 | 4)
            {
                m.keyboard = m.app.files.keyboard;
                m.keyboard_error = m.app.files.error;
            }
            if matches!(m.app.id, crate::apps::TASKS | crate::apps::EXPENSES)
                && m.app.organizer.editing
            {
                m.keyboard = m.app.organizer.keyboard;
                m.keyboard_error = m.app.organizer.error;
            }
            if m.app.id == crate::apps::MEMORY {
                m.app.memory = crate::swap_config::snapshot();
            }
            if m.app.id == crate::apps::DIAGNOSTICS {
                m.app.temperature_deci_c = crate::temperature::snapshot();
                m.app.memory = crate::swap_config::snapshot();
                m.app.uptime = Instant::now().duration_since_epoch().as_secs();
                m.app.free_kib = (esp_alloc::HEAP.free() / 1024) as u32;
                m.app.internal_kib = (esp_alloc::HEAP
                    .free_caps(esp_alloc::MemoryCapability::Internal.into())
                    / 1024) as u32;
                m.app.script_us = self.scene_cache.stats.last_us;
                m.app.cache_hits = self.scene_cache.stats.cache_hits;
            }
            if m.app.id == crate::apps::WEATHER && m.app.weather_editor.active {
                m.keyboard = m.app.weather_editor.keyboard;
                m.keyboard_error = 0;
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
            m.aod_enabled = self.aod_enabled || self.bedtime_active();
            m.aod = m.aod_enabled && m.display_sleep;
            if m.aod {
                m.display_sleep = false;
                m.display_brightness = Some(if self.bedtime_active() {
                    self.bedtime_brightness
                } else {
                    5
                });
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
    }

    /// Возвращает текущее упакованное состояние runtime.
    pub fn state(&self) -> i32 {
        self.state
    }
    /// Проверяет активность диагностики без построения полного снимка.
    pub fn diagnostics_active(&self) -> bool {
        self.native_app.id() == crate::apps::DIAGNOSTICS
    }

    /// Delay singles only where a double click has a visible action. Keyboard
    /// input and alert dismissal must remain immediate.
    pub fn file_gestures_active(&self) -> bool {
        if self.security.locked || clock::alert() != 0 {
            return false;
        }
        ((self.state & 31) == 6
            && self.native_app.id() == crate::apps::NONE
            && matches!((self.state >> 5) & 15, 3 | 4)
            && self.bedtime_editor == 0)
            || self.native_app.browser_clicks_active()
            || self.native_app.id() == crate::apps::CALENDAR
            || (matches!(
                self.native_app.id(),
                crate::apps::FILES | crate::apps::EDITOR
            ) && matches!(self.native_app.view().files.mode, 0 | 3))
    }
    /// Возвращает счётчики работы Rhai и кэша.
    pub fn script_stats(&self) -> ScriptStats {
        self.scene_cache.stats
    }
    /// Определяет ближайшую проверку интерфейса с учётом затемнения и сна.
    pub fn next_background_refresh_ms(&self, now_ms: u64) -> u64 {
        if crate::startup::active() {
            now_ms + 80
        } else {
            idle::next_refresh(now_ms, self.last_input_ms, self.security.timeout)
        }
    }
    /// Забирает отложенную команду питания для аппаратного слоя.
    pub fn take_system_action(&mut self) -> Option<SystemAction> {
        self.system_action.take()
    }
    /// Сообщает, заблокирован ли интерфейс локальным PIN.
    pub fn is_locked(&self) -> bool {
        self.security.locked
    }
    /// Проверяет доступность и ставит команду питания в очередь.
    pub fn request_system_action(&mut self, action: SystemAction) -> bool {
        if self.is_locked() {
            return false;
        }
        self.system_action = Some(action);
        true
    }
    /// Обновляет признак доступности пользовательских обоев.
    pub fn wallpaper_available(&mut self, available: bool) {
        self.has_wallpaper = available || crate::wallpaper::builtin_count() != 0;
    }
    /// Выбирает успешно загруженные пользовательские обои.
    pub fn use_uploaded_wallpaper(&mut self) {
        self.wallpaper_index = 0;
        self.wallpaper_enabled = true;
        self.has_wallpaper = true;
    }
    /// Вычисляет яркость с учётом бездействия и необходимости внимания.
    fn idle_brightness(&self, now_ms: u64) -> u8 {
        let attention =
            clock::alert() != 0 || self.bluetooth_pin.is_some() || (self.state & 31) == 29;
        let selected = ((self.state >> 9) & 127) as u8;
        // Manual bedtime is an overlay: normal brightness/AOD preferences stay
        // intact. Alarms, pairing and the flashlight still receive full attention.
        let level = if self.bedtime_active() && !attention {
            self.bedtime_brightness
        } else {
            selected
        };
        idle::level(
            level,
            now_ms.saturating_sub(self.last_input_ms),
            self.security.timeout,
            attention || self.native_app.keep_awake(),
        )
    }
    fn bedtime_scheduled(&self) -> bool {
        self.bedtime_auto
            && crate::rtc::valid()
            && sleep_window(
                crate::rtc::local_minutes() as u16,
                self.bedtime_start,
                self.bedtime_end,
            )
    }
    fn bedtime_active(&self) -> bool {
        self.bedtime_override
            .unwrap_or_else(|| self.bedtime_scheduled())
    }
    /// Проверяет, наступил ли срок полного выключения экрана.
    fn display_should_sleep(&self, now_ms: u64) -> bool {
        self.idle_brightness(now_ms) == 0
    }
    /// Возвращает сохраняемую конфигурацию таймера и будильника.
    pub fn clock_config(&self) -> i32 {
        clock::config()
    }
    /// Восстанавливает сохранённую конфигурацию часов.
    pub fn restore_clock_config(&mut self, value: i32) {
        clock::restore_config(value);
    }

    /// Забирает Bluetooth-команду, выбранную пользователем.
    pub fn take_bluetooth_action(&mut self) -> Option<BluetoothUiAction> {
        if let Some(action) = self.native_app.bluetooth_action() {
            if action == BluetoothUiAction::Pair {
                self.native_app.close();
                self.state = (self.state & !0x1ff) | 30;
            }
            return Some(action);
        }
        self.bluetooth_action.take()
    }

    /// Обновляет реальное состояние медиаплеера по данным компаньона.
    pub fn set_playback(&mut self, playback: crate::media::Playback) {
        self.playback = Some(playback);
    }
    /// Обновляет состояние BLE и код подтверждения.
    pub fn set_bluetooth_status(&mut self, status: u8, pin: Option<u32>) -> bool {
        let changed = self.bluetooth_status != status || self.bluetooth_pin != pin;
        self.bluetooth_status = status;
        self.native_app.bluetooth_status(status);
        if !matches!(status, 2 | 3) {
            self.playback = None;
        }
        self.bluetooth_pin = pin;
        changed
    }

    /// Обновляет отображаемый IP-адрес, сообщая об изменениях.
    pub fn set_wifi_ip(&mut self, ip: Option<[u8; 4]>) -> bool {
        let changed = self.wifi_ip != ip;
        self.wifi_ip = ip;
        changed
    }

    /// Очищает старый список точек перед новым сканированием.
    pub fn clear_wifi_scan(&mut self) {
        self.wifi_ui.aps.clear();
        self.wifi_ui.selected = 0;
    }
    /// Добавляет одну найденную Wi-Fi-сеть в ограниченный список.
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
    /// Обновляет этап подключения Wi-Fi и признаки перерисовки.
    pub fn set_wifi_link_state(&mut self, state: u8) -> bool {
        let next = state.min(4);
        let changed = self.wifi_ui.link_state != next;
        self.wifi_ui.link_state = next;
        changed
    }
    /// Забирает запрос сканирования или подключения из интерфейса.
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
    /// Дополняет кадр актуальными Wi-Fi-данными и состоянием клавиатуры.
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

    /// Заменяет упакованное состояние после проверки допустимости.
    pub fn replace_state(&mut self, state: i32) -> Result<RuntimeFrame, RuntimeError> {
        if state < 0 {
            return Err(RuntimeError::NonZero);
        }
        self.state = state;
        if matches!(state & 31, 2 | 26) {
            self.launcher_cursor = ((state >> 5) & 15) as u8;
        }
        self.render()
    }
}

#[cfg(not(feature = "rhai-runtime"))]
/// Владелец Rhai-движка и нативного состояния интерфейса компонента Rhai runtime и нативного интерфейса.
pub struct Runtime;

#[cfg(not(feature = "rhai-runtime"))]
impl Runtime {
    /// Подготавливает запуск компонента и его исходное состояние.
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
    /// Проверяет активность диагностики без построения полного снимка.
    pub fn file_gestures_active(&self) -> bool {
        false
    }
    pub fn diagnostics_active(&self) -> bool {
        false
    }
    /// Возвращает счётчики работы Rhai и кэша.
    pub fn script_stats(&self) -> ScriptStats {
        ScriptStats::default()
    }
    /// Определяет ближайшую проверку интерфейса с учётом затемнения и сна.
    pub fn next_background_refresh_ms(&self, now_ms: u64) -> u64 {
        now_ms.saturating_add(1000)
    }
    /// Формирует или выводит кадр текущего состояния.
    pub fn render(&mut self) -> Result<RuntimeFrame, RuntimeError> {
        Self::start(String::new()).map(|v| v.1)
    }
    /// Обрабатывает пользовательский ввод с проверкой блокировки и текущего экрана.
    pub fn dispatch(&mut self, _event: RuntimeEvent) -> Result<RuntimeFrame, RuntimeError> {
        self.render()
    }
    /// Возвращает выбранную чувствительность колёсика.
    pub fn crown_sensitivity(&self) -> u8 {
        1
    }
    /// Возвращает направление вращения колёсика.
    pub fn crown_reversed(&self) -> bool {
        false
    }
    /// Кодирует пользовательские параметры в версионированный формат.
    pub fn preference_config(&self) -> [u8; 20] {
        [3, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0]
    }
    /// Восстанавливает совместимые версии пользовательских параметров.
    pub fn restore_preferences(&mut self, _raw: &[u8]) {}
    /// Отмечает успешную запись параметров PIN.
    pub fn security_saved(&mut self) {}
    /// Возвращает сериализованные параметры локальной блокировки.
    pub fn security_config(&self) -> [u8; 58] {
        [0; 58]
    }
    /// Проверяет и восстанавливает сохранённую конфигурацию PIN.
    pub fn restore_security(&mut self, _raw: &[u8]) {}
    /// Сообщает интерфейсу об ошибке хранилища PIN.
    pub fn security_storage_error(&mut self) {}
    /// Обновляет доступность постоянного хранения параметров PIN.
    pub fn set_security_storage(&mut self, _available: bool, _entropy: [u8; 16]) {}
    /// Возвращает текущее упакованное состояние runtime.
    pub fn state(&self) -> i32 {
        0
    }
    /// Забирает отложенную команду питания для аппаратного слоя.
    pub fn take_system_action(&mut self) -> Option<SystemAction> {
        None
    }
    /// Сообщает, заблокирован ли интерфейс локальным PIN.
    pub fn is_locked(&self) -> bool {
        false
    }
    /// Проверяет доступность и ставит команду питания в очередь.
    pub fn request_system_action(&mut self, _action: SystemAction) -> bool {
        false
    }
    /// Обновляет признак доступности пользовательских обоев.
    pub fn wallpaper_available(&mut self, _available: bool) {}
    /// Выбирает успешно загруженные пользовательские обои.
    pub fn use_uploaded_wallpaper(&mut self) {}
    /// Заменяет упакованное состояние после проверки допустимости.
    pub fn replace_state(&mut self, _state: i32) -> Result<RuntimeFrame, RuntimeError> {
        self.render()
    }
    /// Очищает старый список точек перед новым сканированием.
    pub fn clear_wifi_scan(&mut self) {}
    /// Добавляет одну найденную Wi-Fi-сеть в ограниченный список.
    pub fn push_wifi_ap(&mut self, _ssid: &str, _rssi: i8) {}
    /// Обновляет этап подключения Wi-Fi и признаки перерисовки.
    pub fn set_wifi_link_state(&mut self, _state: u8) -> bool {
        false
    }
    /// Забирает запрос сканирования или подключения из интерфейса.
    pub fn take_wifi_ui_action(&mut self) -> Option<WifiUiAction> {
        None
    }
    /// Забирает Bluetooth-команду, выбранную пользователем.
    pub fn take_bluetooth_action(&mut self) -> Option<BluetoothUiAction> {
        None
    }
    /// Обновляет реальное состояние медиаплеера по данным компаньона.
    pub fn set_playback(&mut self, _playback: crate::media::Playback) {}
    /// Обновляет состояние BLE и код подтверждения.
    pub fn set_bluetooth_status(&mut self, _status: u8, _pin: Option<u32>) -> bool {
        false
    }
    /// Возвращает сохраняемую конфигурацию таймера и будильника.
    pub fn clock_config(&self) -> i32 {
        0
    }
    /// Восстанавливает сохранённую конфигурацию часов.
    pub fn restore_clock_config(&mut self, _value: i32) {}
    /// Обновляет отображаемый IP-адрес, сообщая об изменениях.
    pub fn set_wifi_ip(&mut self, _ip: Option<[u8; 4]>) -> bool {
        false
    }
}

#[path = "../../App/system/priv-app/SystemUI/activity_manager.rs"]
mod activity_manager;
#[path = "../../App/system/app/Settings/activity.rs"]
mod settings_activity;
#[path = "../../App/system/app/Settings/preferences.rs"]
mod settings_preferences;

/// Daily local-time interval, including midnight. Equal endpoints disable the
/// interval instead of silently keeping sleep mode on for the entire day.
fn sleep_window(now: u16, start: u16, end: u16) -> bool {
    if start < end {
        now >= start && now < end
    } else if start > end {
        now >= start || now < end
    } else {
        false
    }
}
