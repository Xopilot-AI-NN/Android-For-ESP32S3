use esp_hal::gpio::{Input, InputConfig, InputPin, Pull};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootMode { Normal, Fastboot, Recovery }

pub struct BootKeys<'d> {
    encoder_a: Input<'d>,
    encoder_b: Input<'d>,
    power: Input<'d>,
    encoder_state: u8,
}

impl<'d> BootKeys<'d> {
    pub fn new(
        a: impl InputPin + 'd,
        b: impl InputPin + 'd,
        power: impl InputPin + 'd,
    ) -> Self {
        let encoder_a = Input::new(a, InputConfig::default().with_pull(Pull::Up));
        let encoder_b = Input::new(b, InputConfig::default().with_pull(Pull::Up));
        let power = Input::new(power, InputConfig::default().with_pull(Pull::Up));
        let encoder_state = ((encoder_a.is_low() as u8) << 1) | encoder_b.is_low() as u8;
        Self { encoder_a, encoder_b, power, encoder_state }
    }

    pub fn power_pressed(&mut self) -> bool { self.power.is_low() }

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
