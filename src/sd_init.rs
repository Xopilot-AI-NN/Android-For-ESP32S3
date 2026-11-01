//! Checked SPI-mode startup. Leaves filesystem/block I/O to embedded-sdmmc.
use crate::sd_spi::SdSpiSession;
use embedded_hal::{
    delay::DelayNs,
    digital::OutputPin,
    spi::{SpiBus, SpiDevice},
};
use embedded_sdmmc::{SdCard, SdCardError};
use embedded_sdmmc_types::sdcard::CardType;

#[derive(Clone, Copy, Debug)]
pub enum InitError {
    Transport,
    NoResponse { command: u8 },
    Busy { command: u8, last: u8 },
    Response { command: u8, r1: u8 },
    Interface([u8; 4]),
    NotReady { cmd55: u8, acmd41: u8 },
    Ocr([u8; 4]),
    Capacity(SdCardError),
}

impl core::fmt::Display for InitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Transport => f.write_str("SPI transport failure"),
            Self::NoResponse { command } => {
                write!(f, "CMD{command}: no R1 response within 64 bytes")
            }
            Self::Busy { command, last } => write!(f, "CMD{command}: busy, last byte=0x{last:02x}"),
            Self::Response { command, r1 } => write!(
                f,
                "CMD{command}: R1=0x{r1:02x}, illegal={}, CRC error={}",
                r1 & 4 != 0,
                r1 & 8 != 0
            ),
            Self::Interface(r7) => write!(f, "CMD8: invalid voltage/echo R7={r7:02x?}"),
            Self::NotReady { cmd55, acmd41 } => write!(
                f,
                "ACMD41: still idle after 200 polls; CMD55=0x{cmd55:02x}, ACMD41=0x{acmd41:02x}"
            ),
            Self::Ocr(ocr) => write!(f, "CMD58: invalid power/voltage OCR={ocr:02x?}"),
            Self::Capacity(error) => write!(f, "CSD capacity read: {error}"),
        }
    }
}

/// Open only after CMD0/CMD8, ACMD41, OCR and CRC negotiation have succeeded.
pub fn open<B, C, D>(
    mut spi: SdSpiSession<B, C, D>,
    mut delay: D,
) -> Result<SdCard<SdSpiSession<B, C, D>, D>, InitError>
where
    B: SpiBus<u8>,
    C: OutputPin,
    D: DelayNs,
{
    let kind = handshake(&mut spi, &mut delay)?;
    let card = SdCard::new(spi, delay);
    // SAFETY: handshake entered SPI mode, observed ready, verified OCR/addressing,
    // enabled CRC, and set 512-byte block length for byte-addressed cards. The
    // same exclusively-owned SPI device is moved directly into this SdCard.
    unsafe {
        card.mark_card_as_init(kind);
    }
    let bytes = card.num_bytes().map_err(InitError::Capacity)?;
    esp_println::println!(
        "microSD: initialized, {} MiB, {:?}",
        bytes / (1024 * 1024),
        kind
    );
    Ok(card)
}

fn byte<S: SpiDevice<u8>>(spi: &mut S) -> Result<u8, InitError> {
    let mut b = [0xff];
    spi.transfer_in_place(&mut b)
        .map_err(|_| InitError::Transport)?;
    Ok(b[0])
}

/// CRC7 plus the required end bit; required for CMD0/CMD8 even before CRC_ON.
fn command_crc(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &byte in data {
        let mut value = byte;
        for _ in 0..8 {
            crc <<= 1;
            if (value ^ crc) & 0x80 != 0 {
                crc ^= 0x09;
            }
            value <<= 1;
        }
    }
    (crc << 1) | 1
}

fn command<B, C, D>(spi: &mut SdSpiSession<B, C, D>, cmd: u8, arg: u32) -> Result<u8, InitError>
where
    B: SpiBus<u8>,
    C: OutputPin,
    D: DelayNs,
{
    spi.begin_command().map_err(|_| InitError::Transport)?;
    if cmd != 0 {
        let mut last = 0;
        for _ in 0..64 {
            last = byte(spi)?;
            if last == 0xff {
                break;
            }
        }
        if last != 0xff {
            return Err(InitError::Busy { command: cmd, last });
        }
    }
    let mut packet = [0x40 | cmd, 0, 0, 0, 0, 0];
    packet[1..5].copy_from_slice(&arg.to_be_bytes());
    packet[5] = command_crc(&packet[..5]);
    spi.write(&packet).map_err(|_| InitError::Transport)?;
    for _ in 0..64 {
        let r1 = byte(spi)?;
        if r1 & 0x80 == 0 {
            if r1 & !1 != 0 {
                let mut trailing = [0xff; 8];
                spi.transfer_in_place(&mut trailing)
                    .map_err(|_| InitError::Transport)?;
                esp_println::println!(
                    "microSD: CMD{cmd} tx={packet:02x?} R1=0x{r1:02x} trailing={trailing:02x?}"
                );
            }
            return Ok(r1);
        }
    }
    Err(InitError::NoResponse { command: cmd })
}

fn expect(cmd: u8, r1: u8, wanted: u8) -> Result<(), InitError> {
    if r1 == wanted {
        Ok(())
    } else {
        Err(InitError::Response { command: cmd, r1 })
    }
}

fn tail<S: SpiDevice<u8>>(spi: &mut S) -> Result<[u8; 4], InitError> {
    let mut bytes = [0xff; 4];
    spi.transfer_in_place(&mut bytes)
        .map_err(|_| InitError::Transport)?;
    Ok(bytes)
}

fn handshake<B, C, D>(spi: &mut SdSpiSession<B, C, D>, delay: &mut D) -> Result<CardType, InitError>
where
    B: SpiBus<u8>,
    C: OutputPin,
    D: DelayNs,
{
    esp_println::println!(
        "microSD: checked startup v3; released idle; CRC_ON after ready; ACMD41 budget 200 x 10ms"
    );
    let mut idle = false;
    for _ in 0..50 {
        match command(spi, 0, 0) {
            Ok(1) => {
                idle = true;
                break;
            }
            Ok(r1) => return Err(InitError::Response { command: 0, r1 }),
            Err(InitError::NoResponse { .. }) => {
                spi.finish_command().map_err(|_| InitError::Transport)?;
                delay.delay_ms(10);
            }
            Err(e) => return Err(e),
        }
    }
    if !idle {
        return Err(InitError::NoResponse { command: 0 });
    }
    let r1 = command(spi, 8, 0x1aa)?;
    let v2 = if r1 == 1 {
        let r7 = tail(spi)?;
        esp_println::println!("microSD: CMD8 R1=0x{r1:02x} R7={r7:02x?}");
        if r7 != [0, 0, 1, 0xaa] {
            return Err(InitError::Interface(r7));
        }
        true
    } else if r1 == 5 {
        false
    } else {
        return Err(InitError::Response { command: 8, r1 });
    };

    let mut last = (0xff, 0xff);
    let mut ready = false;
    for attempt in 0..200 {
        let cmd55 = command(spi, 55, 0)?;
        if cmd55 != 0 && cmd55 != 1 {
            return Err(InitError::Response {
                command: 55,
                r1: cmd55,
            });
        }
        let acmd41 = command(spi, 41, if v2 { 0x4000_0000 } else { 0 })?;
        spi.finish_command().map_err(|_| InitError::Transport)?;
        if attempt == 0 || last != (cmd55, acmd41) || attempt % 50 == 49 {
            esp_println::println!(
                "microSD: init poll {} CMD55 R1=0x{cmd55:02x} ACMD41 R1=0x{acmd41:02x}",
                attempt + 1
            );
        }
        last = (cmd55, acmd41);
        if acmd41 == 0 {
            ready = true;
            break;
        }
        if acmd41 != 1 {
            return Err(InitError::Response {
                command: 41,
                r1: acmd41,
            });
        }
        delay.delay_ms(10);
    }
    if !ready {
        return Err(InitError::NotReady {
            cmd55: last.0,
            acmd41: last.1,
        });
    }
    let r1 = command(spi, 58, 0)?;
    expect(58, r1, 0)?;
    let ocr = tail(spi)?;
    esp_println::println!("microSD: CMD58 OCR={ocr:02x?}");
    let value = u32::from_be_bytes(ocr);
    if value & 0x8000_0000 == 0 || value & 0x00ff_8000 == 0 {
        return Err(InitError::Ocr(ocr));
    }
    let kind = if v2 && value & 0x4000_0000 != 0 {
        CardType::SdhcSdxc
    } else if v2 {
        CardType::SD2
    } else {
        CardType::SD1
    };
    let r1 = command(spi, 59, 1)?;
    expect(59, r1, 0)?;
    if kind != CardType::SdhcSdxc {
        let r1 = command(spi, 16, 512)?;
        expect(16, r1, 0)?;
    }
    Ok(kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{cell::RefCell, convert::Infallible};
    use std::{collections::VecDeque, rc::Rc};
    #[derive(Default)]
    struct Model {
        selected: bool,
        pending: VecDeque<u8>,
        commands: std::vec::Vec<u8>,
        polls: u16,
        ready_after: u16,
        reject: bool,
        bad_echo: bool,
        never_reply: bool,
        bad_ocr: bool,
        reject_crc: bool,
        bad_prefix: bool,
        sleeps: u32,
    }
    struct Bus(Rc<RefCell<Model>>);
    struct Cs(Rc<RefCell<Model>>);
    struct Delay(Rc<RefCell<Model>>);
    impl DelayNs for Delay {
        fn delay_ns(&mut self, ns: u32) {
            let mut m = self.0.borrow_mut();
            if ns >= 1_000_000 {
                assert!(!m.selected, "idle delay must release CS");
            }
            m.sleeps += ns / 1_000_000;
        }
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
            let mut m = self.0.borrow_mut();
            m.selected = false;
            m.pending.clear();
            Ok(())
        }
    }
    impl embedded_hal::spi::ErrorType for Bus {
        type Error = Infallible;
    }
    impl SpiBus<u8> for Bus {
        fn write(&mut self, b: &[u8]) -> Result<(), Self::Error> {
            let mut m = self.0.borrow_mut();
            if m.selected && b.len() == 6 {
                assert_eq!(b[5], command_crc(&b[..5]));
                let cmd = b[0] & 63;
                m.commands.push(cmd);
                let mut response = std::vec![0xff];
                match cmd {
                    0 => response.push(1),
                    8 => {
                        response.extend([1, 0, 0, 1, if m.bad_echo { 0xab } else { 0xaa }]);
                    }
                    55 => response.push(if m.bad_prefix && m.polls == 1 { 7 } else { 1 }),
                    41 => {
                        assert_eq!(&b[1..5], &0x40000000u32.to_be_bytes());
                        m.polls += 1;
                        if !m.never_reply {
                            response.push(if m.reject {
                                9
                            } else if m.polls >= m.ready_after {
                                0
                            } else {
                                1
                            });
                        }
                    }
                    58 => response.extend([0, if m.bad_ocr { 0x40 } else { 0xc0 }, 0xff, 0x80, 0]),
                    59 => {
                        assert!(m.polls >= m.ready_after);
                        response.push(if m.reject_crc { 4 } else { 0 });
                    }
                    _ => panic!("unexpected command {cmd}"),
                }
                m.pending.extend(response);
            }
            Ok(())
        }
        fn read(&mut self, b: &mut [u8]) -> Result<(), Self::Error> {
            b.fill(0xff);
            self.transfer_in_place(b)
        }
        fn transfer(&mut self, r: &mut [u8], w: &[u8]) -> Result<(), Self::Error> {
            assert!(w.iter().all(|&x| x == 0xff));
            r.fill(0xff);
            self.transfer_in_place(r)
        }
        fn transfer_in_place(&mut self, b: &mut [u8]) -> Result<(), Self::Error> {
            let mut m = self.0.borrow_mut();
            assert!(m.selected);
            for x in b {
                assert_eq!(*x, 0xff);
                *x = m.pending.pop_front().unwrap_or(0xff);
            }
            Ok(())
        }
        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    fn run(m: Rc<RefCell<Model>>) -> Result<CardType, InitError> {
        let mut spi = SdSpiSession::new(Bus(m.clone()), Cs(m.clone()), Delay(m.clone()));
        handshake(&mut spi, &mut Delay(m))
    }
    #[test]
    fn known_command_crc_vectors() {
        assert_eq!(command_crc(&[0x40, 0, 0, 0, 0]), 0x95);
        assert_eq!(command_crc(&[0x48, 0, 0, 1, 0xaa]), 0x87);
        assert_eq!(command_crc(&[0x69, 0x40, 0, 0, 0]), 0x77);
        assert_eq!(command_crc(&[0x77, 0, 0, 0, 0]), 0x65);
    }
    #[test]
    fn second_cmd55_error_is_reported_instead_of_waiting_for_acmd41() {
        let m = Rc::new(RefCell::new(Model {
            ready_after: 10,
            bad_prefix: true,
            ..Default::default()
        }));
        assert!(matches!(
            run(m.clone()),
            Err(InitError::Response { command: 55, r1: 7 })
        ));
        assert_eq!(m.borrow().polls, 1);
    }
    #[test]
    fn ready_without_valid_ocr_or_crc_ack_is_not_a_verified_card() {
        let m = Rc::new(RefCell::new(Model {
            ready_after: 1,
            bad_ocr: true,
            ..Default::default()
        }));
        assert!(matches!(run(m), Err(InitError::Ocr(_))));
        let m = Rc::new(RefCell::new(Model {
            ready_after: 1,
            reject_crc: true,
            ..Default::default()
        }));
        assert!(matches!(
            run(m),
            Err(InitError::Response { command: 59, r1: 4 })
        ));
    }
    #[test]
    fn delayed_ready_checks_echo_ocr_and_enables_crc_last() {
        let m = Rc::new(RefCell::new(Model {
            ready_after: 4,
            ..Default::default()
        }));
        assert_eq!(run(m.clone()).unwrap(), CardType::SdhcSdxc);
        let m = m.borrow();
        assert_eq!(m.polls, 4);
        assert_eq!(m.sleeps, 30);
        assert_eq!(&m.commands[m.commands.len() - 2..], &[58, 59]);
    }
    #[test]
    fn crc_rejection_and_no_response_are_distinct() {
        let m = Rc::new(RefCell::new(Model {
            reject: true,
            ready_after: 4,
            ..Default::default()
        }));
        assert!(matches!(
            run(m),
            Err(InitError::Response { command: 41, r1: 9 })
        ));
        let m = Rc::new(RefCell::new(Model {
            never_reply: true,
            ..Default::default()
        }));
        assert!(matches!(run(m), Err(InitError::NoResponse { command: 41 })));
    }
    #[test]
    fn idle_is_bounded_and_voltage_echo_is_checked() {
        let m = Rc::new(RefCell::new(Model {
            ready_after: 201,
            ..Default::default()
        }));
        assert!(matches!(
            run(m.clone()),
            Err(InitError::NotReady {
                cmd55: 1,
                acmd41: 1
            })
        ));
        assert_eq!(m.borrow().polls, 200);
        assert_eq!(m.borrow().sleeps, 2000);
        let m = Rc::new(RefCell::new(Model {
            bad_echo: true,
            ..Default::default()
        }));
        assert!(matches!(run(m), Err(InitError::Interface(_))));
    }
}
