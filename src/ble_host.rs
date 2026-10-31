//! BLE HID media remote with the TrouBLE host's Secure Connections manager.
//! Only this module owns HCI. The old discovery-only HCI path must never drain
//! its packets. Futures are cooperatively polled from RadioServices::tick.
use alloc::{boxed::Box, rc::Rc};
use core::{cell::RefCell, future::Future, pin::Pin, task::{Context, Poll}};
use embassy_futures::{join::join, select::{select, Either}};
use embassy_time::Timer;
use esp_hal::time::Instant;
use esp_radio::ble::controller::BleConnector;
use heapless::Vec;
use trouble_host::prelude::*;

// No executor sleeps on these timers: radio.tick polls every OS iteration.
// A no-op wake is intentional; deadlines still use the real monotonic clock.
struct CooperativeClock;
impl embassy_time_driver::Driver for CooperativeClock {
    fn now(&self) -> u64 { Instant::now().duration_since_epoch().as_micros() }
    fn schedule_wake(&self, _: u64, _: &core::task::Waker) {}
}
embassy_time_driver::time_driver_impl!(static CLOCK: CooperativeClock = CooperativeClock);

const REPORT_MAP: [u8; 23] = [
    0x05,0x0c,0x09,0x01,0xa1,0x01,0x15,0x00,0x26,0xff,0x03,
    0x19,0x00,0x2a,0xff,0x03,0x75,0x10,0x95,0x01,0x81,0x00,0xc0,
];

#[gatt_server]
struct Server { hid: MediaHid }

#[gatt_service(uuid = "1812")]
struct MediaHid {
    #[characteristic(uuid = "2a4a", read, value = [0x11, 0x01, 0x00, 0x02])]
    info: [u8; 4],
    #[characteristic(uuid = "2a4b", read, value = REPORT_MAP)]
    map: [u8; 23],
    #[characteristic(uuid = "2a4c", write_without_response)]
    control: u8,
    #[descriptor(uuid = "2908", read, value = [0u8, 1u8])]
    #[characteristic(uuid = "2a4d", read, notify)]
    report: [u8; 2],
}

#[derive(Default)]
struct State {
    enabled: bool,
    status: u8, // 0 off, 1 advertising, 2 connected, 3 encrypted + subscribed, 4 confirm, 5 error
    pin: Option<u32>,
    decision: Option<bool>,
    disconnect: bool,
    pair_until: u64,
    media: Vec<u16, 8>,
    subscribed: bool,
    bonds: Vec<BondInformation, 4>,
    bonds_dirty: bool,
    forget: bool,
}

pub struct BleHost {
    state: Rc<RefCell<State>>,
    task: Pin<Box<dyn Future<Output = ()>>>,
    task_finished: bool,
}

// The radio controller is active before this RNG is used. esp-hal feeds the
// security manager's ChaCha CSPRNG from hardware entropy, never a fixed seed.
struct RadioRng;
impl rand_core::RngCore for RadioRng {
    fn next_u32(&mut self) -> u32 { esp_hal::rng::Rng::new().random() }
    fn next_u64(&mut self) -> u64 { self.next_u32() as u64 | ((self.next_u32() as u64) << 32) }
    fn fill_bytes(&mut self, out: &mut [u8]) { esp_hal::rng::Rng::new().read(out); }
    fn try_fill_bytes(&mut self, out: &mut [u8]) -> Result<(), rand_core::Error> { self.fill_bytes(out); Ok(()) }
}
impl rand_core::CryptoRng for RadioRng {}

fn now_ms() -> u64 { Instant::now().duration_since_epoch().as_millis() }

pub use crate::ble_bonds::{encode_bonds, decode_bonds};

impl BleHost {
    pub fn new(connector: BleConnector<'static>, address: [u8; 6], bonds: Vec<BondInformation, 4>) -> Self {
        let state = Rc::new(RefCell::new(State { enabled: true, bonds, ..Default::default() }));
        Self { task: Box::pin(run(connector, address, state.clone())), state, task_finished: false }
    }
    pub fn poll(&mut self) {
        let mut cx = Context::from_waker(core::task::Waker::noop());
        // Poll once per OS iteration; never block waiting for a peer.
        if self.task_finished { return; }
        if let Poll::Ready(()) = self.task.as_mut().poll(&mut cx) {
            self.task_finished = true;
            self.state.borrow_mut().status = 5;
        }
    }
    pub fn set_enabled(&mut self, value: bool) {
        let mut s = self.state.borrow_mut(); s.enabled = value;
        if !value { s.disconnect = true; s.pair_until = 0; s.media.clear(); s.pin = None; }
    }
    pub fn pair(&mut self) {
        let mut s = self.state.borrow_mut(); s.enabled = true;
        s.pair_until = now_ms() + 120_000; s.disconnect = true; s.pin = None; s.decision = None;
    }
    pub fn confirm(&mut self, yes: bool) {
        let mut s = self.state.borrow_mut(); s.decision = Some(yes);
        if !yes { s.pair_until = 0; }
    }
    pub fn forget(&mut self) {
        let mut s = self.state.borrow_mut(); s.forget = true; s.disconnect = true; s.pair_until = 0;
    }
    pub fn disconnect(&mut self) { self.state.borrow_mut().disconnect = true; }
    pub fn media(&mut self, usage: u16) -> bool {
        let mut s = self.state.borrow_mut();
        if s.status != 3 || !s.subscribed { return false; }
        // Consumer Control usage IDs: Play/Pause, Next, Previous, Stop, Mute, Volume +/-.
        if !matches!(usage, 0x00CD | 0x00B5 | 0x00B6 | 0x00B7 | 0x00E2 | 0x00E9 | 0x00EA) {
            return false;
        }
        s.media.push(usage).is_ok()
    }
    pub fn snapshot(&self) -> (u8, Option<u32>) { let s = self.state.borrow(); (s.status, s.pin) }
    pub fn bonds_to_save(&self) -> Option<Vec<BondInformation, 4>> {
        let s = self.state.borrow(); if s.bonds_dirty { Some(s.bonds.clone()) } else { None }
    }
    pub fn saved(&mut self) { self.state.borrow_mut().bonds_dirty = false; }
}

async fn run(connector: BleConnector<'static>, mut address: [u8; 6], shared: Rc<RefCell<State>>) {
    let controller: ExternalController<_, 8> = ExternalController::new(connector);
    let mut resources: HostResources<DefaultPacketPool, 1, 3> = HostResources::new();
    // Stable per-board static-random identity, distinct from its public address.
    address[5] |= 0xc0;
    let stack = trouble_host::new(controller, &mut resources)
        .set_random_address(Address::random(address))
        .set_random_generator_seed(&mut RadioRng)
        .set_io_capabilities(IoCapabilities::DisplayYesNo);
    for bond in shared.borrow().bonds.iter() { let _ = stack.add_bond_information(bond.clone()); }
    let Host { mut peripheral, mut runner, .. } = stack.build();
    let Ok(server) = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "AOSP Wear", appearance: &appearance::human_interface_device::GENERIC_HUMAN_INTERFACE_DEVICE,
    })) else { shared.borrow_mut().status = 5; return; };
    let control = async {
        loop {
            if shared.borrow().forget {
                let mut s = shared.borrow_mut();
                for b in s.bonds.iter() { let _ = stack.remove_bond_information(b.identity.clone()); }
                s.bonds.clear(); s.bonds_dirty = true; s.forget = false;
            }
            if !shared.borrow().enabled {
                shared.borrow_mut().status = 0; Timer::after_millis(50).await; continue;
            }
            shared.borrow_mut().status = 1;
            let mut adv = [0; 31];
            let Ok(len) = AdStructure::encode_slice(&[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::ServiceUuids16(&[0x1812u16.to_le_bytes()]),
                AdStructure::CompleteLocalName(b"AOSP Wear"),
            ], &mut adv) else { shared.borrow_mut().status = 5; return; };
            let payload = Advertisement::ConnectableScannableUndirected {
                adv_data: &adv[..len], scan_data: &[],
            };
            let Ok(advertiser) = peripheral.advertise(&Default::default(), payload).await else {
                shared.borrow_mut().status = 5; Timer::after_millis(1000).await; continue;
            };
            let idle = async {
                loop {
                    Timer::after_millis(50).await;
                    if !shared.borrow().enabled || shared.borrow().forget { return; }
                }
            };
            let raw = match select(advertiser.accept(), idle).await {
                Either::First(Ok(conn)) => conn,
                _ => continue,
            };
            let Ok(conn) = raw.with_attribute_server(&server) else { continue; };
            {
                let mut s = shared.borrow_mut(); s.status = 2; s.disconnect = false; s.pin = None; s.media.clear(); s.subscribed = false;
            }
            let pairable = now_ms() < shared.borrow().pair_until;
            let _ = conn.raw().set_bondable(pairable && shared.borrow().bonds.len() < 4);
            // Existing bonds can reconnect outside the pairing window. Unknown
            // peers must be explicitly admitted from the physical watch UI.
            let identity = conn.raw().peer_identity();
            let known = shared.borrow().bonds.iter().any(|b| b.identity.match_identity(&identity));
            if !pairable && !known {
                conn.raw().disconnect();
                continue;
            }
            if let Err(err) = conn.raw().request_security() {
                esp_println::println!("BLE: security request failed: {:?}", err);
                conn.raw().disconnect();
                continue;
            }
            let events = async {
                loop {
                    match conn.next().await {
                        GattConnectionEvent::Disconnected { .. } => return,
                        GattConnectionEvent::PassKeyConfirm(pin) => {
                            let mut s = shared.borrow_mut(); s.pin = Some(pin.value()); s.status = 4;
                        }
                        GattConnectionEvent::PassKeyDisplay(pin) => {
                            let mut s = shared.borrow_mut(); s.pin = Some(pin.value()); s.status = 4;
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
                                    esp_println::println!("BLE: active bond install failed: {:?}", err);
                                    shared.borrow_mut().status = 5;
                                    conn.raw().disconnect();
                                    return;
                                }
                                let mut s = shared.borrow_mut();
                                if let Some(i) = s.bonds.iter().position(|b| b.identity.match_identity(&bond.identity)) {
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
                            esp_println::println!("BLE: pairing failed: {:?}", reason);
                            let mut s = shared.borrow_mut();
                            s.status = 5;
                            s.pin = None;
                            s.decision = None;
                            drop(s);
                            conn.raw().disconnect();
                            return;
                        }
                        GattConnectionEvent::Gatt { event } => {
                            // Require encryption for reports/control; discovery
                            // and the report map remain readable to HID hosts.
                            let sensitive = match &event {
                                GattEvent::Read(e) => e.handle() == server.hid.report.handle,
                                GattEvent::Write(_) => true, _ => false,
                            };
                            let encrypted = conn.raw().security_level().map(|s| s.encrypted()).unwrap_or(false);
                            let subscription = match &event {
                                GattEvent::Write(e) if Some(e.handle()) == server.hid.report.cccd_handle && e.data().len() == 2 => Some(e.data() == [1, 0]),
                                _ => None,
                            };
                            let reply = if sensitive && !encrypted { event.reject(AttErrorCode::INSUFFICIENT_ENCRYPTION) } else { event.accept() };
                            if let Ok(reply) = reply {
                                if encrypted { if let Some(on) = subscription { shared.borrow_mut().subscribed = on; } }
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
                        (s.enabled, s.disconnect, s.decision.take(), s.pin.is_some() && now_ms() >= s.pair_until)
                    };
                    if !conn.raw().is_connected() { return; }
                    if !enabled || disconnect || expired { conn.raw().disconnect(); return; }
                    if let Some(yes) = decision {
                        if yes { let _ = conn.raw().pass_key_confirm(); } else { let _ = conn.raw().pass_key_cancel(); }
                    }
                    let encrypted = conn.raw().security_level().map(|s| s.encrypted()).unwrap_or(false);
                    if encrypted {
                        if shared.borrow().pin.is_none() {
                            let ready = shared.borrow().subscribed;
                            shared.borrow_mut().status = if ready { 3 } else { 2 };
                        }
                        let usage = {
                            let mut s = shared.borrow_mut();
                            if !s.subscribed || s.media.is_empty() { None } else { Some(s.media.remove(0)) }
                        };
                        if let Some(usage) = usage {
                            // Every key-down is followed by key-up, preventing
                            // stuck volume/play keys on Android or Linux.
                            let down = server.hid.report.notify(&conn, &usage.to_le_bytes()).await;
                            Timer::after_millis(35).await;
                            let up = server.hid.report.notify(&conn, &[0, 0]).await;
                            if down.is_err() || up.is_err() {
                                esp_println::println!("BLE: HID notify failed; dropping connection");
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
    let _ = join(async {
        if let Err(e) = runner.run().await { esp_println::println!("BLE host stopped: {:?}", e); shared.borrow_mut().status = 5; }
    }, control).await;
}