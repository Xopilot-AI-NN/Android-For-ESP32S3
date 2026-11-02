//! Monotonic display policy: a dim stage precedes real screen-off.
/// Idle pages need second ticks, with short updates only during the dim fade.
pub fn next_refresh(now_ms: u64, last_input_ms: u64, timeout_secs: u16) -> u64 {
    let off = last_input_ms.saturating_add(u64::from(timeout_secs) * 1000);
    let dim = off.saturating_sub(10_000);
    let second = now_ms.saturating_add(1000 - now_ms % 1000);
    if now_ms < dim {
        second.min(dim)
    } else if now_ms < dim.saturating_add(500) {
        now_ms.saturating_add(100).min(off)
    } else if now_ms < off {
        second.min(off)
    } else {
        second
    }
}
pub fn level(selected: u8, idle_ms: u64, timeout_secs: u16, attention: bool) -> u8 {
    let selected = selected.clamp(5, 100);
    if attention {
        return selected;
    }
    let off = u64::from(timeout_secs) * 1000;
    if idle_ms >= off {
        return 0;
    }
    let dim = off.saturating_sub(10_000);
    if idle_ms < dim {
        return selected;
    }
    let target = selected.min(15);
    let fade = (idle_ms - dim).min(500) as u16;
    (u16::from(selected) - (u16::from(selected - target) * fade / 500)) as u8
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thirty_seconds_includes_ten_seconds_dimmed() {
        assert_eq!(level(70, 19_999, 30, false), 70);
        assert_eq!(level(70, 20_500, 30, false), 15);
        assert_eq!(level(70, 29_999, 30, false), 15);
        assert_eq!(level(70, 30_000, 30, false), 0);
        assert_eq!(level(70, 30_000, 30, true), 70);
    }
    #[test]
    fn idle_ticks_slow_down_without_missing_fractional_dim_and_off_deadlines() {
        assert_eq!(next_refresh(5000, 123, 30), 6000);
        assert_eq!(next_refresh(20_000, 123, 30), 20_123);
        assert_eq!(next_refresh(20_123, 123, 30), 20_223);
        assert_eq!(next_refresh(20_723, 123, 30), 21_000);
        assert_eq!(next_refresh(30_000, 123, 30), 30_123);
        assert_eq!(next_refresh(30_123, 123, 30), 31_000);
    }
}
