// Модуль BLE HID и защищённого сопряжения.
//! BLE HID media remote with the TrouBLE host's Secure Connections manager.
//! Only this module owns HCI. The old discovery-only HCI path must never drain
//! its packets. Futures are cooperatively polled from RadioServices::tick.
use alloc::{boxed::Box, rc::Rc};
use bt_hci::{
    ControllerToHostPacket, HostToControllerPacket, WriteHci,
    transport::{Transport, WithIndicator},
};
use core::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use embassy_futures::select::{Either, select};
use embassy_time::Timer;
use esp_hal::time::Instant;
use esp_radio::ble::controller::BleConnector;
use heapless::Vec;
use trouble_host::prelude::*;

// No executor sleeps on these timers: radio.tick polls every OS iteration.
// A no-op wake is intentional; deadlines still use the real monotonic clock.
struct CooperativeClock;
impl embassy_time_driver::Driver for CooperativeClock {
    /// Возвращает монотонные микросекунды для embassy-time.
    fn now(&self) -> u64 {
        Instant::now().duration_since_epoch().as_micros()
    }
    /// Таймер сам не будит executor: BLE future регулярно опрашивается из radio.tick.
    fn schedule_wake(&self, _: u64, _: &core::task::Waker) {}
}
embassy_time_driver::time_driver_impl!(static CLOCK: CooperativeClock = CooperativeClock);

/// HID-дескриптор Consumer Control для медиакнопок.
const REPORT_MAP: [u8; 70] = [
    // Consumer Control, Report ID 1.
    0x05, 0x0c, 0x09, 0x01, 0xa1, 0x01, 0x85, 0x01, 0x15, 0x00, 0x26, 0xff, 0x03, 0x19, 0x00, 0x2a,
    0xff, 0x03, 0x75, 0x10, 0x95, 0x01, 0x81, 0x00, 0xc0,
    // Keyboard, Report ID 2: modifiers, reserved, six key usages.
    0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x85, 0x02, 0x05, 0x07, 0x19, 0xe0, 0x29, 0xe7, 0x15, 0x00,
    0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x75, 0x08, 0x95, 0x01, 0x81, 0x01, 0x19, 0x00,
    0x29, 0x65, 0x15, 0x00, 0x25, 0x65, 0x75, 0x08, 0x95, 0x06, 0x81, 0x00, 0xc0,
];

#[gatt_server]
/// Таблица GATT-сервисов BLE HID и компаньона компонента BLE HID и защищённого сопряжения.
struct Server {
    /// Единственный BLE HID-host в соответствующей сборке.
    hid: MediaHid,
    /// Дополнительный сервис обмена с компаньоном.
    companion: Companion,
}

#[gatt_service(uuid = "1812")]
/// BLE HID-сервис Consumer Control компонента BLE HID и защищённого сопряжения.
struct MediaHid {
    #[characteristic(uuid = "2a4a", read, value = [0x11, 0x01, 0x00, 0x02])]
    /// Метаданные компонента, выдаваемые клиенту.
    info: [u8; 4],
    #[characteristic(uuid = "2a4b", read, value = REPORT_MAP)]
    /// Дескриптор HID-отчётов.
    map: [u8; 70],
    #[characteristic(uuid = "2a4c", write_without_response)]
    /// Управляющее состояние компонента/протокола.
    control: u8,
    #[descriptor(uuid = "2908", read, value = [1u8, 1u8])]
    #[characteristic(uuid = "2a4d", read, notify)]
    /// HID-отчёт Consumer Control либо результат диагностики.
    report: [u8; 2],
    #[descriptor(uuid = "2908", read, value = [2u8, 1u8])]
    #[characteristic(uuid = "2a4d", read, notify)]
    keyboard: [u8; 8],
}

// Optional custom service; it does not impersonate Google Wear OS services.
#[gatt_service(uuid = "3c7e0001-3b14-4a80-9d3f-178b14740001")]
struct Companion {
    #[characteristic(uuid = "3c7e0002-3b14-4a80-9d3f-178b14740001", write)]
    /// Последнее реальное состояние медиаплеера.
    playback: [u8; 20],
}

#[derive(Default)]
/// Рабочее состояние компонента компонента BLE HID и защищённого сопряжения.
struct State {
    /// Требуемый признак включения сервиса.
    enabled: bool,
    /// Код состояния, передаваемый интерфейсу или диагностике.
    status: u8, // 0 off, 1 advertising, 2 connected, 3 encrypted + subscribed, 4 confirm, 5 error
    /// Необязательный код подтверждения BLE-сопряжения.
    pin: Option<u32>,
    /// Решение пользователя по ожидающему подтверждению.
    decision: Option<bool>,
    /// Отложенный запрос завершения соединения.
    disconnect: bool,
    /// Монотонный срок окончания окна сопряжения.
    pair_until: u64,
    media: Vec<u16, 8>,
    /// Последнее реальное состояние медиаплеера.
    playback: Option<crate::media::Playback>,
    /// Признак подписки подключённого клиента на уведомления.
    subscribed: bool,
    keyboard_subscribed: bool,
    keys: Vec<[u8; 8], 16>,
    /// Сохранённые BLE-ключи сопряжения.
    bonds: Vec<BondInformation, 4>,
    /// Признак необходимости сохранить изменённые BLE-ключи.
    bonds_dirty: bool,
    /// Отложенный запрос удаления ключей сопряжения.
    forget: bool,
    /// Счётчик явных запросов пользователя на новую серию запуска BLE.
    restart_generation: u32,
    /// Последняя отправленная HCI-команда для диагностики зависания.
    last_opcode: u16,
    /// Монотонное время/интервал last_command в миллисекундах.
    last_command_ms: u64,
    /// Счётчик успешных запусков BLE-рекламы.
    advertise_generation: u32,
}

/// Заимствует единственный физический HCI-транспорт: новый host может начать
/// с чистыми очередями команд, не уничтожая BLE-драйвер и не отбирая его BT pin.
struct SessionTransport<'a> {
    connector: &'a BleConnector<'static>,
    /// Общее состояние компонента.
    state: Rc<RefCell<State>>,
}
impl embedded_io::ErrorType for SessionTransport<'_> {
    /// Причины отказа операций компонента BLE HID и защищённого сопряжения.
    type Error = <BleConnector<'static> as embedded_io::ErrorType>::Error;
}
impl Transport for SessionTransport<'_> {
    /// Только активная сессия читает HCI; параллельного raw-HCI reader нет.
    async fn read<'a>(
        &self,
        buffer: &'a mut [u8],
    ) -> Result<ControllerToHostPacket<'a>, Self::Error> {
        Transport::read(self.connector, buffer).await
    }
    /// Запоминает opcode для диагностики зависания и передаёт исходный пакет драйверу.
    async fn write<T: HostToControllerPacket>(&self, packet: &T) -> Result<(), Self::Error> {
        let mut encoded = [0; 259];
        if WithIndicator::new(packet)
            .write_hci(&mut encoded[..])
            .is_ok()
            && encoded[0] == 1
        {
            let mut s = self.state.borrow_mut();
            s.last_opcode = u16::from_le_bytes([encoded[1], encoded[2]]);
            s.last_command_ms = now_ms();
        }
        Transport::write(self.connector, packet).await
    }
}

/// Владелец состояния BLE и единственной опрашиваемой host-задачи компонента BLE HID и защищённого сопряжения.
pub struct BleHost {
    /// Общее состояние компонента.
    state: Rc<RefCell<State>>,
    /// Сохранённая асинхронная задача, опрашиваемая главным циклом.
    task: Pin<Box<dyn Future<Output = ()>>>,
    /// Признак завершения host future; такой экземпляр больше не опрашивается.
    task_finished: bool,
}

// The radio controller is active before this RNG is used. esp-hal feeds the
// security manager's ChaCha CSPRNG from hardware entropy, never a fixed seed.
struct RadioRng;
impl rand_core::RngCore for RadioRng {
    /// Получает 32 бита случайных данных для криптографического генератора.
    fn next_u32(&mut self) -> u32 {
        esp_hal::rng::Rng::new().random()
    }
    /// Собирает 64 бита случайных данных из аппаратного источника.
    fn next_u64(&mut self) -> u64 {
        self.next_u32() as u64 | ((self.next_u32() as u64) << 32)
    }
    /// Заполняет переданный буфер случайными данными.
    fn fill_bytes(&mut self, out: &mut [u8]) {
        esp_hal::rng::Rng::new().read(out);
    }
    /// Заполняет буфер случайными данными через интерфейс rand_core.
    fn try_fill_bytes(&mut self, out: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(out);
        Ok(())
    }
}
impl rand_core::CryptoRng for RadioRng {}

/// Возвращает монотонное время в миллисекундах.
fn now_ms() -> u64 {
    Instant::now().duration_since_epoch().as_millis()
}

pub use crate::ble_bonds::{decode_bonds, encode_bonds};

impl BleHost {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
    pub fn new(
        connector: BleConnector<'static>,
        address: [u8; 6],
        bonds: Vec<BondInformation, 4>,
    ) -> Self {
        let state = Rc::new(RefCell::new(State {
            enabled: true,
            bonds,
            ..Default::default()
        }));
        Self {
            task: Box::pin(run(connector, address, state.clone())),
            state,
            task_finished: false,
        }
    }
    /// Выполняет очередной неблокирующий шаг обработки и проверяет готовые результаты.
    pub fn poll(&mut self) {
        let before = self.state.borrow().status;
        let mut cx = Context::from_waker(core::task::Waker::noop());
        // Poll once per OS iteration; never block waiting for a peer.
        if self.task_finished {
            return;
        }
        if let Poll::Ready(()) = self.task.as_mut().poll(&mut cx) {
            self.task_finished = true;
            self.state.borrow_mut().status = 5;
        }
        let after = self.state.borrow().status;
        if before != after {
            crate::watch_println!("BLE: HID state {} -> {}", before, after);
        }
    }
    /// Включает или отключает сервис, не меняя сохранённые настройки других служб.
    pub fn set_enabled(&mut self, value: bool) {
        let mut s = self.state.borrow_mut();
        if value && self.task_finished {
            s.status = 5;
            return;
        }
        if value && !s.enabled {
            s.restart_generation = s.restart_generation.wrapping_add(1);
        }
        s.enabled = value;
        if !value {
            s.status = 0;
            s.disconnect = true;
            s.pair_until = 0;
            s.media.clear();
            s.pin = None;
        }
    }
    /// Открывает ограниченное по времени окно сопряжения.
    pub fn pair(&mut self) {
        let mut s = self.state.borrow_mut();
        s.enabled = true;
        s.pair_until = now_ms() + 120_000;
        s.disconnect = true;
        s.pin = None;
        s.decision = None;
        s.restart_generation = s.restart_generation.wrapping_add(1);
    }
    /// Передаёт решение пользователя по подтверждению сопряжения.
    pub fn confirm(&mut self, yes: bool) {
        let mut s = self.state.borrow_mut();
        s.decision = Some(yes);
        if !yes {
            s.pair_until = 0;
        }
    }
    /// Удаляет сохранённые связи по явному действию пользователя.
    pub fn forget(&mut self) {
        let mut s = self.state.borrow_mut();
        s.forget = true;
        s.disconnect = true;
        s.pair_until = 0;
    }
    /// Запрашивает завершение текущего соединения.
    pub fn disconnect(&mut self) {
        self.state.borrow_mut().disconnect = true;
    }
    /// Отправляет или рисует управление медиаплеером в текущем компоненте.
    pub fn media(&mut self, usage: u16) -> bool {
        let mut s = self.state.borrow_mut();
        if s.status != 3 || !s.subscribed {
            return false;
        }
        // Consumer Control usage IDs: Play/Pause, Next, Previous, Stop, Mute, Volume +/-.
        if !matches!(
            usage,
            0x00CF | 0x00CD | 0x00B5 | 0x00B6 | 0x00B7 | 0x00E2 | 0x00E9 | 0x00EA
        ) {
            return false;
        }
        s.media.push(usage).is_ok()
    }
    /// Отправляет клавишу только в зашифрованное соединение с подпиской Keyboard Input.
    pub fn keyboard(&mut self, report: [u8; 8]) -> bool {
        let mut s = self.state.borrow_mut();
        if !s.keyboard_subscribed || !matches!(s.status, 2 | 3) {
            return false;
        }
        s.keys.push(report).is_ok()
    }
    /// Забирает последнее реальное состояние воспроизведения.
    pub fn take_playback(&mut self) -> Option<crate::media::Playback> {
        self.state.borrow_mut().playback.take()
    }
    /// Возвращает согласованный снимок текущего состояния.
    pub fn snapshot(&self) -> (u8, Option<u32>) {
        let s = self.state.borrow();
        (s.status, s.pin)
    }
    /// Возвращает оставшееся время окна сопряжения в секундах.
    pub fn pairing_remaining(&self) -> u16 {
        (self.state.borrow().pair_until.saturating_sub(now_ms()) / 1000).min(120) as u16
    }
    /// Возвращает изменённые ключи сопряжения для постоянного хранения.
    pub fn bonds_to_save(&self) -> Option<Vec<BondInformation, 4>> {
        let s = self.state.borrow();
        if s.bonds_dirty {
            Some(s.bonds.clone())
        } else {
            None
        }
    }
    /// Читает сохранённую настройку с проверкой допустимости.
    pub fn saved(&mut self) {
        self.state.borrow_mut().bonds_dirty = false;
    }
    /// Called before the first poll, while the lazy host future has not started.
    pub fn restore_bonds(&mut self, bonds: Vec<BondInformation, 4>) {
        self.state.borrow_mut().bonds = bonds;
    }
}

/// Пересоздаёт только host-сессию после сбоя: её запуск выполняет HCI Reset.
/// Три попытки ограничивают фоновые повторы; затем новое «Сопряжение» или
/// выключение/включение Bluetooth разрешает следующую серию без перезагрузки ESP32.
async fn run(connector: BleConnector<'static>, address: [u8; 6], shared: Rc<RefCell<State>>) {
    let mut recovery = crate::ble_recovery::Recovery::default();
    loop {
        let generation = shared.borrow().restart_generation;
        if !shared.borrow().enabled || !recovery.allowed(generation) {
            Timer::after_millis(50).await;
            continue;
        }
        let advertised = shared.borrow().advertise_generation;
        run_session(&connector, address, shared.clone()).await;
        let failures = recovery.stopped(shared.borrow().advertise_generation != advertised);
        {
            let mut s = shared.borrow_mut();
            s.status = 5;
            s.pin = None;
            s.decision = None;
            s.subscribed = false;
            s.keyboard_subscribed = false;
            s.keys.clear();
            s.playback = None;
            s.media.clear();
        }
        crate::watch_println!("BLE: host session stopped; recovery attempt {}/3", failures);
        Timer::after_millis(500).await;
    }
}

/// Одна сессия владеет своим host, GATT и очередями; физический connector заимствован.
async fn run_session(
    connector: &BleConnector<'static>,
    mut address: [u8; 6],
    shared: Rc<RefCell<State>>,
) {
    let controller: ExternalController<_, 8> = ExternalController::new(SessionTransport {
        connector,
        state: shared.clone(),
    });
    let mut resources: HostResources<DefaultPacketPool, 1, 3> = HostResources::new();
    // Stable per-board static-random identity, distinct from its public address.
    address[5] |= 0xc0;
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(Address::random(address))
        .set_random_generator_seed(&mut RadioRng)
        .set_io_capabilities(IoCapabilities::DisplayYesNo);
    for bond in shared.borrow().bonds.iter() {
        let _ = stack.add_bond_information(bond.clone());
    }
    let Host {
        mut peripheral,
        mut runner,
        ..
    } = stack.build();
    let gap_name = crate::device_name::snapshot();
    let Ok(server) = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: &gap_name,
        appearance: &appearance::human_interface_device::GENERIC_HUMAN_INTERFACE_DEVICE,
    })) else {
        shared.borrow_mut().status = 5;
        return;
    };
    let control = async {
        loop {
            if shared.borrow().forget {
                let mut s = shared.borrow_mut();
                for b in s.bonds.iter() {
                    let _ = stack.remove_bond_information(b.identity.clone());
                }
                s.bonds.clear();
                s.bonds_dirty = true;
                s.forget = false;
            }
            if !shared.borrow().enabled {
                shared.borrow_mut().status = 0;
                Timer::after_millis(50).await;
                continue;
            }
            shared.borrow_mut().status = 7;
            let name_revision = crate::device_name::revision();
            let name = crate::device_name::snapshot();
            let mut name_len = name.len().min(22);
            while !name.is_char_boundary(name_len) {
                name_len -= 1;
            }
            let mut adv = [0; 31];
            let Ok(len) = AdStructure::encode_slice(
                &[
                    AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                    AdStructure::ServiceUuids16(&[0x1812u16.to_le_bytes()]),
                    if name_len == name.len() {
                        AdStructure::CompleteLocalName(&name.as_bytes()[..name_len])
                    } else {
                        AdStructure::ShortenedLocalName(&name.as_bytes()[..name_len])
                    },
                ],
                &mut adv,
            ) else {
                shared.borrow_mut().status = 5;
                return;
            };
            let payload = Advertisement::ConnectableScannableUndirected {
                adv_data: &adv[..len],
                scan_data: &[],
            };
            let advertiser = match select(
                peripheral.advertise(&Default::default(), payload),
                Timer::after_millis(10_000),
            )
            .await
            {
                Either::First(Ok(advertiser)) => advertiser,
                Either::First(Err(error)) => {
                    crate::watch_println!("BLE: advertising start failed: {:?}", error);
                    shared.borrow_mut().status = 5;
                    Timer::after_millis(1000).await;
                    continue;
                }
                Either::Second(_) => {
                    let s = shared.borrow();
                    crate::watch_println!(
                        "BLE: advertising startup timeout; last HCI=0x{:04x} age={}ms; rebuilding host",
                        s.last_opcode,
                        now_ms().saturating_sub(s.last_command_ms)
                    );
                    shared.borrow_mut().status = 5;
                    return;
                }
            };
            let pairing = now_ms() < shared.borrow().pair_until;
            shared.borrow_mut().status = if pairing { 6 } else { 1 };
            crate::watch_println!("BLE: advertising ready as {}", name);
            {
                let mut s = shared.borrow_mut();
                s.advertise_generation = s.advertise_generation.wrapping_add(1);
            }
            let idle = async {
                loop {
                    Timer::after_millis(50).await;
                    let mut s = shared.borrow_mut();
                    if !s.enabled || s.forget || crate::device_name::revision() != name_revision {
                        return;
                    }
                    s.status = if now_ms() < s.pair_until { 6 } else { 1 };
                }
            };
            let raw = match select(advertiser.accept(), idle).await {
                Either::First(Ok(conn)) => conn,
                _ => continue,
            };
            let Ok(conn) = raw.with_attribute_server(&server) else {
                continue;
            };
            {
                let mut s = shared.borrow_mut();
                s.status = 2;
                s.disconnect = false;
                s.pin = None;
                s.media.clear();
                s.playback = None;
                s.subscribed = false;
            }
            let pairable = now_ms() < shared.borrow().pair_until;
            let _ = conn
                .raw()
                .set_bondable(pairable && shared.borrow().bonds.len() < 4);
            // Existing bonds can reconnect outside the pairing window. Unknown
            // peers must be explicitly admitted from the physical watch UI.
            let identity = conn.raw().peer_identity();
            let known = shared
                .borrow()
                .bonds
                .iter()
                .any(|b| b.identity.match_identity(&identity));
            if !pairable && !known {
                conn.raw().disconnect();
                continue;
            }
            if let Err(err) = conn.raw().request_security() {
                crate::watch_println!("BLE: security request failed: {:?}", err);
                conn.raw().disconnect();
                continue;
            }
            let events = async {
                loop {
                    match conn.next().await {
                        GattConnectionEvent::Disconnected { .. } => return,
                        GattConnectionEvent::PassKeyConfirm(pin) => {
                            let mut s = shared.borrow_mut();
                            s.pin = Some(pin.value());
                            s.status = 4;
                        }
                        GattConnectionEvent::PassKeyDisplay(pin) => {
                            let mut s = shared.borrow_mut();
                            s.pin = Some(pin.value());
                            s.status = 4;
                        }
                        GattConnectionEvent::PairingComplete { bond, .. } => {
                            {
                                let mut s = shared.borrow_mut();
                                s.pin = None;
                                s.decision = None;
                                s.status = 2;
                            }
                            if let Some(bond) = bond {
                                // Keep the active SecurityManager synchronized NOW.
                                // Merely putting the LTK into State.bonds breaks reconnects
                                // within the same boot, causing AUTHENTICATION_FAILURE.
                                if let Err(err) = stack.add_bond_information(bond.clone()) {
                                    crate::watch_println!(
                                        "BLE: active bond install failed: {:?}",
                                        err
                                    );
                                    shared.borrow_mut().status = 5;
                                    conn.raw().disconnect();
                                    return;
                                }
                                let mut s = shared.borrow_mut();
                                if let Some(i) = s
                                    .bonds
                                    .iter()
                                    .position(|b| b.identity.match_identity(&bond.identity))
                                {
                                    s.bonds[i] = bond;
                                } else if s.bonds.push(bond).is_err() {
                                    s.status = 5;
                                    conn.raw().disconnect();
                                    return;
                                }
                                s.bonds_dirty = true;
                            }
                        }
                        GattConnectionEvent::PairingFailed(reason) => {
                            crate::watch_println!("BLE: pairing failed: {:?}", reason);

                            let mut s = shared.borrow_mut();
                            s.status = 5;
                            s.pin = None;
                            s.decision = None;
                            s.subscribed = false;
                            s.media.clear();

                            // Не отключаем соединение повторно.
                            // Дожидаемся события Disconnected от BLE-стека.
                        }
                        GattConnectionEvent::Gatt { event } => {
                            // Require encryption for reports/control; discovery
                            // and the report map remain readable to HID hosts.
                            let sensitive = match &event {
                                GattEvent::Read(e) => {
                                    e.handle() == server.hid.report.handle
                                        || e.handle() == server.hid.keyboard.handle
                                }
                                GattEvent::Write(_) => true,
                                _ => false,
                            };
                            let encrypted = conn
                                .raw()
                                .security_level()
                                .map(|s| s.encrypted())
                                .unwrap_or(false);
                            if encrypted {
                                if let GattEvent::Write(e) = &event {
                                    if e.handle() == server.companion.playback.handle {
                                        if let Some(p) =
                                            crate::media::Playback::decode(e.data(), now_ms())
                                        {
                                            shared.borrow_mut().playback = Some(p);
                                        }
                                    }
                                }
                            }
                            let keyboard_subscription = match &event {
                                GattEvent::Write(e)
                                    if Some(e.handle()) == server.hid.keyboard.cccd_handle
                                        && e.data().len() == 2 =>
                                {
                                    Some(e.data() == [1, 0])
                                }
                                _ => None,
                            };
                            let subscription = match &event {
                                GattEvent::Write(e)
                                    if Some(e.handle()) == server.hid.report.cccd_handle
                                        && e.data().len() == 2 =>
                                {
                                    Some(e.data() == [1, 0])
                                }
                                _ => None,
                            };
                            let reply = if sensitive && !encrypted {
                                event.reject(AttErrorCode::INSUFFICIENT_ENCRYPTION)
                            } else {
                                event.accept()
                            };
                            if let Ok(reply) = reply {
                                if encrypted {
                                    if let Some(on) = keyboard_subscription {
                                        shared.borrow_mut().keyboard_subscribed = on;
                                    }
                                    if let Some(on) = subscription {
                                        shared.borrow_mut().subscribed = on;
                                    }
                                }
                                reply.send().await;
                            }
                        }
                        _ => {}
                    }
                }
            };
            let media = async {
                loop {
                    Timer::after_millis(30).await;
                    let (enabled, disconnect, decision, expired) = {
                        let mut s = shared.borrow_mut();
                        (
                            s.enabled,
                            s.disconnect,
                            s.decision.take(),
                            s.pin.is_some() && now_ms() >= s.pair_until,
                        )
                    };
                    if !conn.raw().is_connected() {
                        return;
                    }
                    if !enabled || disconnect || expired {
                        if conn.raw().is_connected() {
                            conn.raw().disconnect();
                        }
                        return;
                    }
                    if let Some(yes) = decision {
                        if yes {
                            let _ = conn.raw().pass_key_confirm();
                        } else {
                            let _ = conn.raw().pass_key_cancel();
                        }
                    }
                    let encrypted = conn
                        .raw()
                        .security_level()
                        .map(|s| s.encrypted())
                        .unwrap_or(false);
                    if encrypted {
                        if shared.borrow().pin.is_none() {
                            let ready = {
                                let s = shared.borrow();
                                s.subscribed || s.keyboard_subscribed
                            };
                            shared.borrow_mut().status = if ready { 3 } else { 2 };
                        }
                        let key = {
                            let mut s = shared.borrow_mut();
                            if s.keyboard_subscribed && !s.keys.is_empty() {
                                Some(s.keys.remove(0))
                            } else {
                                None
                            }
                        };
                        if let Some(key) = key {
                            let down = server.hid.keyboard.notify(&conn, &key).await;
                            Timer::after_millis(30).await;
                            let up = server.hid.keyboard.notify(&conn, &[0; 8]).await;
                            if down.is_err() || up.is_err() {
                                conn.raw().disconnect();
                                return;
                            }
                        }
                        let usage = {
                            let mut s = shared.borrow_mut();
                            if !s.subscribed || s.media.is_empty() {
                                None
                            } else {
                                Some(s.media.remove(0))
                            }
                        };
                        if let Some(usage) = usage {
                            // Every key-down is followed by key-up, preventing
                            // stuck volume/play keys on Android or Linux.
                            let down = server.hid.report.notify(&conn, &usage.to_le_bytes()).await;
                            Timer::after_millis(35).await;
                            let up = server.hid.report.notify(&conn, &[0, 0]).await;
                            if down.is_err() || up.is_err() {
                                crate::watch_println!(
                                    "BLE: HID notify failed; dropping connection"
                                );
                                shared.borrow_mut().status = 5;
                                conn.raw().disconnect();
                                return;
                            }
                        }
                    }
                }
            };
            let _ = select(events, media).await;
            let mut s = shared.borrow_mut();
            s.pin = None;
            s.decision = None;
            s.media.clear();
            s.subscribed = false;
            s.disconnect = false;
            s.status = if s.enabled { 1 } else { 0 };
        }
    };
    let _ = select(
        async {
            if let Err(e) = runner.run().await {
                crate::watch_println!("BLE host stopped: {:?}", e);
                shared.borrow_mut().status = 5;
            }
        },
        control,
    )
    .await;
}
