//! Hardware PWM for the ST7789 BLK pin (GPIO6).
use core::convert::Infallible;
use embedded_hal::digital::{ErrorType, OutputPin};
use esp_hal::ledc::{
    LowSpeed,
    channel::{Channel, ChannelIFace},
};

pub struct PwmBacklight<'a>(pub Channel<'a, LowSpeed>);
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
        self.0
            .set_duty(percent.min(100))
            .expect("configured PWM duty");
        Ok(())
    }
}
use crate::display::Backlight;
