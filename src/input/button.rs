//! Debounce and duration classification without a dependency on loop frequency.
#[derive(Default)]
pub struct ButtonState {
    raw: bool,
    stable: bool,
    changed_ms: u64,
    pressed_ms: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ButtonAction {
    Press,
    Back,
    Home,
    Power,
}

impl ButtonState {
    pub fn poll(&mut self, pressed: bool, now: u64) -> Option<ButtonAction> {
        if pressed != self.raw {
            self.raw = pressed;
            self.changed_ms = now;
        }
        if now.saturating_sub(self.changed_ms) < 25 || pressed == self.stable {
            return None;
        }
        self.stable = pressed;
        if pressed {
            self.pressed_ms = now;
            return None;
        }
        Some(match now.saturating_sub(self.pressed_ms) {
            3500.. => ButtonAction::Power,
            1800.. => ButtonAction::Home,
            700.. => ButtonAction::Back,
            _ => ButtonAction::Press,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounce_does_not_activate_a_button() {
        let mut b = ButtonState::default();
        for (pressed, time) in [(true, 0), (false, 4), (true, 9), (false, 15), (false, 50)] {
            assert_eq!(b.poll(pressed, time), None);
        }
    }
    #[test]
    fn press_durations_are_independent_of_poll_count() {
        for (held, expected) in [
            (100, ButtonAction::Press),
            (800, ButtonAction::Back),
            (2000, ButtonAction::Home),
        ] {
            let mut b = ButtonState::default();
            assert_eq!(b.poll(true, 0), None);
            assert_eq!(b.poll(true, 25), None);
            assert_eq!(b.poll(false, held), None);
            assert_eq!(b.poll(false, held + 25), Some(expected));
            assert_eq!(b.poll(false, held + 100), None);
        }
    }
}
