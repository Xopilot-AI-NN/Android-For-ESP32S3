//! UI telemetry is subscribed by the viewer, not an idle background log.
#[derive(Default)]
pub struct ViewerStream {
    until_ms: u64,
    last: heapless::Vec<u8, 256>,
}
impl ViewerStream {
    pub fn touch(&mut self, now: u64, sync: bool) {
        if sync || !self.active(now) {
            self.last.clear();
        }
        // The desktop viewer pings every 5 seconds. Allow scheduling jitter.
        self.until_ms = now.saturating_add(15_000);
    }
    pub fn active(&self, now: u64) -> bool {
        now < self.until_ms
    }
    pub fn changed(&mut self, line: &[u8]) -> bool {
        if self.last.as_slice() == line {
            return false;
        }
        self.last.clear();
        let _ = self.last.extend_from_slice(line);
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_idle_stream_duplicates_or_abandoned_viewer_subscription() {
        let mut s = ViewerStream::default();
        assert!(!s.active(0));
        s.touch(100, true);
        assert!(s.active(100));
        assert!(s.changed(b"M3|1|0"));
        assert!(!s.changed(b"M3|1|0"));
        s.touch(5100, false);
        assert!(!s.changed(b"M3|1|0"));
        assert!(s.changed(b"M3|1|1"));
        assert!(!s.active(20_100));
        s.touch(20_101, true);
        assert!(s.changed(b"M3|1|1"));
    }
}
