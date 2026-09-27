//! Minimal IPv4/DHCP + wireless ADB transport for AOSP Wear OS.
//!
//! esp-radio owns the 802.11 station, smoltcp owns IPv4/DHCP/TCP and this
//! module exposes an adbd-compatible TCP listener on port 5555.  v0.6 keeps a
//! real ADB stream state so one-shot commands terminate correctly and
//! `adb shell` can stay interactive instead of hanging forever.

extern crate alloc;

use alloc::vec;
use esp_hal::time::Instant as HalInstant;
use esp_println::println;
use esp_radio::wifi::WifiDevice;
use heapless::{String as HString, Vec as HVec};
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet},
    socket::{dhcpv4, tcp},
    time::{Duration as SmolDuration, Instant},
    wire::{DhcpOption, EthernetAddress, IpCidr, Ipv4Address},
};
use static_cell::StaticCell;

use crate::adb::{self, Header, A_CLSE, A_CNXN, A_OKAY, A_OPEN, A_WRTE};

const ADB_MAX_PAYLOAD: usize = 1024;
const ADB_VERSION: u32 = 0x0100_0001;
const ADB_MAX_DATA: u32 = ADB_MAX_PAYLOAD as u32;
const WADB_PORT: u16 = 5555;
const ADB_CLOSE_GRACE_MS: u64 = 1_200;
static DHCP_HOSTNAME: &[u8] = b"android-aosp-wear";
static DHCP_OPTIONS: [DhcpOption<'static>; 1] = [DhcpOption { kind: 12, data: DHCP_HOSTNAME }];

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
                println!(
                    "adbd: dropping oversized packet command={:08x} len={}",
                    header.command, header.data_length
                );
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
        // ADB protocol 0x01000001 (advertised by this device) deliberately
        // skips payload checksums. Modern adb therefore sends data_check=0.
        // Keep accepting the legacy checksum too for older hosts.
        let valid = header.data_check == 0 || adb::checksum(payload.as_slice()) == header.data_check;
        self.reset();
        if valid {
            Some(AdbMessage { header, payload })
        } else {
            println!("adbd: packet checksum mismatch");
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdbStreamMode {
    Closed,
    OneShot,
    Interactive,
}

struct AdbStream {
    host_id: u32,
    device_id: u32,
    mode: AdbStreamMode,
    waiting_for_okay: bool,
    close_after_okay: bool,
    close_deadline_ms: u64,
    input: HString<256>,
}

impl AdbStream {
    const fn new() -> Self {
        Self {
            host_id: 0,
            device_id: 0,
            mode: AdbStreamMode::Closed,
            waiting_for_okay: false,
            close_after_okay: false,
            close_deadline_ms: 0,
            input: HString::new(),
        }
    }

    fn clear(&mut self) {
        self.host_id = 0;
        self.device_id = 0;
        self.mode = AdbStreamMode::Closed;
        self.waiting_for_okay = false;
        self.close_after_okay = false;
        self.close_deadline_ms = 0;
        self.input.clear();
    }

    fn open(&mut self, host_id: u32, device_id: u32, mode: AdbStreamMode) {
        self.clear();
        self.host_id = host_id;
        self.device_id = device_id;
        self.mode = mode;
    }

    fn matches(&self, host_id: u32, device_id: u32) -> bool {
        self.mode != AdbStreamMode::Closed
            && self.host_id == host_id
            && self.device_id == device_id
    }
}

pub struct NetworkService {
    iface: Interface,
    sockets: SocketSet<'static>,
    dhcp_handle: SocketHandle,
    adb_handle: SocketHandle,
    adb_rx: AdbRx,
    adb_stream: AdbStream,
    next_adb_local_id: u32,
    ip: Option<Ipv4Address>,
    link: LinkState,
    wadb_listening: bool,
    wadb_seen_client: bool,
}

impl NetworkService {
    pub fn new(device: &mut WifiDevice<'static>) -> Self {
        let mac = device.mac_address();
        let mut cfg = Config::new(EthernetAddress(mac).into());
        cfg.random_seed = u64::from_le_bytes([
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], 0x5a, 0x33,
        ]);
        let iface = Interface::new(cfg, device, Instant::ZERO);

        let mut sockets = SocketSet::new(vec![]);
        let mut dhcp = dhcpv4::Socket::new();
        // DHCP option 12 makes Android/MIUI hotspot client lists show a real
        // hostname instead of falling back to the station MAC address.
        dhcp.set_outgoing_options(&DHCP_OPTIONS);
        let dhcp_handle = sockets.add(dhcp);
        let rx = &mut WADB_RX_BUF.init([0; 4096])[..];
        let tx = &mut WADB_TX_BUF.init([0; 4096])[..];
        let mut adb_socket = tcp::Socket::new(
            tcp::SocketBuffer::new(rx),
            tcp::SocketBuffer::new(tx),
        );
        // ADB sends many tiny 24-byte control packets. Nagle adds avoidable
        // latency to OKAY/WRTE/CLSE handshakes on a local Wi-Fi network.
        adb_socket.set_nagle_enabled(false);
        adb_socket.set_keep_alive(Some(SmolDuration::from_secs(15)));
        adb_socket.set_timeout(Some(SmolDuration::from_secs(90)));
        let adb_handle = sockets.add(adb_socket);

        Self {
            iface,
            sockets,
            dhcp_handle,
            adb_handle,
            adb_rx: AdbRx::new(),
            adb_stream: AdbStream::new(),
            next_adb_local_id: 1,
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
        self.adb_stream.clear();
        let socket = self.sockets.get_mut::<tcp::Socket>(self.adb_handle);
        if socket.is_open() { socket.abort(); }
        self.wadb_listening = false;
        self.wadb_seen_client = false;
    }

    pub fn poll(&mut self, device: &mut WifiDevice<'static>, wadb_enabled: bool) {
        // Do not let DHCP emit packets while the station is stopped or still
        // associating. smoltcp has no Wi-Fi association callback itself.
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
            self.adb_stream.clear();
            return;
        }

        if !socket.is_open() {
            match socket.listen(WADB_PORT) {
                Ok(()) => {
                    self.wadb_listening = true;
                    self.wadb_seen_client = false;
                    self.adb_stream.clear();
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
                    handle_adb(
                        socket,
                        message,
                        &mut self.adb_stream,
                        &mut self.next_adb_local_id,
                    );
                }
            }
        }

        // A well-behaved adb host ACKs WRTE before we close a one-shot shell.
        // If an older/newer host fails to do that, don't leave the shell stream
        // hung forever: close it after a short grace period.
        if self.adb_stream.mode == AdbStreamMode::OneShot
            && self.adb_stream.close_after_okay
            && self.adb_stream.close_deadline_ms != 0
            && now_ms() >= self.adb_stream.close_deadline_ms
        {
            println!("adbd: shell ACK timeout; closing stream");
            let _ = send_packet(
                socket,
                A_CLSE,
                self.adb_stream.device_id,
                self.adb_stream.host_id,
                &[],
            );
            self.adb_stream.clear();
        }
    }
}

fn smol_now() -> Instant {
    Instant::from_millis(HalInstant::now().duration_since_epoch().as_millis() as i64)
}

fn now_ms() -> u64 {
    HalInstant::now().duration_since_epoch().as_millis()
}

/// Queue one complete ADB packet or none of it.  Ignoring partial `send_slice`
/// results can corrupt the 24-byte framing and was one cause of shell sessions
/// that connected successfully but then never completed.
fn send_packet(
    socket: &mut tcp::Socket<'_>,
    command: u32,
    arg0: u32,
    arg1: u32,
    payload: &[u8],
) -> bool {
    let needed = 24usize.saturating_add(payload.len());
    let free = socket.send_capacity().saturating_sub(socket.send_queue());
    if free < needed {
        println!("adbd: tx queue full (need={} free={})", needed, free);
        return false;
    }

    let header = Header {
        command,
        arg0,
        arg1,
        data_length: payload.len() as u32,
        data_check: adb::checksum(payload),
        magic: !command,
    };
    let raw = header.encode();
    if !matches!(socket.send_slice(&raw), Ok(n) if n == raw.len()) {
        println!("adbd: failed to queue header");
        return false;
    }
    if !payload.is_empty()
        && !matches!(socket.send_slice(payload), Ok(n) if n == payload.len())
    {
        // This should not happen after the capacity check; if it ever does the
        // transport is already unusable, so make it visible in the log.
        println!("adbd: failed to queue complete payload");
        return false;
    }
    true
}

fn next_local_id(next: &mut u32) -> u32 {
    let id = (*next).max(1);
    *next = next.wrapping_add(1).max(1);
    id
}

fn handle_adb(
    socket: &mut tcp::Socket<'_>,
    message: AdbMessage,
    stream: &mut AdbStream,
    next_id: &mut u32,
) {
    match message.header.command {
        A_CNXN => {
            let banner = b"device::ro.product.name=aosp_wear;ro.product.model=AOSP Wear OS;ro.product.device=aosp_wear;ro.product.manufacturer=AOSP;features=;\0";
            let _ = send_packet(socket, A_CNXN, ADB_VERSION, ADB_MAX_DATA, banner);
        }
        A_OPEN => {
            let host_id = message.header.arg0;
            let payload = message.payload.as_slice();
            let end = payload.iter().position(|&b| b == 0).unwrap_or(payload.len());
            let service = core::str::from_utf8(&payload[..end]).unwrap_or("");

            if !(service.starts_with("shell:") || service == "shell") {
                println!("adbd: unsupported service {}", service);
                let _ = send_packet(socket, A_CLSE, 0, host_id, &[]);
                return;
            }

            if stream.mode != AdbStreamMode::Closed {
                let _ = send_packet(socket, A_CLSE, stream.device_id, stream.host_id, &[]);
                stream.clear();
            }

            let device_id = next_local_id(next_id);
            let command = service.strip_prefix("shell:").unwrap_or("").trim();
            let mode = if command.is_empty() {
                AdbStreamMode::Interactive
            } else {
                AdbStreamMode::OneShot
            };
            stream.open(host_id, device_id, mode);

            if !send_packet(socket, A_OKAY, device_id, host_id, &[]) {
                stream.clear();
                return;
            }

            if mode == AdbStreamMode::Interactive {
                println!("adbd: interactive shell opened");
                let greeting = b"AOSP Wear OS - Android 17 QPR1 userdebug\r\naosp-wear:/ $ ";
                if send_packet(socket, A_WRTE, device_id, host_id, greeting) {
                    stream.waiting_for_okay = true;
                }
                return;
            }

            println!("adbd: shell {}", command);
            let mut out = HString::<1536>::new();
            adb::shell_command(command, &mut out);
            if out.is_empty() {
                let _ = send_packet(socket, A_CLSE, device_id, host_id, &[]);
                stream.clear();
            } else if send_packet(socket, A_WRTE, device_id, host_id, out.as_bytes()) {
                stream.waiting_for_okay = true;
                stream.close_after_okay = true;
                stream.close_deadline_ms = now_ms() + ADB_CLOSE_GRACE_MS;
            } else {
                let _ = send_packet(socket, A_CLSE, device_id, host_id, &[]);
                stream.clear();
            }
        }
        A_WRTE => {
            let host_id = message.header.arg0;
            let device_id = message.header.arg1;
            if !stream.matches(host_id, device_id) {
                let _ = send_packet(socket, A_CLSE, 0, host_id, &[]);
                return;
            }

            // ACK host input first. The ADB stream protocol requires one OKAY
            // for every WRTE before that sender may write again.
            let _ = send_packet(socket, A_OKAY, device_id, host_id, &[]);

            if stream.mode != AdbStreamMode::Interactive {
                return;
            }

            let mut execute = false;
            let mut logout = false;
            for &b in message.payload.iter() {
                match b {
                    b'\r' => {}
                    b'\n' => execute = true,
                    0x04 => { execute = true; logout = true; }
                    0x08 | 0x7f => { let _ = stream.input.pop(); }
                    0x20..=0x7e => { let _ = stream.input.push(b as char); }
                    _ => {}
                }
            }
            if !execute { return; }

            let command = stream.input.clone();
            stream.input.clear();
            let cmd = command.trim();
            if cmd == "exit" { logout = true; }

            let mut out = HString::<1536>::new();
            if !logout {
                adb::shell_command(cmd, &mut out);
                let _ = out.push_str("aosp-wear:/ $ ");
            } else {
                let _ = out.push_str("logout\r\n");
            }

            if send_packet(socket, A_WRTE, device_id, host_id, out.as_bytes()) {
                stream.waiting_for_okay = true;
                if logout {
                    stream.close_after_okay = true;
                    stream.close_deadline_ms = now_ms() + ADB_CLOSE_GRACE_MS;
                    stream.mode = AdbStreamMode::OneShot;
                }
            }
        }
        A_OKAY => {
            let host_id = message.header.arg0;
            let device_id = message.header.arg1;
            if stream.matches(host_id, device_id) {
                stream.waiting_for_okay = false;
                if stream.close_after_okay {
                    let _ = send_packet(socket, A_CLSE, device_id, host_id, &[]);
                    stream.clear();
                }
            }
        }
        A_CLSE => {
            // protocol.txt: the recipient MUST NOT reply to CLOSE.  Older code
            // echoed CLSE back and could leave modern adb clients waiting on a
            // stream that should already have been gone.
            let host_id = message.header.arg0;
            let device_id = message.header.arg1;
            if stream.matches(host_id, device_id) || device_id == 0 {
                stream.clear();
            }
        }
        _ => {}
    }
}
