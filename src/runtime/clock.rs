//! Monotonic stopwatch, timer and alarm runtime.
use super::*;

// Wall-clock time is owned by crate::rtc. Timer/stopwatch use only the
// monotonic ESP counter below and therefore remain independent of wall time.
static STOPWATCH_RUNNING: AtomicBool = AtomicBool::new(false);
static STOPWATCH_START_SECONDS: AtomicI32 = AtomicI32::new(0);
static STOPWATCH_ACCUM_SECONDS: AtomicI32 = AtomicI32::new(0);
static TIMER_RUNNING: AtomicBool = AtomicBool::new(false);
static TIMER_END_SECONDS: AtomicI32 = AtomicI32::new(0);
static ALARM_ENABLED: AtomicBool = AtomicBool::new(false);
static ALARM_MINUTES: AtomicI32 = AtomicI32::new(7 * 60);
static ALARM_LATCHED: AtomicBool = AtomicBool::new(false);
static ALARM_RINGING: AtomicBool = AtomicBool::new(false);
static TIMER_EXPIRED: AtomicBool = AtomicBool::new(false);
static TIMER_DURATION: AtomicI32 = AtomicI32::new(300);
pub(super) fn stopwatch_running() -> bool {
    STOPWATCH_RUNNING.load(Ordering::Relaxed)
}

pub(super) fn timer_duration() -> u16 {
    TIMER_DURATION.load(Ordering::Relaxed) as u16
}
pub(super) fn set_timer_duration(seconds: i32) {
    TIMER_DURATION.store(seconds.clamp(60, 3600), Ordering::Relaxed);
}
pub(super) fn start_timer() {
    TIMER_EXPIRED.store(false, Ordering::Relaxed);
    TIMER_END_SECONDS.store(
        uptime_seconds().saturating_add(TIMER_DURATION.load(Ordering::Relaxed)),
        Ordering::Relaxed,
    );
    TIMER_RUNNING.store(true, Ordering::Relaxed);
}

fn uptime_seconds() -> i32 {
    (Instant::now().duration_since_epoch().as_secs() & 0x7fff_ffff) as i32
}

pub(super) fn clock_action(action: i32) -> i32 {
    let now = uptime_seconds();
    match action {
        0 => {
            if STOPWATCH_RUNNING.load(Ordering::Relaxed) {
                let start = STOPWATCH_START_SECONDS.load(Ordering::Relaxed);
                let elapsed = now.saturating_sub(start);
                let accum = STOPWATCH_ACCUM_SECONDS
                    .load(Ordering::Relaxed)
                    .saturating_add(elapsed);
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
            TIMER_EXPIRED.store(false, Ordering::Relaxed);
            if TIMER_RUNNING.load(Ordering::Relaxed) {
                TIMER_RUNNING.store(false, Ordering::Relaxed);
            } else {
                start_timer();
            }
        }
        3 => {
            let next = !ALARM_ENABLED.load(Ordering::Relaxed);
            ALARM_ENABLED.store(next, Ordering::Relaxed);
            ALARM_RINGING.store(false, Ordering::Relaxed);
        }
        4 => {
            let v = ALARM_MINUTES.load(Ordering::Relaxed);
            ALARM_MINUTES.store((v + 60) % 1440, Ordering::Relaxed);
        }
        5 => {
            let v = ALARM_MINUTES.load(Ordering::Relaxed);
            ALARM_MINUTES.store((v / 60) * 60 + (v + 1) % 60, Ordering::Relaxed);
        }
        6 => {
            ALARM_RINGING.store(false, Ordering::Relaxed);
            TIMER_EXPIRED.store(false, Ordering::Relaxed);
        }
        _ => {}
    }
    1
}

pub(super) fn clock_snapshot() -> (i32, i32, i32) {
    let now = uptime_seconds();
    let mut stopwatch = STOPWATCH_ACCUM_SECONDS.load(Ordering::Relaxed);
    if STOPWATCH_RUNNING.load(Ordering::Relaxed) {
        stopwatch = stopwatch
            .saturating_add(now.saturating_sub(STOPWATCH_START_SECONDS.load(Ordering::Relaxed)));
    }
    stopwatch = stopwatch.clamp(0, 3599);

    let mut timer = 0;
    if TIMER_RUNNING.load(Ordering::Relaxed) {
        let end = TIMER_END_SECONDS.load(Ordering::Relaxed);
        timer = end.saturating_sub(now);
        if timer <= 0 {
            TIMER_RUNNING.store(false, Ordering::Relaxed);
            TIMER_EXPIRED.store(true, Ordering::Relaxed);
            timer = 0;
        }
    }
    let alarm_due =
        crate::rtc::valid() && crate::rtc::local_minutes() == ALARM_MINUTES.load(Ordering::Relaxed);
    if !alarm_due {
        ALARM_LATCHED.store(false, Ordering::Relaxed);
    } else if ALARM_ENABLED.load(Ordering::Relaxed) && !ALARM_LATCHED.swap(true, Ordering::Relaxed)
    {
        ALARM_RINGING.store(true, Ordering::Relaxed);
    }
    (
        timer.clamp(0, 3600),
        stopwatch,
        if ALARM_ENABLED.load(Ordering::Relaxed) {
            1
        } else {
            0
        },
    )
}

pub(super) fn alarm_minutes() -> u16 {
    ALARM_MINUTES.load(Ordering::Relaxed) as u16
}

pub(super) fn config() -> i32 {
    ALARM_MINUTES.load(Ordering::Relaxed) | ((ALARM_ENABLED.load(Ordering::Relaxed) as i32) << 11)
}

pub(super) fn restore_config(value: i32) {
    if value < 0 || value & !0xfff != 0 || (value & 0x7ff) >= 1440 {
        return;
    }
    ALARM_MINUTES.store(value & 0x7ff, Ordering::Relaxed);
    ALARM_ENABLED.store(value & 0x800 != 0, Ordering::Relaxed);
    ALARM_RINGING.store(false, Ordering::Relaxed);
    ALARM_LATCHED.store(false, Ordering::Relaxed);
}
pub(super) fn alert() -> u8 {
    if ALARM_RINGING.load(Ordering::Relaxed) {
        2
    } else if TIMER_EXPIRED.load(Ordering::Relaxed) {
        1
    } else {
        0
    }
}
