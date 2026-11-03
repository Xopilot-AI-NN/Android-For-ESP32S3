// Логика приложения Counter: события энкодера и вычисления.
use crate::apps::utilities::*;
use crate::runtime::RuntimeEvent;
impl Tools {
    /// Изменяет ручной счётчик и обрабатывает подтверждение сброса.
    pub fn counter_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Rotate(d) => {
                self.counter_action =
                    (i32::from(self.counter_action) + d.rem_euclid(3)).rem_euclid(3) as u8;
                self.reset_armed = false;
            }
            RuntimeEvent::Press => match self.counter_action {
                0 => {
                    self.count = self.count.saturating_add(1).min(999_999);
                }
                1 => {
                    self.count = self.count.saturating_sub(1);
                }
                _ if self.reset_armed => {
                    self.count = 0;
                    self.reset_armed = false;
                }
                _ => self.reset_armed = true,
            },
            _ => {}
        }
    }

}
