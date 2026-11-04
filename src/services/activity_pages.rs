//! Bounded cache of the 32 activity cursors. Settings and document pages bypass it.
const CURSOR_MASK: i32 = 0x1e0;
pub struct ActivityPages {
    values: [i32; 32],
    known: u32,
    present: u32,
}
impl ActivityPages {
    pub const fn new() -> Self {
        Self {
            values: [0; 32],
            known: 0,
            present: 0,
        }
    }
    /// Outer None means not read yet; inner None means absent/corrupt on disk.
    pub fn get(&self, screen: u32) -> Option<Option<i32>> {
        if screen >= 32 || self.known & (1 << screen) == 0 {
            return None;
        }
        Some((self.present & (1 << screen) != 0).then_some(self.values[screen as usize]))
    }
    pub fn remember(&mut self, screen: u32, value: Option<i32>) {
        if screen >= 32 {
            return;
        }
        self.known |= 1 << screen;
        if let Some(value) = value {
            self.values[screen as usize] = value & CURSOR_MASK;
            self.present |= 1 << screen;
        } else {
            self.present &= !(1 << screen);
        }
    }
    pub fn needs_write(&self, screen: u32, state: i32) -> bool {
        self.get(screen) != Some(Some(state & CURSOR_MASK))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchanged_cursors_ignore_global_flags_and_misses_are_cached() {
        let mut cache = ActivityPages::new();
        assert_eq!(cache.get(5), None);
        cache.remember(5, None);
        assert_eq!(cache.get(5), Some(None));
        assert!(cache.needs_write(5, 5 | 64));
        cache.remember(5, Some(5 | 64 | (1 << 22)));
        assert!(!cache.needs_write(5, 5 | 64 | (1 << 23)));
        assert!(cache.needs_write(5, 5 | 96));
        assert_eq!(cache.get(5), Some(Some(64)));
        assert_eq!(cache.get(32), None);
        cache.remember(32, Some(1));
    }
}
