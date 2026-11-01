//! Software SPI mode 0: SCK=GPIO1, MOSI=GPIO2 on the ESP32-S3.
//! This is a diagnostic/compatibility alternative to the SPI3 GPIO matrix.
//! Existing ST7789 commands, reset, CS and data/command handling are reused.

use embedded_hal::spi::{Error as SpiError, ErrorKind, ErrorType, SpiBus};
use esp_hal::gpio::Output;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftSpiError {
    UnsupportedRead,
}

impl SpiError for SoftSpiError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

pub struct SoftSpi<'d> {
    sck: Output<'d>,
    mosi: Output<'d>,
}

impl<'d> SoftSpi<'d> {
    pub fn new(mut sck: Output<'d>, mut mosi: Output<'d>) -> Self {
        sck.set_low();
        mosi.set_low();
        Self { sck, mosi }
    }
}

impl ErrorType for SoftSpi<'_> {
    type Error = SoftSpiError;
}

impl SpiBus<u8> for SoftSpi<'_> {
    fn read(&mut self, _words: &mut [u8]) -> Result<(), Self::Error> {
        Err(SoftSpiError::UnsupportedRead)
    }

    fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        for &byte in words {
            let mut mask = 0x80u8;
            while mask != 0 {
                // Mode 0: SCK is idle low, data is stable before rising edge.
                if byte & mask != 0 {
                    self.mosi.set_high();
                } else {
                    self.mosi.set_low();
                }
                core::hint::spin_loop();
                self.sck.set_high();
                core::hint::spin_loop();
                self.sck.set_low();
                mask >>= 1;
            }
        }
        Ok(())
    }

    fn transfer(&mut self, _read: &mut [u8], _write: &[u8]) -> Result<(), Self::Error> {
        Err(SoftSpiError::UnsupportedRead)
    }

    fn transfer_in_place(&mut self, _words: &mut [u8]) -> Result<(), Self::Error> {
        Err(SoftSpiError::UnsupportedRead)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
