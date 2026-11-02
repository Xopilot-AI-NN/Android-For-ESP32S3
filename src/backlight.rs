//! Hardware PWM for the ST7789 BLK pin (GPIO6).
use core::convert::Infallible;
use embedded_hal::digital::{ErrorType, OutputPin};
use esp_hal::ledc::{
    LowSpeed,
    channel::{Channel, ChannelIFace},
};

pub struct PwmBacklight<'a> {
    channel: Channel<'a, LowSpeed>,
    last: Option<u8>,
}
impl<'a> PwmBacklight<'a> {
    pub fn new(channel: Channel<'a, LowSpeed>) -> Self {
        Self {
            channel,
            last: Some(0),
        }
    }
}
impl ErrorType for PwmBacklight<'_> {
    type Error = Infallible;
}
impl OutputPin for PwmBacklight<'_> {
    fn set_low(&mut self) -> Result<(), Infallible> {
        self.set_brightness(0)
    }
    fn set_high(&mut self) -> Result<(), Infallible> {
        self.set_brightness(100)
    }
}
impl crate::display::Backlight for PwmBacklight<'_> {
    fn set_brightness(&mut self, percent: u8) -> Result<(), Infallible> {
        // Configured once at boot; the bounded duty is always valid.
        let percent = percent.min(100);
        if self.last == Some(percent) {
            return Ok(());
        }
        self.channel.set_duty(percent).expect("configured PWM duty");
        self.last = Some(percent);
        Ok(())
    }
}
use crate::display::Backlight;
