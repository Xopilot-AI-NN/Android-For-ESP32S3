//! Diagnostic full-duplex mode-0 SPI; keeps the existing SD protocol/CRC checks.
use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
    spi::{ErrorKind, ErrorType, SpiBus},
};
pub struct SdBitBang<C, O, I, D> {
    clock: C,
    mosi: O,
    miso: I,
    delay: D,
}
impl<C: OutputPin, O: OutputPin, I: InputPin, D: DelayNs> SdBitBang<C, O, I, D> {
    pub fn new(mut clock: C, mut mosi: O, miso: I, delay: D) -> Result<Self, ErrorKind> {
        clock.set_low().map_err(|_| ErrorKind::Other)?;
        mosi.set_high().map_err(|_| ErrorKind::Other)?;
        Ok(Self {
            clock,
            mosi,
            miso,
            delay,
        })
    }
    fn byte(&mut self, out: u8) -> Result<u8, ErrorKind> {
        let mut received = 0;
        for shift in (0..8).rev() {
            self.mosi
                .set_state((out & (1 << shift) != 0).into())
                .map_err(|_| ErrorKind::Other)?;
            self.delay.delay_us(5);
            self.clock.set_high().map_err(|_| ErrorKind::Other)?;
            self.delay.delay_us(5);
            let bit = self.miso.is_high().map_err(|_| ErrorKind::Other);
            // Always restore idle low, including a failed input read.
            self.clock.set_low().map_err(|_| ErrorKind::Other)?;
            received = (received << 1) | u8::from(bit?);
        }
        Ok(received)
    }
}
impl<C: OutputPin, O: OutputPin, I: InputPin, D: DelayNs> ErrorType for SdBitBang<C, O, I, D> {
    type Error = ErrorKind;
}
impl<C: OutputPin, O: OutputPin, I: InputPin, D: DelayNs> SpiBus<u8> for SdBitBang<C, O, I, D> {
    fn read(&mut self, b: &mut [u8]) -> Result<(), Self::Error> {
        for x in b {
            *x = self.byte(0xff)?;
        }
        Ok(())
    }
    fn write(&mut self, b: &[u8]) -> Result<(), Self::Error> {
        for &x in b {
            self.byte(x)?;
        }
        Ok(())
    }
    fn transfer(&mut self, r: &mut [u8], w: &[u8]) -> Result<(), Self::Error> {
        for i in 0..r.len().max(w.len()) {
            let x = self.byte(w.get(i).copied().unwrap_or(0xff))?;
            if let Some(dst) = r.get_mut(i) {
                *dst = x;
            }
        }
        Ok(())
    }
    fn transfer_in_place(&mut self, b: &mut [u8]) -> Result<(), Self::Error> {
        for x in b {
            *x = self.byte(*x)?;
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{cell::RefCell, convert::Infallible};
    use std::rc::Rc;
    #[derive(Default)]
    struct Wire {
        clock: bool,
        mosi: bool,
        sent: std::vec::Vec<bool>,
        read: usize,
        us: u32,
    }
    struct Clock(Rc<RefCell<Wire>>);
    struct Mosi(Rc<RefCell<Wire>>);
    struct Miso(Rc<RefCell<Wire>>);
    struct Delay(Rc<RefCell<Wire>>);
    impl embedded_hal::digital::ErrorType for Clock {
        type Error = Infallible;
    }
    impl embedded_hal::digital::ErrorType for Mosi {
        type Error = Infallible;
    }
    impl embedded_hal::digital::ErrorType for Miso {
        type Error = Infallible;
    }
    impl OutputPin for Clock {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.0.borrow_mut().clock = false;
            Ok(())
        }
        fn set_high(&mut self) -> Result<(), Self::Error> {
            let mut w = self.0.borrow_mut();
            assert!(!w.clock);
            w.clock = true;
            let bit = w.mosi;
            w.sent.push(bit);
            Ok(())
        }
    }
    impl OutputPin for Mosi {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            let mut w = self.0.borrow_mut();
            assert!(!w.clock);
            w.mosi = false;
            Ok(())
        }
        fn set_high(&mut self) -> Result<(), Self::Error> {
            let mut w = self.0.borrow_mut();
            assert!(!w.clock);
            w.mosi = true;
            Ok(())
        }
    }
    impl InputPin for Miso {
        fn is_high(&mut self) -> Result<bool, Self::Error> {
            let mut w = self.0.borrow_mut();
            assert!(w.clock);
            let bit = 0xa5 & (0x80 >> (w.read % 8)) != 0;
            w.read += 1;
            Ok(bit)
        }
        fn is_low(&mut self) -> Result<bool, Self::Error> {
            self.is_high().map(|b| !b)
        }
    }
    impl DelayNs for Delay {
        fn delay_ns(&mut self, ns: u32) {
            self.0.borrow_mut().us += ns / 1000;
        }
    }
    #[test]
    fn mode_zero_samples_high_changes_data_low_and_pads_reads_with_ones() {
        let w = Rc::new(RefCell::new(Wire::default()));
        let mut spi = SdBitBang::new(
            Clock(w.clone()),
            Mosi(w.clone()),
            Miso(w.clone()),
            Delay(w.clone()),
        )
        .unwrap();
        let mut received = [0; 2];
        spi.transfer(&mut received, &[0x77]).unwrap();
        assert_eq!(received, [0xa5, 0xa5]);
        let w = w.borrow();
        assert!(!w.clock);
        assert_eq!(w.us, 160);
        assert_eq!(
            w.sent,
            [
                false, true, true, true, false, true, true, true, true, true, true, true, true,
                true, true, true
            ]
        );
    }
}
