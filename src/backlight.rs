//! Constant GPIO backlight. No PWM is applied to the user's panel.
//! Intermediate brightness is applied to RGB pixels by the display driver.
use core::convert::Infallible;
use embedded_hal::digital::{ErrorType, OutputPin};
use esp_hal::gpio::Output;
pub struct DcBacklight<'a> {
    pin: Output<'a>,
    on: bool,
}
impl<'a> DcBacklight<'a> {
    pub fn new(pin: Output<'a>) -> Self {
        Self { pin, on: false }
    }
}
impl ErrorType for DcBacklight<'_> {
    type Error = Infallible;
}
impl OutputPin for DcBacklight<'_> {
    fn set_low(&mut self) -> Result<(), Infallible> {
        self.set_brightness(0)
    }
    fn set_high(&mut self) -> Result<(), Infallible> {
        self.set_brightness(100)
    }
}
impl crate::display::Backlight for DcBacklight<'_> {
    fn software_dimming(&self) -> bool {
        true
    }
    fn set_brightness(&mut self, percent: u8) -> Result<(), Infallible> {
        let on = percent != 0;
        if on != self.on {
            if on {
                self.pin.set_high();
            } else {
                self.pin.set_low();
            }
            self.on = on;
        }
        Ok(())
    }
}
use crate::display::Backlight;
