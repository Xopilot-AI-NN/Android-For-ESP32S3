// Модуль преобразования PCNT в шаги колёсика.
//! Convert the hardware quadrature counter to stable crown detents.
#[derive(Default)]
pub struct EncoderSteps {
    /// Последнее принятое значение для сравнения с новым.
    last: i16,
    /// Необработанный запрос или ожидающий результат.
    pending: i32,
    /// Уровень чувствительности преобразования фронтов в шаги.
    sensitivity: u8,
}
impl EncoderSteps {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
    pub fn new() -> Self {
        Self {
            sensitivity: 1,
            ..Self::default()
        }
    }
    /// Меняет чувствительность и очищает накопленные неполные шаги.
    pub fn sensitivity(&mut self, value: u8) {
        let value = value.min(2);
        if value != self.sensitivity {
            self.pending = 0;
            self.sensitivity = value;
        }
    }
    /// Выполняет очередной неблокирующий шаг обработки и проверяет готовые результаты.
    pub fn poll(&mut self, count: i16) -> i8 {
        let delta = i32::from(count.wrapping_sub(self.last));
        self.last = count;
        let divisor = [4, 2, 2][self.sensitivity as usize];
        // Cancel unrendered motion when the user reverses, but retain sub-step
        // bounce cancellation. Never replay old positive steps after a reversal.
        if self.pending.abs() >= divisor && delta != 0 && self.pending.signum() != delta.signum() {
            self.pending = 0;
        }
        self.pending += delta;
        let limit = [1, 1, 2][self.sensitivity as usize];
        let steps = (self.pending / divisor).clamp(-limit, limit);
        self.pending -= steps * divisor;
        steps as i8
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Проверяет дребезг, переполнение PCNT и направление шагов во время отрисовки.
    fn bounce_wrap_and_frames_do_not_reverse_steps() {
        let mut e = EncoderSteps::new();
        e.sensitivity(0);
        for n in [1, 0, 1, 2, 1, 2, 3] {
            assert_eq!(e.poll(n), 0);
        }
        assert_eq!(e.poll(4), 1);
        assert_eq!(e.poll(16), 1); // Captured while rendering; drain smoothly.
        assert_eq!(e.poll(16), 1);
        assert_eq!(e.poll(16), 1);
        assert_eq!(e.poll(12), -1);
        e.last = i16::MAX - 1;
        assert_eq!(e.poll(i16::MIN + 2), 1);
        e.sensitivity(0);
        assert_eq!(e.poll(i16::MIN + 4), 0);
        assert_eq!(e.poll(i16::MIN + 6), 1);
    }
    #[test]
    /// Проверяет симметричные устойчивые шаги на каждом уровне чувствительности.
    fn each_sensitivity_has_symmetric_stable_steps() {
        for (level, edges) in [(0, 4), (1, 2), (2, 2)] {
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
    #[test]
    /// Проверяет ограничение быстрого вращения и немедленное изменение направления.
    fn fast_motion_is_bounded_and_reverse_is_immediate() {
        let mut e = EncoderSteps::new();
        e.sensitivity(2);
        assert_eq!(e.poll(20), 2);
        assert_eq!(e.poll(20), 2);
        assert_eq!(e.poll(16), -2);
        assert_eq!(e.poll(16), 0);
    }
}
