// Модуль программных часов и часового пояса.
//! Software wall-clock RTC for the ESP32-S3 Wear runtime.
//!
//! The ESP32-S3 board used by this project has no battery-backed calendar RTC.
//! We therefore keep wall time as an epoch base plus the chip monotonic timer.
//! Wi-Fi SNTP, the paired-phone companion, or manual settings correct the base;
//! normal timekeeping never depends on USB or the desktop viewer.

use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use esp_hal::time::Instant;

/// Параметр DEFAULT_TIME_ZONE_MINUTES компонента программных часов и часового пояса.
const DEFAULT_TIME_ZONE_MINUTES: i32 = 180; // UTC+03 device default.

/// Параметр BASE_UNIX_SECONDS компонента программных часов и часового пояса.
static BASE_UNIX_SECONDS: AtomicI32 = AtomicI32::new(0);
/// Параметр BASE_UPTIME_SECONDS компонента программных часов и часового пояса.
static BASE_UPTIME_SECONDS: AtomicI32 = AtomicI32::new(0);
/// Параметр VALID компонента программных часов и часового пояса.
static VALID: AtomicBool = AtomicBool::new(false);
/// Параметр AUTOMATIC компонента программных часов и часового пояса.
static AUTOMATIC: AtomicBool = AtomicBool::new(true);
/// Параметр NETWORK_SYNC_REQUESTED компонента программных часов и часового пояса.
static NETWORK_SYNC_REQUESTED: AtomicBool = AtomicBool::new(true);
/// Параметр TIME_ZONE_OFFSET_MINUTES компонента программных часов и часового пояса.
static TIME_ZONE_OFFSET_MINUTES: AtomicI32 = AtomicI32::new(DEFAULT_TIME_ZONE_MINUTES);

/// Возвращает время работы устройства в секундах.
fn uptime_seconds() -> i32 {
    (Instant::now().duration_since_epoch().as_secs() & 0x7fff_ffff) as i32
}

/// Вычисляет Unix-время по сохранённой базе и монотонному отсчёту.
fn unix_seconds_raw() -> i32 {
    let now = uptime_seconds();
    BASE_UNIX_SECONDS
        .load(Ordering::Relaxed)
        .saturating_add(now.saturating_sub(BASE_UPTIME_SECONDS.load(Ordering::Relaxed)))
}

/// Сообщает, было ли время подтверждено источником RTC.
pub fn valid() -> bool {
    VALID.load(Ordering::Relaxed)
}
/// Возвращает признак автоматической синхронизации часов.
pub fn automatic() -> bool {
    AUTOMATIC.load(Ordering::Relaxed)
}
/// Возвращает смещение местного времени относительно UTC в минутах.
pub fn timezone_minutes() -> i32 {
    TIME_ZONE_OFFSET_MINUTES.load(Ordering::Relaxed)
}
/// Возвращает смещение UTC в часах для настроек интерфейса.
pub fn timezone_hours() -> i8 {
    (timezone_minutes() / 60).clamp(-12, 14) as i8
}

/// One-shot request consumed by the connectivity service. This makes enabling
/// Automatic time trigger an immediate SNTP refresh instead of waiting for the
/// periodic six-hour network resync.
pub fn take_network_sync_request() -> bool {
    NETWORK_SYNC_REQUESTED.swap(false, Ordering::Relaxed)
}

/// Return local wall-clock minutes. The value keeps advancing from the ESP
/// monotonic timer even when Wi-Fi is off.
pub fn local_minutes() -> i32 {
    // Before the first authoritative sync we still expose a real ticking
    // software clock instead of a frozen "--:--" surface.  It starts at
    // 00:00 on boot and is corrected atomically when SNTP/phone/manual time
    // becomes available. Unsynchronised time is never persisted as holdover.
    if !valid() {
        return (uptime_seconds().div_euclid(60)).rem_euclid(1440);
    }
    let utc_minutes = unix_seconds_raw().div_euclid(60);
    (utc_minutes + timezone_minutes()).rem_euclid(1440)
}

/// Seconds follow the same RTC base as minutes, including manual corrections.
pub fn local_seconds() -> u8 {
    (if valid() {
        unix_seconds_raw()
    } else {
        uptime_seconds()
    })
    .rem_euclid(60) as u8
}
/// Real calendar date is shown only when an epoch source supplied it.
pub fn calendar() -> Option<(u8, u8)> {
    calendar_date().map(|(_, month, day)| (day, month))
}

/// Local civil date, including the year for the calendar application.
pub fn calendar_date() -> Option<(u16, u8, u8)> {
    let unix = unix_seconds_raw();
    if !valid() || unix < 1_000_000_000 {
        return None;
    }
    let z = (i64::from(unix) + i64::from(timezone_minutes()) * 60).div_euclid(86400) + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Some((year as u16, month as u8, day as u8))
}

/// Устанавливает местное время, сохраняя текущую дату, когда она известна.
pub fn set_local_minutes(minutes: u16) {
    let target_local = (minutes % 1440) as i32 * 60;
    let tz = timezone_minutes() * 60;
    let now = uptime_seconds();
    let current = unix_seconds_raw();
    let day = if valid() {
        current.div_euclid(86_400) * 86_400
    } else {
        0
    };
    BASE_UNIX_SECONDS.store(day + target_local - tz, Ordering::Relaxed);
    BASE_UPTIME_SECONDS.store(now, Ordering::Relaxed);
    VALID.store(true, Ordering::Relaxed);
}

/// Apply authoritative network time. Manual time mode is never overridden.
pub fn set_network_unix_seconds(unix_seconds: u32) -> bool {
    if !automatic() || unix_seconds > i32::MAX as u32 {
        return false;
    }
    BASE_UNIX_SECONDS.store(unix_seconds as i32, Ordering::Relaxed);
    BASE_UPTIME_SECONDS.store(uptime_seconds(), Ordering::Relaxed);
    VALID.store(true, Ordering::Relaxed);
    true
}

/// Paired-phone source used by the companion path/development simulator.
pub fn set_companion_minutes(minutes: u16) {
    if automatic() {
        set_local_minutes(minutes);
    }
}

/// Power-loss holdover. It deliberately does not claim elapsed time while the
/// board was fully unpowered; the next SNTP/phone sync corrects that drift.
pub fn restore_holdover_minutes(minutes: u16) {
    set_local_minutes(minutes);
}

/// Возвращает сохраняемое время только при действительном RTC.
pub fn snapshot_minutes() -> Option<u16> {
    if valid() {
        Some(local_minutes() as u16)
    } else {
        None
    }
}

/// Compact persisted configuration: bit 0 automatic-time, bits 8..15 signed
/// timezone hours biased by 16. This is independent from Activity state pages.
pub fn config_snapshot() -> i32 {
    let auto = if automatic() { 1 } else { 0 };
    let tz = timezone_hours() as i32 + 16;
    auto | ((tz & 0xff) << 8)
}

/// Восстанавливает параметры компонента из проверенного представления.
pub fn restore_config(raw: i32) {
    let auto = (raw & 1) != 0;
    AUTOMATIC.store(auto, Ordering::Relaxed);
    if auto {
        NETWORK_SYNC_REQUESTED.store(true, Ordering::Relaxed);
    }
    let tz = (((raw >> 8) & 0xff) - 16).clamp(-12, 14);
    TIME_ZONE_OFFSET_MINUTES.store(tz * 60, Ordering::Relaxed);
}

/// Settings action ABI used by the Rhai Date & time Activity.
pub fn action(action: i32) -> i32 {
    match action {
        0 => {
            let next = !automatic();
            AUTOMATIC.store(next, Ordering::Relaxed);
            if next {
                NETWORK_SYNC_REQUESTED.store(true, Ordering::Relaxed);
            }
        }
        1 if !automatic() => set_local_minutes(((local_minutes() + 60).rem_euclid(1440)) as u16),
        2 if !automatic() => set_local_minutes(((local_minutes() + 1).rem_euclid(1440)) as u16),
        3 if !automatic() => {
            let mut hours = timezone_hours() as i32 + 1;
            if hours > 14 {
                hours = -12;
            }
            TIME_ZONE_OFFSET_MINUTES.store(hours * 60, Ordering::Relaxed);
        }
        _ => {}
    }
    1
}
