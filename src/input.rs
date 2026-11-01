#[path = "input/button.rs"]
mod button;
use button::{ButtonAction, ButtonState};
use esp_hal::gpio::{Input, InputConfig, InputPin, Pull};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(not(feature = "pc-block-boot"))]
pub enum BootMode {
    Normal,
    Fastboot,
    Recovery,
}

pub struct BootKeys<'d> {
    encoder_a: Input<'d>,
    encoder_b: Input<'d>,
    power: Input<'d>,
    encoder_state: u8,
    button: ButtonState,
}

impl<'d> BootKeys<'d> {
    pub fn new(a: impl InputPin + 'd, b: impl InputPin + 'd, power: impl InputPin + 'd) -> Self {
        let encoder_a = Input::new(a, InputConfig::default().with_pull(Pull::Up));
        let encoder_b = Input::new(b, InputConfig::default().with_pull(Pull::Up));
        let power = Input::new(power, InputConfig::default().with_pull(Pull::Up));
        let encoder_state = ((encoder_a.is_low() as u8) << 1) | encoder_b.is_low() as u8;
        Self {
            encoder_a,
            encoder_b,
            power,
            encoder_state,
            button: ButtonState::default(),
        }
    }

    #[cfg(not(feature = "pc-block-boot"))]
    pub fn power_pressed(&mut self) -> bool {
        self.power.is_low()
    }

    /// Debounced release actions use elapsed time, independent of SPI/UI latency.
    pub fn poll_button_event(&mut self) -> Option<crate::runtime::RuntimeEvent> {
        let now = esp_hal::time::Instant::now()
            .duration_since_epoch()
            .as_millis();
        self.button
            .poll(self.power.is_low(), now)
            .map(|action| match action {
                ButtonAction::Press => crate::runtime::RuntimeEvent::Press,
                ButtonAction::Back => crate::runtime::RuntimeEvent::Back,
                ButtonAction::Home => crate::runtime::RuntimeEvent::Home,
            })
    }

    pub fn poll_rotation(&mut self) -> i8 {
        let state = ((self.encoder_a.is_low() as u8) << 1) | self.encoder_b.is_low() as u8;
        let transition = (self.encoder_state << 2) | state;
        self.encoder_state = state;
        match transition {
            0b0001 | 0b0111 | 0b1110 | 0b1000 => 1,
            0b0010 | 0b0100 | 0b1101 | 0b1011 => -1,
            _ => 0,
        }
    }
}
