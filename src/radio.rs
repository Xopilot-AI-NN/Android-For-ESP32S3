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

    pub fn set_wifi(&mut self, enabled: bool) {
        if enabled {
            if !self.ensure_wifi_started() { return; }
            self.wifi_requested = true;
            // Do not perform a synchronous active scan merely because Wi-Fi
            // was toggled on. esp-radio scanning can hold the main loop for a
            // few seconds and, on USB-PC storage builds, starve the shared
            // USB-Serial-JTAG transport. Wear-style network discovery is
            // requested explicitly when the Available networks page opens.
            if let Some(credentials) = self.saved_wifi {
                let _ = self.connect_wifi(credentials);
            }
        } else {
            self.disconnect_wifi();
        }
    }

    fn ensure_wifi_started(&mut self) -> bool {
        if matches!(self.wifi.is_started(), Ok(true)) { return true; }
        let free = internal_free_bytes();
        if free < MIN_WIFI_INTERNAL_FREE {
            println!(
                "radio: wifi start refused: only {} KiB internal heap free (need >= {} KiB)",
                free / 1024,
                MIN_WIFI_INTERNAL_FREE / 1024
            );
            return false;
        }
        if let Err(e) = self.wifi.set_config(&ModeConfig::Client(ClientConfig::default())) {
            println!("radio: wifi config failed: {:?}", e);
            return false;
        }
        if let Err(e) = self.wifi.start() {
            println!("radio: wifi start failed: {:?}", e);
            return false;
        }
        // start() is explicitly non-blocking in esp-radio 0.17.  Scanning
        // immediately after it can race StaStart and yield a misleading empty
        // list, so wait briefly for the driver state machine to settle.
        let delay = Delay::new();
        let mut started = false;
        for _ in 0..100 {
            if matches!(self.wifi.is_started(), Ok(true)) { started = true; break; }
            delay.delay_millis(10);
        }
        if !started {
            println!("radio: wifi start timed out");
            return false;
        }
        self.net.set_link_state(LinkState::LinkUp);
        println!(
            "radio: wifi STA enabled (internal_free={} KiB)",
            internal_free_bytes() / 1024
        );
        true
    }

    pub fn scan_wifi(&mut self) -> bool {
        if !self.ensure_wifi_started() { return false; }
        self.wifi_requested = true;
        self.wifi_aps.clear();
        println!("radio: scanning Wi-Fi networks...");
        // Keep explicit foreground discovery short enough that the crown UI
        // and shared USB debug/storage transport remain responsive. Nearby
        // watch/phone hotspots are normally discovered within this dwell; an
        // empty first pass is retried below after the PHY settles.
        let scan_config = ScanConfig::default()
            .with_scan_type(ScanTypeConfig::Active {
                min: Duration::from_millis(20),
                max: Duration::from_millis(70),
            })
            .with_max(10);
        let mut result = self.wifi.scan_with_config(scan_config);
        if matches!(&result, Ok(aps) if aps.is_empty()) {
            // A first scan right after radio bring-up can still race the PHY.
            Delay::new().delay_millis(120);
            result = self.wifi.scan_with_config(scan_config);
        }
        match result {
            Ok(aps) => {
                for ap in aps.iter() {
                    let mut ssid = HString::<32>::new();
                    push_safe_text(&mut ssid, ap.ssid.as_str());
                    let _ = self.wifi_aps.push(WifiAp {
                        ssid,
                        rssi: ap.signal_strength,
                        channel: ap.channel,
                    });
                }
                println!("radio: scan found {} AP(s)", self.wifi_aps.len());
                for ap in self.wifi_aps.iter() {
                    println!("radio: ap ssid={} rssi={} ch={}", ap.ssid, ap.rssi, ap.channel);
                }
                true
            }
            Err(e) => {
                println!("radio: scan failed: {:?}", e);
                false
            }
        }
    }

    pub fn connect_wifi(&mut self, credentials: WifiCredentials) -> bool {
        if !self.ensure_wifi_started() { return false; }
        let Some(ssid) = credentials.ssid() else {
            println!("radio: invalid SSID encoding");
            return false;
        };
        let Some(password) = credentials.password() else {
            println!("radio: invalid password encoding");
            return false;
        };
        if ssid.is_empty() {
            println!("radio: refusing empty SSID");
            return false;
        }

        if self.saved_wifi == Some(credentials) && self.wifi.is_connected().unwrap_or(false) {
            self.wifi_requested = true;
            println!("radio: already associated with {}; keeping current link", ssid);
            return true;
        }

        // Proven-good ESP32-S3 path from Zephyr 0.4: let the Espressif
        // station driver perform its own AP selection.  Scans remain available
        // for UI/diagnostics, but we deliberately do not pin BSSID/channel here.
        // Phone hotspots can rotate/reconfigure their BSSID while remaining on
        // the same SSID and pinning the scan result made association fragile.
        let _ = self.wifi.disconnect();
        let config = ModeConfig::Client(
            ClientConfig::default()
                .with_ssid(ssid.into())
                .with_password(password.into()),
        );
        if let Err(e) = self.wifi.set_config(&config) {
            println!("radio: set network failed: {:?}", e);
            return false;
        }
        if !matches!(self.wifi.is_started(), Ok(true)) {
            if let Err(e) = self.wifi.start() {
                println!("radio: wifi start before association failed: {:?}", e);
                return false;
            }
        }

        match self.wifi.connect() {
            Ok(()) => {
                self.connecting_wifi = Some(credentials);
                self.wifi_requested = true;
                self.wifi_connect_started = Some(now_ms());
                self.wifi_reconnect_at = 0;
                self.net.set_link_state(LinkState::Associating);
                println!("radio: associating with {} (driver AP selection)", ssid);
                true
            }
            Err(e) => {
                println!("radio: connect request failed: {:?}", e);
                self.wifi_connect_started = None;
                self.wifi_reconnect_at = now_ms() + WIFI_RECONNECT_BACKOFF_MS;
                false
            }
        }
    }

    fn reset_wifi_station(&mut self) {
        let delay = Delay::new();
        let _ = self.wifi.disconnect();
        delay.delay_millis(WIFI_DRIVER_SETTLE_MS);

        if matches!(self.wifi.is_started(), Ok(true)) {
            if let Err(e) = self.wifi.stop() {
                println!("radio: wifi stop during reset failed: {:?}", e);
            }
            for _ in 0..100 {
                if !matches!(self.wifi.is_started(), Ok(true)) {
                    break;
                }
                delay.delay_millis(10);
            }
        }

        let _ = self.wifi.set_config(&ModeConfig::None);
        self.net.reset();
    }

    pub fn disconnect_wifi(&mut self) {
        let _ = self.wifi.disconnect();
        if matches!(self.wifi.is_started(), Ok(true)) {
            if let Err(e) = self.wifi.stop() {
                println!("radio: wifi stop failed: {:?}", e);
            }
        }
        let _ = self.wifi.set_config(&ModeConfig::None);
        self.wifi_requested = false;
        self.connecting_wifi = None;
        self.wifi_connect_started = None;
        self.wifi_reconnect_at = 0;
        self.wifi_failures = 0;
        self.net.reset();
        println!("radio: wifi disabled");
    }

    pub fn format_wifi_status<const N: usize>(&self, out: &mut HString<N>) {
        let _ = write!(out, "enabled={} state={:?} aps={}", self.wifi_requested as u8, self.net.link_state(), self.wifi_aps.len());
        if let Some(ip) = self.net.ip() { let _ = write!(out, " ip={} web=http://{}/", ip, ip); }
        if matches!(self.net.link_state(), LinkState::Online) {
            if let Ok(rssi) = self.wifi.rssi() { let _ = write!(out, " rssi={}", rssi); }
        }
        if let Some(c) = self.saved_wifi {
            if let Some(ssid) = c.ssid() {
                let _ = out.push_str(" saved=");
                push_safe_text(out, ssid);
            }
        }
        let _ = write!(out, " wadb={} listener={} client={}", self.net.wadb_active() as u8, self.net.wadb_active() as u8, self.net.wadb_seen_client() as u8);
    }

    pub fn format_wifi_scan<const N: usize>(&self, out: &mut HString<N>) {
        if self.wifi_aps.is_empty() {
            let _ = out.push_str("no access points found");
            return;
        }
        for (i, ap) in self.wifi_aps.iter().enumerate() {
            if i != 0 { let _ = out.push_str("; "); }
            let _ = write!(out, "{} rssi={} ch={}", ap.ssid, ap.rssi, ap.channel);
        }
    }

    pub fn set_bluetooth(&mut self, enabled: bool) {
        if enabled {
            if !self.ensure_ble() { return; }
            self.bt_requested = true;
            if !self.ble_advertising && self.ble_scanning_until.is_none() {
                self.enable_advertising();
            }
            println!("radio: bluetooth service enabled");
        } else {
            if let Some(ble) = self.ble.as_mut() {
                if self.ble_scanning_until.is_some() { let _ = hci_command(ble, 0x200c, &[0, 0]); }
                if self.ble_advertising { let _ = hci_command(ble, 0x200a, &[0]); }
                if let Some(connection) = self.ble_connection {
                    let params = [connection.handle as u8, (connection.handle >> 8) as u8, 0x13];
                    let _ = hci_command(ble, 0x0406, &params);
                }
            }
            self.ble_scanning_until = None;
            self.ble_advertising = false;
            self.ble_pending_address = None;
            self.ble_connection = None;
            self.bt_requested = false;
            println!("radio: bluetooth service disabled");
        }
    }

    fn ensure_ble(&mut self) -> bool {
        if self.ble.is_some() { return true; }
        let free = internal_free_bytes();
        if free < MIN_BLE_INTERNAL_FREE {
            println!(
                "radio: bluetooth start refused: only {} KiB internal heap free (need >= {} KiB)",
                free / 1024,
                MIN_BLE_INTERNAL_FREE / 1024
            );
            return false;
        }
        let Some(bt) = self.bt_device.take() else {
            println!("radio: bluetooth peripheral unavailable until reboot");
            return false;
        };
        match BleConnector::new(self.controller, bt, Default::default()) {
            Ok(connector) => {
                self.ble = Some(connector);
                println!("radio: BLE HCI initialized (internal_free={} KiB)", internal_free_bytes() / 1024);
                true
            }
            Err(e) => {
                println!("radio: BLE HCI init failed: {:?}", e);
                false
            }
        }
    }

    fn enable_advertising(&mut self) {
        let Some(ble) = self.ble.as_mut() else { return; };
        // ADV_IND, public address, all channels, 100 ms interval.
        let params = [0xa0,0x00, 0xa0,0x00, 0x00, 0x00, 0x00, 0,0,0,0,0,0, 0x07, 0x00];
        if !hci_command(ble, 0x2006, &params) { return; }

        let mut data = [0u8; 32];
        let name = b"AOSP Wear OS";
        let ad_len = 3 + 2 + name.len();
        data[0] = ad_len as u8;
        data[1..4].copy_from_slice(&[2, 0x01, 0x06]);
        data[4] = (name.len() + 1) as u8;
        data[5] = 0x09;
        data[6..6 + name.len()].copy_from_slice(name);
        if !hci_command(ble, 0x2008, &data) { return; }
        if hci_command(ble, 0x200a, &[1]) {
            self.ble_advertising = true;
            println!("radio: BLE advertising as 'AOSP Wear OS'");
        }
    }

    pub fn start_ble_scan(&mut self) -> bool {
        if !self.ensure_ble() { return false; }
        self.bt_requested = true;
        let Some(ble) = self.ble.as_mut() else { return false; };
        if self.ble_advertising {
            let _ = hci_command(ble, 0x200a, &[0]);
            self.ble_advertising = false;
        }
        // Active scan, 100 ms interval / 50 ms window, public address.
        let params = [0x01, 0xa0,0x00, 0x50,0x00, 0x00, 0x00];
        if !hci_command(ble, 0x200b, &params) { return false; }
        if !hci_command(ble, 0x200c, &[1, 1]) { return false; }
        self.ble_devices.clear();
        self.ble_scanning_until = Some(now_ms() + BLE_SCAN_MS);
        println!("radio: BLE active scan started");
        true
    }

    pub fn connect_ble(&mut self, address: [u8; 6]) -> bool {
        if !self.ensure_ble() { return false; }
        self.bt_requested = true;
        if self.ble_connection.is_some() {
            println!("radio: BLE already connected");
            return false;
        }
        let address_type = self.ble_devices.iter()
            .find(|d| d.address == address)
            .map(|d| d.address_type)
            .unwrap_or(0x00);
        let Some(ble) = self.ble.as_mut() else { return false; };
        if self.ble_scanning_until.is_some() {
            let _ = hci_command(ble, 0x200c, &[0, 0]);
            self.ble_scanning_until = None;
        }
        if self.ble_advertising {
            let _ = hci_command(ble, 0x200a, &[0]);
            self.ble_advertising = false;
        }
        // LE Create Connection.  The address is stored in HCI byte order.
        let mut params = [0u8; 25];
        params[0..2].copy_from_slice(&0x0060u16.to_le_bytes()); // scan interval 60 ms
        params[2..4].copy_from_slice(&0x0030u16.to_le_bytes()); // scan window 30 ms
        params[4] = 0x00; // use peer address below
        params[5] = address_type;
        params[6..12].copy_from_slice(&address);
        params[12] = 0x00; // public own address
        params[13..15].copy_from_slice(&0x0018u16.to_le_bytes()); // 30 ms min
        params[15..17].copy_from_slice(&0x0028u16.to_le_bytes()); // 50 ms max
        params[17..19].copy_from_slice(&0u16.to_le_bytes());
        params[19..21].copy_from_slice(&0x01f4u16.to_le_bytes()); // 5 s supervision
        params[21..23].copy_from_slice(&0u16.to_le_bytes());
        params[23..25].copy_from_slice(&0u16.to_le_bytes());
        if hci_command(ble, 0x200d, &params) {
            self.ble_pending_address = Some(address);
            println!("radio: BLE connection requested");
            true
        } else {
            false
        }
    }

    pub fn disconnect_ble(&mut self) -> bool {
        let Some(connection) = self.ble_connection else {
            self.ble_pending_address = None;
            return false;
        };
        let Some(ble) = self.ble.as_mut() else { return false; };
        let params = [connection.handle as u8, (connection.handle >> 8) as u8, 0x13];
        if hci_command(ble, 0x0406, &params) {
            println!("radio: BLE disconnect requested handle={}", connection.handle);
            true
        } else { false }
    }

    fn poll_ble(&mut self) {
        if self.ble.is_none() { return; }
        {
            let ble = self.ble.as_mut().expect("BLE checked above");
            let mut packet = [0u8; 260];
            for _ in 0..8 {
                let Ok(n) = ble.next(&mut packet) else { break; };
                if n == 0 { break; }
                parse_ble_event(
                    &packet[..n],
                    &mut self.ble_devices,
                    &mut self.ble_connection,
                    &mut self.ble_pending_address,
                );
            }
            if let Some(until) = self.ble_scanning_until {
                if now_ms() >= until {
                    let _ = hci_command(ble, 0x200c, &[0, 0]);
                    self.ble_scanning_until = None;
                    println!("radio: BLE scan complete: {} device(s)", self.ble_devices.len());
                }
            }
        }
        if self.bt_requested
            && self.ble_connection.is_none()
            && self.ble_pending_address.is_none()
            && self.ble_scanning_until.is_none()
            && !self.ble_advertising
        {
            self.enable_advertising();
        }
    }

    pub fn format_bt_status<const N: usize>(&self, out: &mut HString<N>) {
        let _ = write!(
            out,
            "enabled={} hci={} advertising={} scanning={} devices={} connected={} pending={}",
            self.bt_requested as u8,
            self.ble.is_some() as u8,
            self.ble_advertising as u8,
            self.ble_scanning_until.is_some() as u8,
            self.ble_devices.len(),
            self.ble_connection.is_some() as u8,
            self.ble_pending_address.is_some() as u8,
        );
        if let Some(connection) = self.ble_connection {
            let a = connection.address;
            let _ = write!(out, " peer={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", a[5],a[4],a[3],a[2],a[1],a[0]);
        }
    }

    pub fn format_bt_scan<const N: usize>(&self, out: &mut HString<N>) {
        if self.ble_devices.is_empty() {
            let _ = out.push_str(if self.ble_scanning_until.is_some() { "scan in progress" } else { "no BLE devices discovered" });
            return;
        }
        for (i, dev) in self.ble_devices.iter().enumerate() {
            if i != 0 { let _ = out.push_str("; "); }
            if dev.name.is_empty() { let _ = out.push_str("BLE"); } else { let _ = out.push_str(dev.name.as_str()); }
            let a = dev.address;
            let _ = write!(out, " {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} rssi={}", a[5],a[4],a[3],a[2],a[1],a[0], dev.rssi);
        }
    }
}

fn push_safe_text<const N: usize>(out: &mut HString<N>, text: &str) {
    for ch in text.chars() {
        let safe = if ch == '\r' || ch == '\n' || ch == '|' { '?' } else { ch };
        if out.push(safe).is_err() { break; }
    }
}

fn hci_command(ble: &mut BleConnector<'static>, opcode: u16, params: &[u8]) -> bool {
    if params.len() > 255 { return false; }
    let mut packet = [0u8; 260];
    packet[0] = 0x01;
    packet[1..3].copy_from_slice(&opcode.to_le_bytes());
    packet[3] = params.len() as u8;
    packet[4..4 + params.len()].copy_from_slice(params);
    if let Err(e) = ble.write(&packet[..4 + params.len()]) {
        println!("radio: HCI command 0x{:04x} failed: {:?}", opcode, e);
        return false;
    }
    // Give the controller task a short opportunity to consume the command. A
    // command-complete event is intentionally left for poll_ble to drain.
    Delay::new().delay_millis(8);
    true
}

fn parse_ble_event(
    packet: &[u8],
    out: &mut HVec<BleDevice, 10>,
    connection: &mut Option<BleConnection>,
    pending: &mut Option<[u8; 6]>,
) {
    // VHCI uses H4 framing. Be tolerant of a backend without the H4 type byte.
    let (event_code, params) = if packet.len() >= 3 && packet[0] == 0x04 {
        let plen = packet[2] as usize;
        if packet.len() < 3 + plen { return; }
        (packet[1], &packet[3..3 + plen])
    } else if packet.len() >= 2 {
        let plen = packet[1] as usize;
        if packet.len() < 2 + plen { return; }
        (packet[0], &packet[2..2 + plen])
    } else { return; };

    // LE Meta Event.
    if event_code == 0x3e && !params.is_empty() {
        match params[0] {
            0x01 => { // LE Connection Complete
                if params.len() < 12 { return; }
                let status = params[1];
                let handle = u16::from_le_bytes([params[2], params[3]]) & 0x0fff;
                let mut address = [0u8; 6];
                address.copy_from_slice(&params[6..12]);
                if status == 0 {
                    *connection = Some(BleConnection { address, handle });
                    *pending = None;
                    println!("radio: BLE connected handle={}", handle);
                } else {
                    *pending = None;
                    println!("radio: BLE connection failed status=0x{:02x}", status);
                }
            }
            0x02 => parse_advertising_params(params, out),
            _ => {}
        }
        return;
    }

    // Disconnection Complete.
    if event_code == 0x05 && params.len() >= 4 {
        let handle = u16::from_le_bytes([params[1], params[2]]) & 0x0fff;
        if connection.as_ref().map(|c| c.handle) == Some(handle) {
            *connection = None;
            println!("radio: BLE disconnected handle={} reason=0x{:02x}", handle, params[3]);
        }
    }
}

fn parse_advertising_params(params: &[u8], out: &mut HVec<BleDevice, 10>) {
    if params.len() < 2 || params[0] != 0x02 { return; }
    let reports = params[1] as usize;
    let mut pos = 2usize;
    for _ in 0..reports {
        if pos + 9 > params.len() { break; }
        let _event_type = params[pos];
        let address_type = params[pos + 1];
        let mut address = [0u8; 6];
        address.copy_from_slice(&params[pos + 2..pos + 8]);
        let data_len = params[pos + 8] as usize;
        pos += 9;
        if pos + data_len + 1 > params.len() { break; }
        let data = &params[pos..pos + data_len];
        pos += data_len;
        let rssi = params[pos] as i8;
        pos += 1;

        let mut name = HString::<24>::new();
        parse_local_name(data, &mut name);
        if let Some(existing) = out.iter_mut().find(|d| d.address == address) {
            existing.rssi = rssi;
            existing.address_type = address_type;
            if !name.is_empty() { existing.name = name; }
        } else {
            let _ = out.push(BleDevice { address, address_type, rssi, name });
        }
    }
}

fn parse_local_name(data: &[u8], out: &mut HString<24>) {
    let mut pos = 0usize;
    while pos < data.len() {
        let len = data[pos] as usize;
        if len == 0 || pos + 1 + len > data.len() { break; }
        let kind = data[pos + 1];
        if kind == 0x09 || kind == 0x08 {
            for &b in &data[pos + 2..pos + 1 + len] {
                let ch = if b.is_ascii_graphic() || b == b' ' { b as char } else { '?' };
                if out.push(ch).is_err() { break; }
            }
            return;
        }
        pos += 1 + len;
    }
}
