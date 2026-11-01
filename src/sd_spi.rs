//! Session adapter for embedded-sdmmc 0.10 on the dedicated SPI2 bus.
//!
//! The driver splits commands, R1 polling and data into separate SpiDevice
//! calls. A conventional ExclusiveDevice releases CS between these fragments.
//! This adapter owns the whole bus and keeps the SD session selected instead.
//! It must never be used on a shared bus or as a general-purpose SPI device.
use embedded_hal::{
    delay::DelayNs,
    digital::OutputPin,
    spi::{ErrorType, Operation, SpiBus, SpiDevice},
};
use embedded_hal_bus::spi::DeviceError;

pub struct SdSpiSession<B: SpiBus<u8>, C: OutputPin, D: DelayNs> {
    bus: B,
    cs: C,
    delay: D,
    selected: bool,
}

impl<B: SpiBus<u8>, C: OutputPin, D: DelayNs> SdSpiSession<B, C, D> {
    /// Caller has already sent the power-up clocks with CS high at <=400 kHz.
    pub fn new(bus: B, cs: C, delay: D) -> Self {
        Self {
            bus,
            cs,
            delay,
            selected: false,
        }
    }

    /// Start a complete SD command, not an individual response byte.
    pub fn begin_command(&mut self) -> Result<(), DeviceError<B::Error, C::Error>> {
        self.finish_command()?;
        self.select()
    }
    /// Finish the response with eight selected clocks, then release CS and
    /// clock DO to high impedance. Idle waits must occur with the card released.
    pub fn finish_command(&mut self) -> Result<(), DeviceError<B::Error, C::Error>> {
        let flush = self.bus.flush();
        let trailer = if flush.is_ok() && self.selected {
            self.bus.write(&[0xff]).and_then(|()| self.bus.flush())
        } else {
            Ok(())
        };
        // Cleanup must release CS even if the bus operation failed.
        let cs = self.cs.set_high();
        self.selected = false;
        let idle = self.bus.write(&[0xff]).and_then(|()| self.bus.flush());
        flush.map_err(DeviceError::Spi)?;
        trailer.map_err(DeviceError::Spi)?;
        cs.map_err(DeviceError::Cs)?;
        idle.map_err(DeviceError::Spi)
    }
    #[cfg(not(feature = "sd-software-spi"))]
    pub fn bus_mut(&mut self) -> &mut B {
        &mut self.bus
    }

    fn select(&mut self) -> Result<(), DeviceError<B::Error, C::Error>> {
        if !self.selected {
            self.bus.flush().map_err(DeviceError::Spi)?;
            self.cs.set_low().map_err(DeviceError::Cs)?;
            self.selected = true;
        }
        Ok(())
    }

    /// Release only after the session ends or a transport error, not per byte.
    fn release(&mut self) {
        let _ = self.finish_command();
    }
}

impl<B: SpiBus<u8>, C: OutputPin, D: DelayNs> ErrorType for SdSpiSession<B, C, D> {
    type Error = DeviceError<B::Error, C::Error>;
}

impl<B: SpiBus<u8>, C: OutputPin, D: DelayNs> SpiDevice<u8> for SdSpiSession<B, C, D> {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        self.select()?;
        let result = operations.iter_mut().try_for_each(|op| match op {
            Operation::Read(bytes) => {
                // SD requires MOSI high while clocking response/data bytes.
                bytes.fill(0xff);
                self.bus.transfer_in_place(bytes)
            }
            Operation::Write(bytes) => self.bus.write(bytes),
            Operation::Transfer(read, write) => self.bus.transfer(read, write),
            Operation::TransferInPlace(bytes) => self.bus.transfer_in_place(bytes),
            Operation::DelayNs(ns) => {
                self.bus.flush()?;
                self.delay.delay_ns(*ns);
                Ok(())
            }
        });
        // esp-hal SpiBus::write may leave the last bytes in flight.
        let result = result
            .and_then(|()| self.bus.flush())
            .map_err(DeviceError::Spi);
        if result.is_err() {
            self.release();
        }
        result
    }
}

impl<B: SpiBus<u8>, C: OutputPin, D: DelayNs> Drop for SdSpiSession<B, C, D> {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{cell::RefCell, convert::Infallible};
    use std::rc::Rc;

    #[derive(Default)]
    struct Card {
        selected: bool,
        pending: bool,
        response: bool,
        fail: bool,
        released_clock: bool,
    }
    struct Bus(Rc<RefCell<Card>>);
    struct Cs(Rc<RefCell<Card>>);
    struct Delay;
    impl DelayNs for Delay {
        fn delay_ns(&mut self, _: u32) {}
    }
    impl embedded_hal::digital::ErrorType for Cs {
        type Error = Infallible;
    }
    impl OutputPin for Cs {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.0.borrow_mut().selected = true;
            Ok(())
        }
        fn set_high(&mut self) -> Result<(), Self::Error> {
            let mut c = self.0.borrow_mut();
            assert!(!c.pending, "CS changed while clocks were still in flight");
            c.selected = false;
            c.response = false;
            Ok(())
        }
    }
    impl ErrorType for Bus {
        type Error = embedded_hal::spi::ErrorKind;
    }
    impl SpiBus<u8> for Bus {
        fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
            let mut c = self.0.borrow_mut();
            if c.fail {
                return Err(Self::Error::Other);
            }
            c.pending = true;
            if c.selected && bytes == [0x69, 0x40, 0, 0, 0, 0x77] {
                c.response = true;
            }
            if !c.selected && bytes == [0xff] {
                c.released_clock = true;
            }
            Ok(())
        }
        fn flush(&mut self) -> Result<(), Self::Error> {
            self.0.borrow_mut().pending = false;
            Ok(())
        }
        fn read(&mut self, bytes: &mut [u8]) -> Result<(), Self::Error> {
            bytes.fill(0xff);
            self.transfer_in_place(bytes)
        }
        fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
            assert!(write.iter().all(|&b| b == 0xff));
            read.fill(0xff);
            self.transfer_in_place(read)
        }
        fn transfer_in_place(&mut self, bytes: &mut [u8]) -> Result<(), Self::Error> {
            assert!(bytes.iter().all(|&b| b == 0xff));
            let c = self.0.borrow();
            bytes.fill(if c.selected && c.response { 0x01 } else { 0xff });
            Ok(())
        }
    }

    #[test]
    fn command_response_survives_fragments_and_drop_releases_after_flush() {
        let card = Rc::new(RefCell::new(Card::default()));
        {
            let mut sd = SdSpiSession::new(Bus(card.clone()), Cs(card.clone()), Delay);
            sd.write(&[0x69, 0x40, 0, 0, 0, 0x77]).unwrap();
            let mut r1 = [0xff];
            sd.transfer_in_place(&mut r1).unwrap();
            assert_eq!(r1, [0x01]);
            assert!(card.borrow().selected);
        }
        assert!(!card.borrow().selected);
        assert!(card.borrow().released_clock);
    }

    #[test]
    fn ordinary_device_loses_this_cards_response_and_transport_errors_release_cs() {
        let card = Rc::new(RefCell::new(Card::default()));
        let mut old =
            embedded_hal_bus::spi::ExclusiveDevice::new(Bus(card.clone()), Cs(card.clone()), Delay)
                .unwrap();
        old.write(&[0x69, 0x40, 0, 0, 0, 0x77]).unwrap();
        let mut r1 = [0xff];
        old.transfer_in_place(&mut r1).unwrap();
        assert_eq!(r1, [0xff]);
        let mut sd = SdSpiSession::new(Bus(card.clone()), Cs(card.clone()), Delay);
        card.borrow_mut().fail = true;
        assert!(sd.write(&[0]).is_err());
        assert!(!card.borrow().selected);
    }
}
