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

pub(super) fn clock_snapshot() -> (i32, i32, i32) {
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
