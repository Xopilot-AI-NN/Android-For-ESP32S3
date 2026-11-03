// Модуль программной поверхности Material UI.
extern crate alloc;

use alloc::{vec, vec::Vec};

#[path = "material/localization.rs"]
mod localization;
/// Локаль системных подписей по умолчанию.
pub const DEFAULT_LOCALE: &str = localization::DEFAULT_LOCALE;

/// Логическая ширина программного экрана, пиксели.
pub const WIDTH: usize = 240;
/// Логическая высота программного экрана, пиксели.
pub const HEIGHT: usize = 280;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Полный снимок данных для программного рендерера компонента программной поверхности Material UI.
pub struct MaterialFrame {
    /// Снимок активного нативного приложения.
    pub app: crate::apps::View,
    /// Ревизия графических ресурсов для сброса зависимого кэша.
    pub resource_revision: u32,
    /// Текущий этап стартового экрана.
    pub startup: u8,
    /// Текущее дружественное имя устройства в ограниченном буфере.
    pub device_name: [u8; 32],
    /// Число значащих байтов буфера device_name.
    pub device_name_len: u8,
    /// Признак открытого редактора имени.
    pub name_editing: bool,
    /// Состояние общей RU/EN-клавиатуры.
    pub keyboard: crate::text_input::KeyboardView,
    /// Код ошибки проверки пользовательского ввода.
    pub keyboard_error: u8,
    /// Черновик имени для отображения на часах.
    pub name_draft: [u8; 32],
    /// Число значащих байтов буфера name_draft.
    pub name_draft_len: u8,
    /// Номер текущего системного экрана.
    pub screen: u8,
    /// Индекс текущей цели навигации.
    pub cursor: u8,
    /// Пользовательское значение яркости.
    pub brightness: u8,
    /// Признак режима «Не беспокоить».
    pub dnd: bool,
    /// Признак включённого авиарежима.
    pub airplane: bool,
    /// Индекс цветовой темы.
    pub theme: u8,
    /// Признак локальной PIN-блокировки интерфейса.
    pub locked: bool,
    /// Системный флаг либо драйвер станции Wi-Fi.
    pub wifi: bool,
    /// Системный флаг Bluetooth.
    pub bt: bool,
    /// Разрешение беспроводной отладки ADB.
    pub adb: bool,
    /// Количество ожидающих уведомлений.
    pub notes: u8,
    /// Местное время в минутах суток.
    pub time_minutes: u16,
    /// Оставшаяся длительность таймера, с.
    pub timer_secs: u16,
    /// Показание секундомера, с.
    pub stopwatch_secs: u16,
    /// Признак включённого будильника.
    pub alarm_enabled: bool,
    /// Разрешение автоматической синхронизации RTC.
    pub auto_time: bool,
    /// Время подтверждено источником или установлено пользователем.
    pub time_valid: bool,
    /// Смещение местного времени относительно UTC в часах.
    pub timezone_hours: i8,
    /// Число сохранённых Activity-страниц.
    pub swap_pages: u8,
    /// Количество доступных результатов сканирования.
    pub wifi_ap_count: u8,
    /// Индекс выбранной точки доступа.
    pub wifi_ap_index: u8,
    /// SSID выбранной сети в UTF-8-буфере.
    pub wifi_ap_ssid: [u8; 32],
    /// Число значащих байтов буфера wifi_ap_ssid.
    pub wifi_ap_ssid_len: u8,
    /// Измеренный уровень сигнала выбранной сети, dBm.
    pub wifi_ap_rssi: i8,
    /// Число значащих байтов буфера wifi_password.
    pub wifi_password_len: u8,
    // On-device-only clear-text preview. Desktop serializers deliberately
    // omit these bytes; they exist only so the physical keyboard can show
    // exactly what was entered with the crown.
    pub wifi_password_preview: [u8; 16],
    /// Число значащих байтов буфера wifi_password_preview.
    pub wifi_password_preview_len: u8,
    /// Совместимое с прежним протоколом положение клавиатуры пароля.
    pub wifi_editor_char: u8,
    /// Этап ассоциации/DHCP/готовности сети для интерфейса.
    pub wifi_link_state: u8,
    /// Live HID state: off, advertising, connected, ready, confirmation, error.
    pub bt_status: u8,
    pub bt_pin: Option<u32>,
    /// Время будильника в минутах суток.
    pub alarm_minutes: u16,
    /// Активный сигнал таймера или будильника.
    pub clock_alert: u8,
    /// Текущий IP-адрес Wi-Fi либо отсутствие адреса.
    pub wifi_ip: Option<[u8; 4]>,
    /// Подрежим выбора длительности таймера/секундомера.
    pub timer_editor: u8,
    /// Выбранная длительность таймера, секунды.
    pub timer_duration_secs: u16,
    /// Выбранный уровень чувствительности колёсика.
    pub crown_sensitivity: u8,
    /// Признак обратного направления колёсика.
    pub crown_reverse: bool,
    /// Признак повышенной контрастности интерфейса.
    pub high_contrast: bool,
    /// Признак настроенного PIN.
    pub has_pin: bool,
    /// Параметры PIN изменились и ожидают сохранения.
    pub security_pending: bool,
    /// Шаг ввода или настройки PIN.
    pub security_view: u8,
    /// Количество уже введённых PIN-цифр.
    pub pin_count: u8,
    /// Выбранная следующая цифра PIN.
    pub pin_digit: u8,
    /// Код результата операции PIN для интерфейса.
    pub security_message: u8,
    /// Оставшийся запрет повторного ввода PIN, секунды.
    pub lockout_secs: u16,
    /// Выбранный срок автоблокировки/сна, секунды.
    pub lock_timeout_secs: u16,
    /// Экран должен быть полностью выключен до пользовательского ввода.
    pub display_sleep: bool,
    /// Редактируется срок сна дисплея, а не защитной блокировки.
    pub timeout_is_display: bool,
    /// Яркость после применения idle-политики, если она переопределена.
    pub display_brightness: Option<u8>,
    /// Коэффициент смешивания при переходе между кадрами.
    pub transition_alpha: u8,
    /// Дробная фаза отображения между целыми секундами.
    pub animation_phase: u8,
    /// Номер выбранного сохраняемого циферблата.
    pub watch_face: u8,
    /// Секунды текущей минуты RTC.
    pub time_seconds: u8,
    /// Подтверждённое число месяца либо ноль при неизвестной дате.
    pub date_day: u8,
    /// Подтверждённый номер месяца.
    pub date_month: u8,
    /// Признак запущенного секундомера.
    pub stopwatch_running: bool,
    /// Признак открытого меню питания.
    pub power_menu: bool,
    /// Разрешение отображать выбранные обои.
    pub wallpaper_enabled: bool,
    /// Признак наличия загруженных пользовательских обоев.
    pub wallpaper_available: bool,
    /// Номер обоев; ноль соответствует пользовательскому изображению.
    pub wallpaper_index: u8,
    /// Масштаб яркости обоев, проценты.
    pub wallpaper_level: u8,
    /// Разрешение показа секунд на поддерживающих их циферблатах.
    pub show_seconds: bool,
    /// Пользовательское разрешение режима Always-on Display.
    pub aod_enabled: bool,
    /// Признак текущего ограниченного AOD-кадра.
    pub aod: bool,
    /// Есть действительные данные воспроизведения от компаньона.
    pub media_known: bool,
    /// Подтверждённый признак воспроизведения.
    pub media_playing: bool,
    /// Позиция трека в долях 0–1000.
    pub media_progress: u16,
}

impl MaterialFrame {
    /// Разбирает и проверяет входное представление данных.
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
            device_name: [0; 32],
            device_name_len: 0,
            name_editing: false,
            keyboard: crate::text_input::KeyboardView::default(),
            keyboard_error: 0,
            resource_revision: 0,
            startup: 0,
            app: crate::apps::View::default(),
            name_draft: [0; 32],
            name_draft_len: 0,
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
            crown_sensitivity: 1,
            crown_reverse: false,
            high_contrast: false,
            has_pin: false,
            security_pending: false,
            security_view: 0,
            pin_count: 0,
            pin_digit: 0,
            security_message: 0,
            lockout_secs: 0,
            lock_timeout_secs: 30,
            display_sleep: false,
            timeout_is_display: false,
            display_brightness: None,
            transition_alpha: 255,
            animation_phase: 0,
            watch_face: 0,
            time_seconds: 0,
            date_day: 0,
            date_month: 0,
            stopwatch_running: false,
            power_menu: false,
            wallpaper_enabled: true,
            wallpaper_available: false,
            wallpaper_index: 1,
            wallpaper_level: 85,
            show_seconds: true,
            aod_enabled: false,
            aod: false,
            media_known: false,
            media_playing: false,
            media_progress: 0,
        })
    }

    /// Обновляет отображаемое число сохранённых страниц состояния.
    pub fn set_swap_pages(&mut self, pages: u8) {
        self.swap_pages = pages;
    }

    /// Возвращает резервные подписи для простого backend без Material UI.
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
/// Прямоугольная область обновления кадра компонента программной поверхности Material UI.
pub struct Rect {
    /// Горизонтальная координата в логических пикселях.
    pub x: i32,
    /// Вертикальная координата в логических пикселях.
    pub y: i32,
    /// Ширина области в пикселях.
    pub w: i32,
    /// Высота области в пикселях.
    pub h: i32,
}
impl Rect {
    /// Параметр FULL компонента программной поверхности Material UI.
    pub const FULL: Self = Self {
        x: 0,
        y: 0,
        w: WIDTH as i32,
        h: HEIGHT as i32,
    };
    /// Возвращает правую границу прямоугольника.
    pub fn x1(self) -> i32 {
        self.x + self.w - 1
    }
    /// Возвращает нижнюю границу прямоугольника.
    pub fn y1(self) -> i32 {
        self.y + self.h - 1
    }
}

#[derive(Clone, Copy)]
/// Согласованная палитра элементов интерфейса компонента программной поверхности Material UI.
struct Theme {
    /// Основной цвет фона.
    bg: u16,
    /// Основная подложка карточек.
    surface: u16,
    /// Приподнятая подложка кнопок и строк.
    surface_high: u16,
    /// Основной акцентный цвет темы.
    primary: u16,
    /// Цвет текста/иконок на основном акценте.
    on_primary: u16,
    /// Подложка акцентных контейнеров.
    primary_container: u16,
    /// Цвет содержимого акцентного контейнера.
    on_primary_container: u16,
    /// Основной цвет текста на поверхности.
    on_surface: u16,
    /// Вторичный цвет подписей на поверхности.
    on_surface_variant: u16,
    /// Цвет контуров и неактивных элементов.
    outline: u16,
    /// Сигнальный цвет ошибки темы.
    error: u16,
}

/// Упаковывает компоненты RGB888 в 16-битный RGB565.
const fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    (((r as u16) & 0xF8) << 8) | (((g as u16) & 0xFC) << 3) | ((b as u16) >> 3)
}

/// Выбирает согласованную цветовую схему по номеру темы.
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
            primary: rgb565(130, 235, 250),
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

/// Владелец RGB565-поверхности и графических кэшей компонента программной поверхности Material UI.
pub struct Surface {
    /// Пиксели кадрового буфера в формате RGB565.
    pixels: Vec<u16>,
    /// Кэш выбранных пользовательских/системных обоев.
    wallpaper: Option<Vec<u16>>,
    /// Пиксели загруженных пользователем обоев.
    custom_wallpaper: Option<Vec<u16>>,
    /// Номер изображения, уже загруженного в графический кэш.
    selected_wallpaper: u8,
}

impl Surface {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
    pub fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT],
            wallpaper: None,
            custom_wallpaper: None,
            selected_wallpaper: u8::MAX,
        }
    }
    /// Возвращает готовые пиксели программной поверхности.
    pub fn pixels(&self) -> &[u16] {
        &self.pixels
    }
    /// Принимает подготовленный RGB565-буфер пользовательских обоев.
    pub fn set_wallpaper(&mut self, pixels: Vec<u16>) {
        if pixels.len() == WIDTH * HEIGHT {
            self.custom_wallpaper = Some(pixels);
            self.selected_wallpaper = u8::MAX;
        }
    }

    /// Формирует или выводит кадр текущего состояния.
    pub fn render(&mut self, frame: MaterialFrame) -> Rect {
        let mut t = theme(frame.theme);
        if frame.high_contrast {
            t.surface = 0;
            t.surface_high = rgb565(28, 28, 28);
            t.on_surface = 0xffff;
            t.on_surface_variant = 0xffff;
            t.outline = 0xffff;
        }
        self.fill_rect(0, 0, WIDTH as i32, HEIGHT as i32, t.bg);
        if frame.startup != 0 && !frame.power_menu {
            self.boot_frame(frame.startup - 1);
            return Rect::FULL;
        }
        if frame.aod {
            // LCD AOD: no wallpaper, no private data and only minute updates.
            self.fill_rect(0, 0, WIDTH as i32, HEIGHT as i32, 0);
            let text = Self::time_text(frame.time_minutes);
            let shift = i32::from(frame.time_minutes % 5) - 2;
            self.text_center_raw(
                119 + shift,
                core::str::from_utf8(&text).unwrap_or("00:00"),
                3,
                0xffff,
            );
            return Rect::FULL;
        }
        if frame.screen == 1 && frame.wallpaper_enabled {
            if self.selected_wallpaper != frame.wallpaper_index {
                self.wallpaper = if frame.wallpaper_index == 0 {
                    self.custom_wallpaper
                        .clone()
                        .or_else(|| crate::wallpaper::builtin(1))
                } else {
                    crate::wallpaper::builtin(frame.wallpaper_index)
                };
                if self.wallpaper.is_some() {
                    self.selected_wallpaper = frame.wallpaper_index;
                }
            }
            if let Some(wallpaper) = self.wallpaper.as_ref() {
                self.pixels.copy_from_slice(wallpaper);
                self.scale_pixels(frame.wallpaper_level);
            }
        }
        if frame.power_menu {
            self.power_page(frame, t);
            self.status_strip(frame, t);
            return Rect::FULL;
        }
        if frame.security_view != 0 && (frame.screen == 0 || frame.screen == 20) {
            self.pin_entry(frame, t);
            self.status_strip(frame, t);
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
            return Rect::FULL;
        }
        if frame.name_editing {
            self.device_name_page(frame, t);
            self.status_strip(frame, t);
            return Rect::FULL;
        }
        if frame.app.id != crate::apps::NONE {
            self.native_application(frame, t);
            self.status_strip(frame, t);
            return Rect::FULL;
        }
        match frame.screen {
            0 => self.lock_screen(frame, t),
            1 => self.watchface(frame, t),
            2 => self.list_launcher(frame, t),
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
            26 => self.launcher(frame, t),
            27 => self.launcher(frame, t),
            28 => self.brightness_editor(frame, t),
            29 => self.flashlight_page(),
            30 => self.bluetooth_pairing(frame, t),
            31 => self.alarm_editor(frame, t),
            _ => self.watchface(frame, t),
        }
        if frame.screen != 29 {
            self.status_strip(frame, t);
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
#[path = "material/keyboard.rs"]
mod keyboard;
#[path = "material/pages_apps.rs"]
mod pages_apps;
#[path = "material/primitives.rs"]
mod primitives;
#[path = "material/typography.rs"]
mod typography;

#[path = "material/symbols/mod.rs"]
mod symbols;
use symbols::Symbol;

#[cfg(test)]
mod theme_tests {
    use super::*;
    #[test]
    /// Проверяет использование выбранной палитры в медиаплеере и таймере.
    fn media_and_timer_follow_every_selected_theme() {
        for index in 0..4 {
            let mut frame =
                MaterialFrame::parse("M3|10|0|70|0|0|0|0|0|0|0|0|131|0|0|0|1|1|3").unwrap();
            frame.theme = index;
            let colors = theme(index);
            let mut surface = Surface::new();
            surface.render(frame);
            assert_eq!(
                surface.pixels()[126 * WIDTH + 39],
                Surface::focus_fill(colors.primary, true)
            );
            frame.screen = 11;
            frame.timer_editor = 1;
            surface.render(frame);
            assert_eq!(surface.pixels()[199 * WIDTH + 34], colors.primary);
            assert_eq!(surface.pixels()[199 * WIDTH + 91], colors.surface_high);
        }
    }
}
#[path = "material/compositor.rs"]
mod compositor;
#[path = "material/dial.rs"]
mod dial;
#[path = "material/frame_key.rs"]
mod frame_key;
#[path = "material/pages_boot.rs"]
mod pages_boot;

// Application-owned views are compiled into the MCU firmware, never into loader tools.
#[path = "../../App/system/app/Calculator/calculator_page.rs"]
mod app_calculator_calculator_page;
#[path = "../../App/system/app/Calendar/utilities.rs"]
mod app_calendar_utilities;
#[path = "../../App/system/app/Clock/faces.rs"]
mod app_clock_faces;
#[path = "../../App/system/app/Clock/main.rs"]
mod app_clock_main;
#[path = "../../App/system/app/Clock/weather.rs"]
mod app_clock_weather;
#[path = "../../App/system/app/Files/view.rs"]
mod app_files_view;
#[path = "../../App/system/app/Flashlight/main.rs"]
mod app_flashlight_view;
#[path = "../../App/system/app/Focus/organizer.rs"]
mod app_focus_organizer;
#[path = "../../App/system/app/Phone/main.rs"]
mod app_phone_organizer;
#[path = "../../App/system/app/Settings/bluetooth.rs"]
mod app_settings_bluetooth;
#[path = "../../App/system/app/Settings/diagnostics_page.rs"]
mod app_settings_diagnostics_page;
#[path = "../../App/system/app/Settings/main.rs"]
mod app_settings_main;
#[path = "../../App/system/app/Settings/memory_page.rs"]
mod app_settings_memory_page;
#[path = "../../App/system/app/Settings/system.rs"]
mod app_settings_system;
#[path = "../../App/system/app/Settings/wifi.rs"]
mod app_settings_wifi;
#[path = "../../App/system/app/Share/main.rs"]
mod app_share_share_page;
#[path = "../../App/system/priv-app/SystemUI/main.rs"]
mod app_systemui_main;
#[path = "../../App/system/app/Tasks/organizer.rs"]
mod app_tasks_organizer;
#[path = "../../App/system/app/WearLauncher/main.rs"]
mod app_wearlauncher_main;
#[path = "../../App/system/app/WearMedia/main.rs"]
mod app_wearmedia_main;
#[path = "../../App/system/app/WearWidgets/main.rs"]
mod app_wearwidgets_main;
#[path = "../../App/system/app/Weather/weather.rs"]
mod app_weather_weather;

#[path = "../../App/system/app/Gallery/view.rs"]
mod app_gallery_view;
#[path = "../../App/system/app/BluetoothKeyboard/view.rs"]
mod app_keyboard_view;
#[path = "../../App/shared/remote_view.rs"]
mod app_remote_view;
