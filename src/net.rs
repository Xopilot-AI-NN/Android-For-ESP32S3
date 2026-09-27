//! Minimal IPv4/DHCP + wireless ADB transport for Zephyr Android.
//!
//! This stays deliberately small: esp-radio owns the 802.11 link, smoltcp owns
//! IPv4/DHCP/TCP, and this module exposes a single adbd-compatible TCP listener
//! on port 5555 when WADB is enabled from SystemUI.

extern crate alloc;

use alloc::vec;
use esp_hal::time::Instant as HalInstant;
use esp_println::println;
use esp_radio::wifi::WifiDevice;
use heapless::Vec as HVec;
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet},
    socket::{dhcpv4, tcp},
    time::Instant,
    wire::{EthernetAddress, IpCidr, Ipv4Address},
};
use static_cell::StaticCell;

use crate::adb::{self, Header, A_CLSE, A_CNXN, A_OKAY, A_OPEN, A_WRTE};

const ADB_MAX_PAYLOAD: usize = 768;
const ADB_VERSION: u32 = 0x0100_0001;
const ADB_MAX_DATA: u32 = 4096;
const WADB_PORT: u16 = 5555;

static WADB_RX_BUF: StaticCell<[u8; 4096]> = StaticCell::new();
static WADB_TX_BUF: StaticCell<[u8; 4096]> = StaticCell::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkState {
    Down,
    Associating,
    LinkUp,
    Dhcp,
    Online,
}

struct AdbMessage {
    header: Header,
    payload: HVec<u8, ADB_MAX_PAYLOAD>,
}

struct AdbRx {
    header_raw: [u8; 24],
    header_len: usize,
    header: Option<Header>,
    payload: [u8; ADB_MAX_PAYLOAD],
    payload_len: usize,
}

impl AdbRx {
    const fn new() -> Self {
        Self {
            header_raw: [0; 24],
            header_len: 0,
            header: None,
            payload: [0; ADB_MAX_PAYLOAD],
            payload_len: 0,
        }
    }

    fn reset(&mut self) {
        self.header_len = 0;
        self.header = None;
        self.payload_len = 0;
    }

    fn push(&mut self, byte: u8) -> Option<AdbMessage> {
        if self.header.is_none() {
            self.header_raw[self.header_len] = byte;
            self.header_len += 1;
            if self.header_len != self.header_raw.len() {
                return None;
            }
            let Some(header) = Header::decode(&self.header_raw) else {
                self.reset();
                return None;
            };
            if header.data_length as usize > ADB_MAX_PAYLOAD {
                self.reset();
                return None;
            }
            if header.data_length == 0 {
                self.reset();
                return Some(AdbMessage { header, payload: HVec::new() });
            }
            self.header = Some(header);
            return None;
        }

        let header = self.header?;
        self.payload[self.payload_len] = byte;
        self.payload_len += 1;
        if self.payload_len != header.data_length as usize {
            return None;
        }

        let mut payload = HVec::new();
        let _ = payload.extend_from_slice(&self.payload[..self.payload_len]);
        let valid = adb::checksum(payload.as_slice()) == header.data_check;
        self.reset();
        if valid { Some(AdbMessage { header, payload }) } else { None }
    }
}

pub struct NetworkService {
    iface: Interface,
    sockets: SocketSet<'static>,
    dhcp_handle: SocketHandle,
    adb_handle: SocketHandle,
    adb_rx: AdbRx,
    ip: Option<Ipv4Address>,
    link: LinkState,
    wadb_listening: bool,
    wadb_seen_client: bool,
}

impl NetworkService {
    pub fn new(device: &mut WifiDevice<'static>) -> Self {
        let mac = device.mac_address();
        let mut cfg = Config::new(EthernetAddress(mac).into());
        cfg.random_seed = u64::from_le_bytes([mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], 0x5a, 0x33]);
        let iface = Interface::new(cfg, device, Instant::ZERO);

        let mut sockets = SocketSet::new(vec![]);
        let dhcp_handle = sockets.add(dhcpv4::Socket::new());
        let rx = &mut WADB_RX_BUF.init([0; 4096])[..];
        let tx = &mut WADB_TX_BUF.init([0; 4096])[..];
        let adb_handle = sockets.add(tcp::Socket::new(
            tcp::SocketBuffer::new(rx),
            tcp::SocketBuffer::new(tx),
        ));

        Self {
            iface,
            sockets,
            dhcp_handle,
            adb_handle,
            adb_rx: AdbRx::new(),
            ip: None,
            link: LinkState::Down,
            wadb_listening: false,
            wadb_seen_client: false,
        }
    }

    pub fn set_link_state(&mut self, state: LinkState) {
        self.link = state;
        if matches!(state, LinkState::Down | LinkState::Associating | LinkState::LinkUp) {
            self.clear_ip();
        }
    }

    pub fn link_state(&self) -> LinkState { self.link }
    pub fn ip(&self) -> Option<Ipv4Address> { self.ip }
    pub fn wadb_active(&self) -> bool { self.wadb_listening }
    pub fn wadb_seen_client(&self) -> bool { self.wadb_seen_client }

    pub fn reset(&mut self) {
        self.link = LinkState::Down;
        self.clear_ip();
        self.adb_rx.reset();
        let socket = self.sockets.get_mut::<tcp::Socket>(self.adb_handle);
        if socket.is_open() { socket.abort(); }
        self.wadb_listening = false;
    }

    pub fn poll(&mut self, device: &mut WifiDevice<'static>, wadb_enabled: bool) {
        // Do not let DHCP emit packets while the 802.11 station is stopped or
        // still associating.  smoltcp's Device trait has no explicit link-state
        // callback, so ConnectivityService gates the network stack here.
        if matches!(self.link, LinkState::Dhcp | LinkState::Online) {
            let now = smol_now();
            let _ = self.iface.poll(now, device, &mut self.sockets);
            self.poll_dhcp();
            self.poll_wadb(wadb_enabled);
        } else {
            self.poll_wadb(false);
        }
    }

    fn poll_dhcp(&mut self) {
        let event = self.sockets.get_mut::<dhcpv4::Socket>(self.dhcp_handle).poll();
        match event {
            None => {}
            Some(dhcpv4::Event::Configured(config)) => {
                let address = config.address.address();
                self.iface.update_ip_addrs(|addrs| {
                    addrs.clear();
                    let _ = addrs.push(IpCidr::Ipv4(config.address));
                });
                if let Some(router) = config.router {
                    let _ = self.iface.routes_mut().add_default_ipv4_route(router);
                } else {
                    self.iface.routes_mut().remove_default_ipv4_route();
                }
                if self.ip != Some(address) {
                    println!("net: DHCP address {}", address);
                }
                self.ip = Some(address);
                self.link = LinkState::Online;
            }
            Some(dhcpv4::Event::Deconfigured) => {
                if self.ip.take().is_some() {
                    println!("net: DHCP lease lost");
                }
                self.iface.update_ip_addrs(|addrs| addrs.clear());
                self.iface.routes_mut().remove_default_ipv4_route();
                if self.link != LinkState::Down {
                    self.link = LinkState::Dhcp;
                }
            }
        }
    }

    fn clear_ip(&mut self) {
        self.ip = None;
        self.iface.update_ip_addrs(|addrs| addrs.clear());
        self.iface.routes_mut().remove_default_ipv4_route();
        self.sockets.get_mut::<dhcpv4::Socket>(self.dhcp_handle).reset();
    }

    fn poll_wadb(&mut self, enabled: bool) {
        let online = self.ip.is_some();
        let socket = self.sockets.get_mut::<tcp::Socket>(self.adb_handle);

        if !enabled || !online {
            if socket.is_open() { socket.abort(); }
            self.wadb_listening = false;
            self.adb_rx.reset();
            return;
        }

        if !socket.is_open() {
            match socket.listen(WADB_PORT) {
                Ok(()) => {
                    self.wadb_listening = true;
                    println!("adbd: wireless transport listening on tcp:{}", WADB_PORT);
                }
                Err(e) => {
                    self.wadb_listening = false;
                    println!("adbd: listen failed: {:?}", e);
                    return;
                }
            }
        }

        if socket.is_active() {
            self.wadb_seen_client = true;
        }

        let mut incoming = [0u8; 512];
        while socket.can_recv() {
            let Ok(n) = socket.recv_slice(&mut incoming) else { break; };
            if n == 0 { break; }
            for &byte in &incoming[..n] {
                if let Some(message) = self.adb_rx.push(byte) {
                    handle_adb(socket, message);
                }
            }
        }
    }
}

fn smol_now() -> Instant {
    Instant::from_millis(HalInstant::now().duration_since_epoch().as_millis() as i64)
}

fn send_packet(socket: &mut tcp::Socket<'_>, command: u32, arg0: u32, arg1: u32, payload: &[u8]) {
    let header = Header {
        command,
        arg0,
        arg1,
        data_length: payload.len() as u32,
        data_check: adb::checksum(payload),
        magic: !command,
    };
    let raw = header.encode();
    let _ = socket.send_slice(&raw);
    if !payload.is_empty() {
        let _ = socket.send_slice(payload);
    }
}

fn handle_adb(socket: &mut tcp::Socket<'_>, message: AdbMessage) {
    match message.header.command {
        A_CNXN => {
            // Deliberately omit shell_v2/auth features: this development adbd
            // implements the legacy one-shot shell stream and accepts local-LAN
            // connections without RSA while bring-up is in progress.
            let banner = b"device::ro.product.name=Zephyr Watch;ro.product.model=Zephyr Watch;ro.product.device=zero;\0";
            send_packet(socket, A_CNXN, ADB_VERSION, ADB_MAX_DATA, banner);
        }
        A_OPEN => {
            let host_id = message.header.arg0;
            let device_id = 1u32;
            send_packet(socket, A_OKAY, device_id, host_id, &[]);
            let payload = message.payload.as_slice();
            let end = payload.iter().position(|&b| b == 0).unwrap_or(payload.len());
            let service = core::str::from_utf8(&payload[..end]).unwrap_or("");
            let mut out = heapless::String::<640>::new();
            adb::shell_command(service, &mut out);
            if !out.is_empty() {
                send_packet(socket, A_WRTE, device_id, host_id, out.as_bytes());
            }
            send_packet(socket, A_CLSE, device_id, host_id, &[]);
        }
        A_WRTE => {
            // Legacy one-shot shell services do not accept stdin yet, but WRTE
            // must still be acknowledged to keep the host state machine sane.
            send_packet(socket, A_OKAY, message.header.arg1, message.header.arg0, &[]);
        }
        A_OKAY | A_CLSE => {}
        _ => {}
    }
}
