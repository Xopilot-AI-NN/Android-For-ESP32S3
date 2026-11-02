//! Monotonic display policy: a dim stage precedes real screen-off.
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
}
