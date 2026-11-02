#[path = "input/button.rs"]
mod button;
use button::{ButtonAction, ButtonState};
#[path = "input/encoder.rs"]
mod encoder;
use esp_hal::gpio::{Input, InputConfig, InputPin, Pull};
use esp_hal::pcnt::{
    Pcnt,
    channel::{CtrlMode, EdgeMode},
};
use esp_hal::peripherals::PCNT;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(not(feature = "pc-block-boot"))]
pub enum BootMode {
    Normal,
    Fastboot,
    Recovery,
}

pub struct BootKeys<'d> {
    _encoder_a: Input<'d>,
    _encoder_b: Input<'d>,
    power: Input<'d>,
    pcnt: Pcnt<'d>,
    steps: encoder::EncoderSteps,
    button: ButtonState,
}

impl<'d> BootKeys<'d> {
    pub fn new(
        peripheral: PCNT<'d>,
        a: impl InputPin + 'd,
        b: impl InputPin + 'd,
        power: impl InputPin + 'd,
    ) -> Self {
        let encoder_a = Input::new(a, InputConfig::default().with_pull(Pull::Up));
        let encoder_b = Input::new(b, InputConfig::default().with_pull(Pull::Up));
        let power = Input::new(power, InputConfig::default().with_pull(Pull::Up));
        let pcnt = Pcnt::new(peripheral);
        let unit = &pcnt.unit0;
        unit.set_low_limit(None).unwrap();
        unit.set_high_limit(None).unwrap();
        unit.set_filter(Some(800)).unwrap();
        let a = encoder_a.peripheral_input();
        let b = encoder_b.peripheral_input();
        unit.channel0.set_ctrl_signal(a.clone());
        unit.channel0.set_edge_signal(b.clone());
        unit.channel0
            .set_ctrl_mode(CtrlMode::Reverse, CtrlMode::Keep);
        unit.channel0
            .set_input_mode(EdgeMode::Increment, EdgeMode::Decrement);
        unit.channel1.set_ctrl_signal(b);
        unit.channel1.set_edge_signal(a);
        unit.channel1
            .set_ctrl_mode(CtrlMode::Reverse, CtrlMode::Keep);
        unit.channel1
            .set_input_mode(EdgeMode::Decrement, EdgeMode::Increment);
        unit.clear();
        unit.resume();
        Self {
            _encoder_a: encoder_a,
            _encoder_b: encoder_b,
            power,
            pcnt,
            steps: encoder::EncoderSteps::new(),
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

    pub fn set_sensitivity(&mut self, value: u8) {
        self.steps.sensitivity(value);
    }
    pub fn poll_rotation(&mut self) -> i8 {
        self.steps.poll(self.pcnt.unit0.counter.get())
    }
}
