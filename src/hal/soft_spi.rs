// Модуль программного SPI дисплея.
//! Software SPI mode 0: SCK=GPIO1, MOSI=GPIO2 on the ESP32-S3.
//! This is a diagnostic/compatibility alternative to the SPI3 GPIO matrix.
//! Existing ST7789 commands, reset, CS and data/command handling are reused.

use embedded_hal::spi::{Error as SpiError, ErrorKind, ErrorType, SpiBus};
use esp_hal::gpio::Output;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Ошибки операций компонента программного SPI дисплея.
pub enum SoftSpiError {
    UnsupportedRead,
}

impl SpiError for SoftSpiError {
    /// Возвращает вид транспортной ошибки.
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

/// Структура данных компонента программного SPI дисплея.
pub struct SoftSpi<'d> {
    /// Тактовая линия программного SPI.
    sck: Output<'d>,
    /// Линия данных от ведущего SPI к устройству.
    mosi: Output<'d>,
}

impl<'d> SoftSpi<'d> {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
    pub fn new(mut sck: Output<'d>, mut mosi: Output<'d>) -> Self {
        sck.set_low();
        mosi.set_low();
        Self { sck, mosi }
    }
}

impl ErrorType for SoftSpi<'_> {
    /// Причины отказа операций компонента программного SPI дисплея.
    type Error = SoftSpiError;
}

impl SpiBus<u8> for SoftSpi<'_> {
    /// Считывает запрошенные данные через интерфейс текущего транспорта.
    fn read(&mut self, _words: &mut [u8]) -> Result<(), Self::Error> {
        Err(SoftSpiError::UnsupportedRead)
    }

    /// Передаёт данные через интерфейс текущего транспорта.
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

    /// Передаёт выходные байты и одновременно считывает входные.
    fn transfer(&mut self, _read: &mut [u8], _write: &[u8]) -> Result<(), Self::Error> {
        Err(SoftSpiError::UnsupportedRead)
    }

    /// Выполняет полнодуплексный обмен, заменяя исходный буфер принятыми байтами.
    fn transfer_in_place(&mut self, _words: &mut [u8]) -> Result<(), Self::Error> {
        Err(SoftSpiError::UnsupportedRead)
    }

    /// Завершает отложенную передачу данных.
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
