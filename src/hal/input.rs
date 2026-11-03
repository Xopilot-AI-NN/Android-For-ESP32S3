// Модуль энкодера, кнопки и выбора режима загрузки.
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

pub use crate::boot_policy::BootMode;

/// Аппаратное чтение колёсика и кнопки компонента энкодера, кнопки и выбора режима загрузки.
pub struct BootKeys<'d> {
    _encoder_a: Input<'d>,
    _encoder_b: Input<'d>,
    power: Input<'d>,
    /// Аппаратный счётчик фронтов энкодера.
    pcnt: Pcnt<'d>,
    /// Накопитель устойчивых шагов колёсика.
    steps: encoder::EncoderSteps,
    /// Аппаратный/временной обработчик кнопки.
    button: ButtonState,
    last_gesture: Option<(u64, i8)>,
}

impl<'d> BootKeys<'d> {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
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
            last_gesture: None,
        }
    }

    /// Проверяет состояние кнопки питания/колёсика.
    pub fn power_pressed(&mut self) -> bool {
        self.power.is_low()
    }

    /// Поглощает кнопку, которой пользователь вошёл в сервисный режим.
    /// Её отпускание не должно немедленно активировать первую строку меню.
    pub fn consume_boot_press(&mut self) {
        if self.power.is_low() {
            let now = esp_hal::time::Instant::now().duration_since_epoch().as_millis();
            let _ = self.button.poll(true, now);
            self.button.consume_hold();
        }
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
                ButtonAction::Power => crate::runtime::RuntimeEvent::Power,
            })
    }

    /// Применяет чувствительность аппаратного колёсика.
    pub fn set_sensitivity(&mut self, value: u8) {
        self.steps.sensitivity(value);
    }
    /// Sample button and crown together so a chord cannot also activate a row.
    pub fn poll_runtime_events(
        &mut self,
        reverse_gesture: bool,
    ) -> [Option<crate::runtime::RuntimeEvent>; 2] {
        use crate::runtime::RuntimeEvent;
        let now = esp_hal::time::Instant::now()
            .duration_since_epoch()
            .as_millis();
        let raw_pressed = self.power.is_low();
        let action = self.button.poll(raw_pressed, now);
        let rotation = self.poll_rotation();
        if raw_pressed || self.button.is_pressed() {
            if rotation != 0 {
                self.button.consume_hold();
                let direction = if reverse_gesture {
                    -rotation.signum()
                } else {
                    rotation.signum()
                };
                let ready = self
                    .last_gesture
                    .map(|(at, old)| old != direction || now.saturating_sub(at) >= 180)
                    .unwrap_or(true);
                if ready {
                    self.last_gesture = Some((now, direction));
                    return [Some(RuntimeEvent::Swipe(i32::from(direction) * 2)), None];
                }
            }
            return [None, None];
        }
        self.last_gesture = None;
        let button = action.map(|action| match action {
            ButtonAction::Press => RuntimeEvent::Press,
            ButtonAction::Back => RuntimeEvent::Back,
            ButtonAction::Home => RuntimeEvent::Home,
            ButtonAction::Power => RuntimeEvent::Power,
        });
        [
            if rotation != 0 {
                Some(RuntimeEvent::Rotate(i32::from(rotation)))
            } else {
                None
            },
            button,
        ]
    }
    /// Читает аппаратный счётчик и выдаёт устойчивое направление шага.
    pub fn poll_rotation(&mut self) -> i8 {
        self.steps.poll(self.pcnt.unit0.counter.get())
    }
}
