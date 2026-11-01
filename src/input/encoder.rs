//! Convert the hardware quadrature counter to stable crown detents.
#[derive(Default)]
pub struct EncoderSteps {
    last: i16,
    pending: i32,
    sensitivity: u8,
}
impl EncoderSteps {
    pub fn new() -> Self {
        Self {
            sensitivity: 1,
            ..Self::default()
        }
    }
    pub fn sensitivity(&mut self, value: u8) {
        let value = value.min(2);
        if value != self.sensitivity {
            self.pending = 0;
            self.sensitivity = value;
        }
    }
    pub fn poll(&mut self, count: i16) -> i8 {
        self.pending += i32::from(count.wrapping_sub(self.last));
        self.last = count;
        let divisor = [8, 4, 2][self.sensitivity as usize];
        let steps = (self.pending / divisor).clamp(-127, 127);
        self.pending -= steps * divisor;
        steps as i8
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounce_wrap_and_frames_do_not_reverse_steps() {
        let mut e = EncoderSteps::new();
        for n in [1, 0, 1, 2, 1, 2, 3] {
            assert_eq!(e.poll(n), 0);
        }
        assert_eq!(e.poll(4), 1);
        assert_eq!(e.poll(16), 3); // Captured while rendering a frame.
        assert_eq!(e.poll(12), -1);
        e.last = i16::MAX - 1;
        assert_eq!(e.poll(i16::MIN + 2), 1);
        e.sensitivity(0);
        assert_eq!(e.poll(i16::MIN + 6), 0);
        assert_eq!(e.poll(i16::MIN + 10), 1);
    }
    #[test]
    fn each_sensitivity_has_symmetric_stable_steps() {
        for (level, edges) in [(0, 8), (1, 4), (2, 2)] {
            let mut e = EncoderSteps::new();
            e.sensitivity(level);
            for count in 1..edges {
                assert_eq!(e.poll(count), 0);
            }
            assert_eq!(e.poll(edges), 1);
            for count in (1..edges).rev() {
                assert_eq!(e.poll(count), 0);
            }
            assert_eq!(e.poll(0), -1);
        }
    }
}
