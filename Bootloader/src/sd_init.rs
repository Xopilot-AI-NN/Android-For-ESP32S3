// Модуль проверенной инициализации microSD.
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
/// Точная стадия отказа инициализации microSD компонента проверенной инициализации microSD.
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
    /// Форматирует значение для диагностики, не изменяя его.
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
                "ACMD41: still idle after 500 polls; CMD55=0x{cmd55:02x}, ACMD41=0x{acmd41:02x}"
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
    crate::watch_println!(
        "microSD: initialized, {} MiB, {:?}",
        bytes / (1024 * 1024),
        kind
    );
    Ok(card)
}

/// Передаёт байт SPI, одновременно считывая входные биты.
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

/// Начинает отдельную CS-сессию и передаёт SD-команду с аргументом.
fn command<B, C, D>(spi: &mut SdSpiSession<B, C, D>, cmd: u8, arg: u32) -> Result<u8, InitError>
where
    B: SpiBus<u8>,
    C: OutputPin,
    D: DelayNs,
{
    spi.begin_command().map_err(|_| InitError::Transport)?;
    selected_command(spi, cmd, arg)
}

// Diagnostic path can preserve CS across the APP_CMD prefix and ACMD41.
fn selected_command<B: SpiBus<u8>, C: OutputPin, D: DelayNs>(
    spi: &mut SdSpiSession<B, C, D>,
    cmd: u8,
    arg: u32,
) -> Result<u8, InitError> {
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
            return Ok(r1);
        }
    }
    Err(InitError::NoResponse { command: cmd })
}

/// Сверяет ответ команды с ожидаемым значением.
fn expect(cmd: u8, r1: u8, wanted: u8) -> Result<(), InitError> {
    if r1 == wanted {
        Ok(())
    } else {
        Err(InitError::Response { command: cmd, r1 })
    }
}

/// Читает четыре дополнительных байта R7/OCR.
fn tail<S: SpiDevice<u8>>(spi: &mut S) -> Result<[u8; 4], InitError> {
    let mut bytes = [0xff; 4];
    spi.transfer_in_place(&mut bytes)
        .map_err(|_| InitError::Transport)?;
    Ok(bytes)
}

/// Выполняет проверенную последовательность входа карты в SPI-режим.
pub(crate) fn handshake<B: SpiBus<u8>, C: OutputPin, D: DelayNs>(
    spi: &mut SdSpiSession<B, C, D>,
    delay: &mut D,
) -> Result<CardType, InitError> {
    handshake_impl::<B, C, D, false>(spi, delay)
}

#[cfg(any(test, feature = "sd-selftest"))]
/// Выполняет диагностическую инициализацию с тактами во время ожиданий.
pub(crate) fn handshake_clocked<B: SpiBus<u8>, C: OutputPin, D: DelayNs>(
    spi: &mut SdSpiSession<B, C, D>,
    delay: &mut D,
) -> Result<CardType, InitError> {
    handshake_impl::<B, C, D, true>(spi, delay)
}

/// Выдерживает паузу инициализации выбранным способом.
fn settle<B: SpiBus<u8>, C: OutputPin, D: DelayNs, const CLOCKED: bool>(
    spi: &mut SdSpiSession<B, C, D>,
    delay: &mut D,
    ms: u32,
) -> Result<(), InitError> {
    if CLOCKED {
        // At <=100 kHz, 16 bytes provide at least 1.28ms of selected clocks.
        // 0xff cannot form an SD command start bit and is safe during idle.
        for _ in 0..ms {
            spi.write(&[0xff; 16]).map_err(|_| InitError::Transport)?;
        }
    } else {
        delay.delay_ms(ms);
    }
    Ok(())
}

/// Выполняет общую последовательность CMD0/CMD8/ACMD41/OCR/CRC.
fn handshake_impl<B, C, D, const CLOCKED: bool>(
    spi: &mut SdSpiSession<B, C, D>,
    delay: &mut D,
) -> Result<CardType, InitError>
where
    B: SpiBus<u8>,
    C: OutputPin,
    D: DelayNs,
{
    crate::watch_println!("microSD: checked startup v6; two CMD0 resets; ACMD41 budget 500 x 10ms");
    // ESP-IDF sends an initial GO_IDLE without relying on its response, then
    // repeats it with R1 checking: some cards do not select SPI on the first.
    crate::watch_println!("microSD: wait clocks={CLOCKED}");
    let wake = command(spi, 0, 0);
    if matches!(wake, Err(InitError::Transport)) {
        return Err(InitError::Transport);
    }
    crate::watch_println!("microSD: first CMD0 wake response={wake:?}");
    spi.finish_command().map_err(|_| InitError::Transport)?;
    settle::<B, C, D, CLOCKED>(spi, delay, 100)?;
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
                settle::<B, C, D, CLOCKED>(spi, delay, 10)?;
            }
            Err(e) => return Err(e),
        }
    }
    if !idle {
        return Err(InitError::NoResponse { command: 0 });
    }
    spi.finish_command().map_err(|_| InitError::Transport)?;
    settle::<B, C, D, CLOCKED>(spi, delay, 100)?;
    let r1 = command(spi, 8, 0x1aa)?;
    let v2 = if r1 == 1 {
        let r7 = tail(spi)?;
        crate::watch_println!("microSD: CMD8 R1=0x{r1:02x} R7={r7:02x?}");
        if r7 != [0, 0, 1, 0xaa] {
            return Err(InitError::Interface(r7));
        }
        true
    } else if r1 == 5 {
        false
    } else {
        return Err(InitError::Response { command: 8, r1 });
    };

    // OCR is readable in idle. Validate the supply window before requesting
    // initialization; the ready/CCS bits are interpreted only after it starts.
    let initial_r1 = command(spi, 58, 0)?;
    if initial_r1 > 1 {
        return Err(InitError::Response {
            command: 58,
            r1: initial_r1,
        });
    }
    let initial_ocr = tail(spi)?;
    crate::watch_println!("microSD: preflight CMD58 R1=0x{initial_r1:02x} OCR={initial_ocr:02x?}");
    if u32::from_be_bytes(initial_ocr) & 0x00ff_8000 == 0 {
        return Err(InitError::Ocr(initial_ocr));
    }

    // Some controllers stop answering while their flash controller starts.
    // Keep their initialization running: never issue another CMD0 here.
    let mut last_error = InitError::NotReady {
        cmd55: 0xff,
        acmd41: 0xff,
    };
    let mut transient_count = 0u16;
    let mut ready = false;
    for attempt in 0..500 {
        let response = (|| {
            let prefix = command(spi, 55, 0)?;
            if prefix > 1 {
                return Err(InitError::Response {
                    command: 55,
                    r1: prefix,
                });
            }
            let arg = if v2 { 0x4000_0000 } else { 0 };
            let status = if CLOCKED {
                selected_command(spi, 41, arg)?
            } else {
                command(spi, 41, arg)?
            };
            if status > 1 {
                return Err(InitError::Response {
                    command: 41,
                    r1: status,
                });
            }
            Ok((prefix, status))
        })();
        spi.finish_command().map_err(|_| InitError::Transport)?;
        match response {
            Ok((prefix, 0)) => {
                crate::watch_println!(
                    "microSD: ready at poll {}; CMD55=0x{prefix:02x}, ACMD41=0x00, transient replies={transient_count}",
                    attempt + 1
                );
                ready = true;
                break;
            }
            Ok((prefix, status)) => {
                last_error = InitError::NotReady {
                    cmd55: prefix,
                    acmd41: status,
                };
                if attempt == 0 || attempt % 50 == 49 {
                    crate::watch_println!(
                        "microSD: init poll {} CMD55=0x{prefix:02x} ACMD41=0x{status:02x}",
                        attempt + 1
                    );
                }
            }
            Err(InitError::Transport) => return Err(InitError::Transport),
            Err(error) => {
                transient_count += 1;
                last_error = error;
                if transient_count == 1 || attempt % 50 == 49 {
                    crate::watch_println!(
                        "microSD: init poll {} retry without reset: {error}",
                        attempt + 1
                    );
                }
            }
        }
        // Independently check readiness instead of assuming every missing
        // APP_CMD means the flash controller is still in idle. Never accept an
        // error response: both R1=0 and OCR power-up-complete are required.
        if attempt == 1 || attempt % 50 == 49 {
            match command(spi, 58, 0) {
                Ok(r1) if r1 <= 1 => {
                    let ocr = tail(spi)?;
                    crate::watch_println!(
                        "microSD: OCR poll {} R1=0x{r1:02x} OCR={ocr:02x?}",
                        attempt + 1
                    );
                    if r1 == 0
                        && u32::from_be_bytes(ocr) & 0x8000_0000 != 0
                        && u32::from_be_bytes(ocr) & 0x00ff_8000 != 0
                    {
                        ready = true;
                        crate::watch_println!(
                            "microSD: ready confirmed by OCR after transient APP_CMD replies"
                        );
                        break;
                    }
                }
                Ok(r1) => crate::watch_println!("microSD: OCR poll R1=0x{r1:02x}"),
                Err(e) => crate::watch_println!("microSD: OCR poll: {e}"),
            }
            spi.finish_command().map_err(|_| InitError::Transport)?;
        }
        settle::<B, C, D, CLOCKED>(spi, delay, 10)?;
    }
    if !ready {
        crate::watch_println!(
            "microSD: initialization budget exhausted; transient replies={transient_count}"
        );
        return Err(last_error);
    }
    let r1 = command(spi, 58, 0)?;
    expect(58, r1, 0)?;
    let ocr = tail(spi)?;
    crate::watch_println!("microSD: CMD58 OCR={ocr:02x?}");
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

#[cfg(all(test, not(target_os = "none")))]
mod tests {
    use super::*;
    use core::{cell::RefCell, convert::Infallible};
    use std::{collections::VecDeque, rc::Rc};
    #[derive(Default)]
    /// Структура данных компонента проверенной инициализации microSD.
    struct Model {
        /// Выбранный элемент либо признак его выделения.
        selected: bool,
        /// Необработанный запрос или ожидающий результат.
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
        prefix_calls: u16,
        prefix_pauses: u16,
        prefix_garbled: u16,
        /// Признак завершения подготовки данных.
        ready: bool,
        ocr_ready_without_41_zero: bool,
        cmd0_count: u8,
        needs_two_cmd0: bool,
        sleeps: u32,
        clocked_bytes: usize,
    }
    /// Структура данных компонента проверенной инициализации microSD.
    struct Bus(Rc<RefCell<Model>>);
    /// Структура данных компонента проверенной инициализации microSD.
    struct Cs(Rc<RefCell<Model>>);
    /// Структура данных компонента проверенной инициализации microSD.
    struct Delay(Rc<RefCell<Model>>);
    impl DelayNs for Delay {
        /// Выполняет задержку, заданную в наносекундах.
        fn delay_ns(&mut self, ns: u32) {
            let mut m = self.0.borrow_mut();
            if ns >= 1_000_000 {
                assert!(!m.selected, "idle delay must release CS");
            }
            m.sleeps += ns / 1_000_000;
        }
    }
    impl embedded_hal::digital::ErrorType for Cs {
        /// Причины отказа операций компонента проверенной инициализации microSD.
        type Error = Infallible;
    }
    impl OutputPin for Cs {
        /// Устанавливает низкий уровень цифрового выхода.
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.0.borrow_mut().selected = true;
            Ok(())
        }
        /// Устанавливает высокий уровень цифрового выхода.
        fn set_high(&mut self) -> Result<(), Self::Error> {
            let mut m = self.0.borrow_mut();
            m.selected = false;
            m.pending.clear();
            Ok(())
        }
    }
    impl embedded_hal::spi::ErrorType for Bus {
        /// Причины отказа операций компонента проверенной инициализации microSD.
        type Error = Infallible;
    }
    impl SpiBus<u8> for Bus {
        /// Передаёт данные через интерфейс текущего транспорта.
        fn write(&mut self, b: &[u8]) -> Result<(), Self::Error> {
            let mut m = self.0.borrow_mut();
            if m.selected && b == [0xff; 16] {
                m.clocked_bytes += b.len();
            }
            if m.selected && b.len() == 6 {
                assert_eq!(b[5], command_crc(&b[..5]));
                let cmd = b[0] & 63;
                m.commands.push(cmd);
                let mut response = std::vec![0xff];
                match cmd {
                    0 => {
                        m.cmd0_count += 1;
                        response.push(1);
                    }
                    8 => {
                        response.extend([1, 0, 0, 1, if m.bad_echo { 0xab } else { 0xaa }]);
                    }
                    55 => {
                        m.prefix_calls += 1;
                        if m.needs_two_cmd0 && m.cmd0_count < 2 {
                            response.push(5);
                            m.pending.extend(response);
                            return Ok(());
                        }
                        if m.prefix_calls > 1 && m.prefix_calls <= m.prefix_pauses + 1 {
                        } else {
                            response.push(
                                if (m.bad_prefix && m.polls == 1)
                                    || (m.prefix_calls > 1
                                        && m.prefix_calls <= m.prefix_garbled + 1)
                                {
                                    7
                                } else {
                                    1
                                },
                            );
                        }
                    }
                    41 => {
                        assert_eq!(&b[1..5], &0x40000000u32.to_be_bytes());
                        m.polls += 1;
                        if !m.never_reply {
                            if !m.reject && m.polls >= m.ready_after {
                                m.ready = true;
                            }
                            response.push(if m.reject {
                                9
                            } else if m.polls >= m.ready_after {
                                0
                            } else {
                                1
                            });
                        }
                    }
                    58 => {
                        let ready = m.ready || (m.ocr_ready_without_41_zero && m.polls > 0);
                        response.extend([
                            if ready { 0 } else { 1 },
                            if ready && !m.bad_ocr { 0xc0 } else { 0x40 },
                            0xff,
                            0x80,
                            0,
                        ]);
                    }
                    59 => {
                        assert!(m.ready || (m.ocr_ready_without_41_zero && m.polls > 0));
                        response.push(if m.reject_crc { 4 } else { 0 });
                    }
                    _ => panic!("unexpected command {cmd}"),
                }
                m.pending.extend(response);
            }
            Ok(())
        }
        /// Считывает запрошенные данные через интерфейс текущего транспорта.
        fn read(&mut self, b: &mut [u8]) -> Result<(), Self::Error> {
            b.fill(0xff);
            self.transfer_in_place(b)
        }
        /// Передаёт выходные байты и одновременно считывает входные.
        fn transfer(&mut self, r: &mut [u8], w: &[u8]) -> Result<(), Self::Error> {
            assert!(w.iter().all(|&x| x == 0xff));
            r.fill(0xff);
            self.transfer_in_place(r)
        }
        /// Выполняет полнодуплексный обмен, заменяя исходный буфер принятыми байтами.
        fn transfer_in_place(&mut self, b: &mut [u8]) -> Result<(), Self::Error> {
            let mut m = self.0.borrow_mut();
            assert!(m.selected);
            for x in b {
                assert_eq!(*x, 0xff);
                *x = m.pending.pop_front().unwrap_or(0xff);
            }
            Ok(())
        }
        /// Завершает отложенную передачу данных.
        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    /// Запускает основной цикл компонента и обслуживает его события.
    fn run(m: Rc<RefCell<Model>>) -> Result<CardType, InitError> {
        let mut spi = SdSpiSession::new(Bus(m.clone()), Cs(m.clone()), Delay(m.clone()));
        handshake(&mut spi, &mut Delay(m))
    }
    #[test]
    /// Проверяет восстановление после задержек и мусорного префикса без лишнего CMD0.
    fn pauses_and_garbled_prefix_during_init_recover_without_another_cmd0() {
        for garbled in [false, true] {
            let m = Rc::new(RefCell::new(Model {
                ready_after: 2,
                prefix_pauses: if garbled { 0 } else { 3 },
                prefix_garbled: if garbled { 3 } else { 0 },
                ..Default::default()
            }));
            assert_eq!(run(m.clone()).unwrap(), CardType::SdhcSdxc);
            let m = m.borrow();
            assert_eq!(m.commands.iter().filter(|&&cmd| cmd == 0).count(), 2);
            assert_eq!(m.polls, 2);
            assert_eq!(m.sleeps, 240);
            assert_eq!(&m.commands[m.commands.len() - 2..], &[58, 59]);
        }
    }
    #[test]
    /// Проверяет готовность по OCR и обязательное подтверждение включения CRC.
    fn valid_ocr_can_confirm_readiness_but_crc_ack_still_required() {
        let m = Rc::new(RefCell::new(Model {
            ready_after: 501,
            ocr_ready_without_41_zero: true,
            ..Default::default()
        }));
        assert_eq!(run(m).unwrap(), CardType::SdhcSdxc);
        let m = Rc::new(RefCell::new(Model {
            ready_after: 501,
            ocr_ready_without_41_zero: true,
            reject_crc: true,
            ..Default::default()
        }));
        assert!(matches!(
            run(m),
            Err(InitError::Response { command: 59, r1: 4 })
        ));
    }
    #[test]
    /// Проверяет, что ответ на пробуждение не отменяет второй сброс SPI-режима.
    fn wake_ack_does_not_skip_the_second_spi_mode_reset() {
        let m = Rc::new(RefCell::new(Model {
            ready_after: 2,
            needs_two_cmd0: true,
            ..Default::default()
        }));
        assert_eq!(run(m.clone()).unwrap(), CardType::SdhcSdxc);
        assert_eq!(m.borrow().cmd0_count, 2);
    }
    #[test]
    /// Проверяет выдачу тактов во время диагностического ожидания без изменения команд.
    fn clocked_wait_preserves_commands_and_replaces_cpu_sleep() {
        let m = Rc::new(RefCell::new(Model {
            ready_after: 2,
            needs_two_cmd0: true,
            ..Default::default()
        }));
        let mut spi = SdSpiSession::new(Bus(m.clone()), Cs(m.clone()), Delay(m.clone()));
        assert_eq!(
            handshake_clocked(&mut spi, &mut Delay(m.clone())).unwrap(),
            CardType::SdhcSdxc
        );
        let m = m.borrow();
        assert_eq!(m.cmd0_count, 2);
        assert_eq!(m.sleeps, 0);
        assert_eq!(m.clocked_bytes, 210 * 16);
    }
    #[test]
    /// Сверяет CRC7 SD-команд с известными пакетами.
    fn known_command_crc_vectors() {
        assert_eq!(command_crc(&[0x40, 0, 0, 0, 0]), 0x95);
        assert_eq!(command_crc(&[0x48, 0, 0, 1, 0xaa]), 0x87);
        assert_eq!(command_crc(&[0x69, 0x40, 0, 0, 0]), 0x77);
        assert_eq!(command_crc(&[0x77, 0, 0, 0, 0]), 0x65);
    }
    #[test]
    /// Проверяет точное сообщение об ошибке второго CMD55 вместо общего таймаута ACMD41.
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
    /// Проверяет отказ считать карту готовой без правильных OCR и подтверждения CRC.
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
    /// Проверяет задержанную готовность карты, echo CMD8, OCR и порядок включения CRC.
    fn delayed_ready_checks_echo_ocr_and_enables_crc_last() {
        let m = Rc::new(RefCell::new(Model {
            ready_after: 4,
            ..Default::default()
        }));
        assert_eq!(run(m.clone()).unwrap(), CardType::SdhcSdxc);
        let m = m.borrow();
        assert_eq!(m.polls, 4);
        assert_eq!(m.sleeps, 230);
        assert_eq!(&m.commands[m.commands.len() - 2..], &[58, 59]);
    }
    #[test]
    /// Проверяет различение отказа CRC и отсутствия ответа карты.
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
    /// Проверяет предел ожидания idle и валидацию напряжения/echo CMD8.
    fn idle_is_bounded_and_voltage_echo_is_checked() {
        let m = Rc::new(RefCell::new(Model {
            ready_after: 501,
            ..Default::default()
        }));
        assert!(matches!(
            run(m.clone()),
            Err(InitError::NotReady {
                cmd55: 1,
                acmd41: 1
            })
        ));
        assert_eq!(m.borrow().polls, 500);
        assert_eq!(m.borrow().sleeps, 5200);
        let m = Rc::new(RefCell::new(Model {
            bad_echo: true,
            ..Default::default()
        }));
        assert!(matches!(run(m), Err(InitError::Interface(_))));
    }
}
