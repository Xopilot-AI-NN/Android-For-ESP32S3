//! Native ESP32-S3 connectivity services for AOSP Wear OS.
//!
//! Wi-Fi uses esp-radio for the 802.11 station and smoltcp for DHCP/TCP.
//! Bluetooth exposes a real BLE controller: the watch advertises itself and can
//! perform active discovery using raw HCI without requiring a second heap-heavy
//! host stack just for scanning.

use core::{fmt::Write, time::Duration};

use esp_hal::{delay::Delay, peripherals::{BT, WIFI}, time::Instant};
use esp_println::println;
use esp_radio::{
    Controller,
    ble::controller::BleConnector,
    wifi::{ClientConfig, ModeConfig, ScanConfig, ScanTypeConfig, WifiController, WifiDevice},
};
use heapless::{String as HString, Vec as HVec};
use static_cell::StaticCell;

use crate::{desktop::WifiCredentials, net::{LinkState, NetworkService, WebRemoteEvent}, runtime::RuntimeFrame};

static RADIO: StaticCell<Controller<'static>> = StaticCell::new();

const MIN_WIFI_INTERNAL_FREE: usize = 32 * 1024;
const MIN_BLE_INTERNAL_FREE: usize = 24 * 1024;
const WIFI_CONNECT_TIMEOUT_MS: u64 = 30_000;
const WIFI_RECONNECT_BACKOFF_MS: u64 = 5_000;
const WIFI_DRIVER_SETTLE_MS: u32 = 150;
const BLE_SCAN_MS: u64 = 6_000;

fn internal_free_bytes() -> usize {
    let stats = esp_alloc::HEAP.stats();
    stats
        .region_stats
        .iter()
        .filter_map(|region| region.as_ref())
        .filter(|region| region.capabilities.contains(esp_alloc::MemoryCapability::Internal))
        .map(|region| region.free)
        .sum()
}

fn now_ms() -> u64 {
    Instant::now().duration_since_epoch().as_millis()
}

#[derive(Clone, Debug)]
struct WifiAp {
    ssid: HString<32>,
    rssi: i8,
    channel: u8,
}

#[derive(Clone, Debug)]
struct BleDevice {
    address: [u8; 6],
    address_type: u8,
    rssi: i8,
    name: HString<24>,
}

#[derive(Clone, Copy, Debug)]
struct BleConnection {
    address: [u8; 6],
    handle: u16,
}

pub struct RadioServices {
    controller: &'static Controller<'static>,
    wifi: WifiController<'static>,
    wifi_device: WifiDevice<'static>,
    net: NetworkService,
    bt_device: Option<BT<'static>>,
    ble: Option<BleConnector<'static>>,
    wifi_requested: bool,
    bt_requested: bool,
    saved_wifi: Option<WifiCredentials>,
    // Candidate credentials are not committed to persistent storage until the
    // station actually associates. This prevents a one-key keyboard typo from
    // replacing the last known-good profile and creating an endless retry loop.
    connecting_wifi: Option<WifiCredentials>,
    persist_wifi: Option<WifiCredentials>,
    wifi_connect_started: Option<u64>,
    wifi_reconnect_at: u64,
    wifi_failures: u8,
    wifi_aps: HVec<WifiAp, 10>,
    ble_devices: HVec<BleDevice, 10>,
    ble_scanning_until: Option<u64>,
    ble_advertising: bool,
    ble_pending_address: Option<[u8; 6]>,
    ble_pending_since: Option<u64>,
    ble_connection: Option<BleConnection>,
}

impl RadioServices {
    pub fn new(wifi: WIFI<'static>, bt: BT<'static>) -> Option<Self> {
        let controller: &'static Controller<'static> = match esp_radio::init() {
            Ok(c) => &*RADIO.init(c),
            Err(e) => {
                println!("radio: init failed: {:?}", e);
                return None;
            }
        };

        let (wifi_controller, interfaces) = match esp_radio::wifi::new(controller, wifi, Default::default()) {
            Ok(v) => v,
            Err(e) => {
                println!("radio: wifi init failed: {:?}", e);
                return None;
            }
        };
        let mut wifi_device = interfaces.sta;
        let net = NetworkService::new(&mut wifi_device);

        println!(
            "radio: ESP32-S3 Wi-Fi ready; stable bring-up profile; BLE deferred (internal_free={} KiB)",
            internal_free_bytes() / 1024
        );

        Some(Self {
            controller,
            wifi: wifi_controller,
            wifi_device,
            net,
            bt_device: Some(bt),
            ble: None,
            wifi_requested: false,
            bt_requested: false,
            saved_wifi: None,
            connecting_wifi: None,
            persist_wifi: None,
            wifi_connect_started: None,
            wifi_reconnect_at: 0,
            wifi_failures: 0,
            wifi_aps: HVec::new(),
            ble_devices: HVec::new(),
            ble_scanning_until: None,
            ble_advertising: false,
            ble_pending_address: None,
            ble_pending_since: None,
            ble_connection: None,
        })
    }

    pub fn set_saved_wifi(&mut self, credentials: WifiCredentials) {
        self.saved_wifi = Some(credentials);
    }

    pub fn network_state(&self) -> LinkState { self.net.link_state() }
    pub fn ip_address(&self) -> Option<smoltcp::wire::Ipv4Address> { self.net.ip() }
    pub fn take_network_time(&mut self) -> Option<u32> { self.net.take_network_time() }
    pub fn request_network_time_sync(&mut self) { self.net.request_time_sync(); }
    pub fn take_web_event(&mut self) -> Option<WebRemoteEvent> { self.net.take_web_event() }
    pub fn take_connected_wifi_for_persist(&mut self) -> Option<WifiCredentials> { self.persist_wifi.take() }
    pub fn wifi_ap_count(&self) -> usize { self.wifi_aps.len() }
    pub fn wifi_ap_info(&self, index: usize) -> Option<(&str, i8)> {
        self.wifi_aps.get(index).map(|ap| (ap.ssid.as_str(), ap.rssi))
    }

    pub fn sync_frame(&mut self, frame: &RuntimeFrame) {
        self.net.set_web_frame(frame);
        let RuntimeFrame::Material(m) = frame else { return; };
        if m.wifi != self.wifi_requested { self.set_wifi(m.wifi); }
        if m.bt != self.bt_requested { self.set_bluetooth(m.bt); }
    }

    pub fn tick(&mut self, wadb_enabled: bool) {
        if self.wifi_requested {
            let now = now_ms();
            let connected = self.wifi.is_connected().unwrap_or(false);
            if connected {
                if !matches!(self.net.link_state(), LinkState::Dhcp | LinkState::Online) {
                    println!("radio: Wi-Fi associated; requesting DHCP");
                    self.net.set_link_state(LinkState::Dhcp);
                }
                if let Some(credentials) = self.connecting_wifi.take() {
                    if self.saved_wifi != Some(credentials) {
                        self.saved_wifi = Some(credentials);
                        self.persist_wifi = Some(credentials);
                        println!("radio: candidate Wi-Fi profile verified; queued for persistence");
                    }
                }
                self.wifi_connect_started = None;
                self.wifi_reconnect_at = 0;
                self.wifi_failures = 0;
            } else if let Some(start) = self.wifi_connect_started {
                if now.saturating_sub(start) > WIFI_CONNECT_TIMEOUT_MS {
                    self.wifi_failures = self.wifi_failures.saturating_add(1);
                    println!(
                        "radio: wifi association timeout (attempt {}); retry scheduled",
                        self.wifi_failures
                    );
                    // Keep the STA driver running for the first retries.  esp-radio's
                    // own stress path reconnects repeatedly without destroying the
                    // controller, and this also matches the path that proved stable
                    // with the phone hotspot in Zephyr 0.4/0.5.
                    let _ = self.wifi.disconnect();
                    if self.wifi_failures >= 3 {
                        println!("radio: three failed associations; rejecting unverified candidate and resetting STA");
                        if self.connecting_wifi.is_some() && self.connecting_wifi != self.saved_wifi {
                            self.connecting_wifi = None;
                        }
                        self.reset_wifi_station();
                        self.wifi_failures = 0;
                    }
                    self.wifi_connect_started = None;
                    self.wifi_reconnect_at = now + WIFI_RECONNECT_BACKOFF_MS;
                    self.net.set_link_state(LinkState::LinkUp);
                }
            } else {
                if matches!(self.net.link_state(), LinkState::Dhcp | LinkState::Online) {
                    println!("radio: Wi-Fi link lost; reconnect scheduled");
                    self.net.set_link_state(LinkState::LinkUp);
                    self.wifi_reconnect_at = now + 1_500;
                }
                if now >= self.wifi_reconnect_at {
                    self.wifi_reconnect_at = now + WIFI_RECONNECT_BACKOFF_MS;
                    if let Some(credentials) = self.connecting_wifi.or(self.saved_wifi) {
                        println!("radio: reconnecting {} Wi-Fi profile", if self.connecting_wifi.is_some() { "candidate" } else { "saved" });
                        let _ = self.connect_wifi(credentials);
                    }
                }
            }
            self.net.poll(&mut self.wifi_device, wadb_enabled);
        } else {
            self.net.poll(&mut self.wifi_device, false);
        }

        self.poll_ble();
    }
}

mod wifi;
mod ble;
mod hci;
use hci::{hci_command, parse_ble_event, push_safe_text};
