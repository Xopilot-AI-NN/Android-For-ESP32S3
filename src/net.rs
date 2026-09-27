//! Minimal IPv4/DHCP + wireless ADB transport for AOSP Wear OS.
//!
//! esp-radio owns the 802.11 station, smoltcp owns IPv4/DHCP/TCP and this
//! module exposes an adbd-compatible TCP listener on port 5555.  v0.6 keeps a
//! real ADB stream state so one-shot commands terminate correctly and
//! `adb shell` can stay interactive instead of hanging forever.

extern crate alloc;

use core::fmt::Write;
use alloc::vec;
use esp_hal::time::Instant as HalInstant;
use esp_println::println;
use esp_radio::wifi::WifiDevice;
use heapless::{String as HString, Vec as HVec};
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet},
    socket::{dhcpv4, tcp, udp},
    time::{Duration as SmolDuration, Instant},
    wire::{DhcpOption, EthernetAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address},
};
use static_cell::StaticCell;

use crate::{adb::{self, Header, A_CLSE, A_CNXN, A_OKAY, A_OPEN, A_WRTE}, material::MaterialFrame, runtime::RuntimeFrame};

const ADB_MAX_PAYLOAD: usize = 1024;
const ADB_VERSION: u32 = 0x0100_0001;
const ADB_MAX_DATA: u32 = ADB_MAX_PAYLOAD as u32;
const WADB_PORT: u16 = 5555;
const ADB_CLOSE_GRACE_MS: u64 = 1_200;
static DHCP_HOSTNAME: &[u8] = b"android-aosp-wear";
static DHCP_OPTIONS: [DhcpOption<'static>; 1] = [DhcpOption { kind: 12, data: DHCP_HOSTNAME }];

static WADB_RX_BUF: StaticCell<[u8; 4096]> = StaticCell::new();
static WADB_TX_BUF: StaticCell<[u8; 4096]> = StaticCell::new();
static NTP_RX_META: StaticCell<[udp::PacketMetadata; 2]> = StaticCell::new();
static NTP_TX_META: StaticCell<[udp::PacketMetadata; 2]> = StaticCell::new();
static NTP_RX_BUF: StaticCell<[u8; 128]> = StaticCell::new();
static NTP_TX_BUF: StaticCell<[u8; 128]> = StaticCell::new();
static WEB_RX_BUF: StaticCell<[u8; 1536]> = StaticCell::new();
static WEB_TX_BUF: StaticCell<[u8; 8192]> = StaticCell::new();

const WEB_PORT: u16 = 80;
const WEB_HTML: &str = r#"<!doctype html><html><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1,viewport-fit=cover'><title>AOSP Wear Remote</title><style>*{box-sizing:border-box}body{margin:0;background:#0b0d0c;color:#eaf4ec;font:15px system-ui,Roboto,sans-serif;display:grid;place-items:center;min-height:100vh}.wrap{width:min(94vw,430px);padding:18px}.head{display:flex;justify-content:space-between;align-items:center;margin:0 4px 12px;color:#9eaaa1}.watch{height:390px;background:#000;border-radius:54px;border:1px solid #29312c;box-shadow:0 20px 60px #0008,inset 0 0 30px #0b2418;padding:22px;display:flex;flex-direction:column;align-items:center;justify-content:center;overflow:hidden}.clock{font-size:54px;font-weight:650;letter-spacing:-2px}.title{font-size:22px;font-weight:650;margin:8px 0}.meta{color:#9ea8a0;text-align:center;min-height:38px}.chips{display:flex;gap:8px;margin-top:14px}.chip{padding:7px 11px;border-radius:20px;background:#16231b;color:#9eaaa1}.on{background:#143c28;color:#75f2a5}.grid{display:grid;grid-template-columns:repeat(3,62px);gap:13px;margin-top:8px}.app{width:62px;height:62px;border-radius:50%;display:grid;place-items:center;font-weight:800;background:#173a2a;border:2px solid #2a5540}.app.sel{transform:scale(1.12);border-color:#75f2a5}.controls{margin-top:14px;display:grid;grid-template-columns:repeat(5,1fr);gap:8px}.controls button,.sw button{border:0;border-radius:18px;background:#1d2520;color:#eaf4ec;padding:14px 5px;font-weight:700;font-size:14px}.controls button:active,.sw button:active{background:#2d563e}.sw{display:grid;grid-template-columns:repeat(4,1fr);gap:8px;margin-top:8px}.url{text-align:center;color:#758078;font-size:12px;margin-top:10px}</style></head><body><div class=wrap><div class=head><b>AOSP Wear OS</b><span id=net>offline</span></div><div class=watch id=watch><div class=clock id=clock>00:00</div><div class=title id=title>WATCH FACE</div><div class=meta id=meta>Waiting for state...</div><div class=chips><span class=chip id=wifi>Wi-Fi</span><span class=chip id=bt>BT</span><span class=chip id=adb>ADB</span></div><div class=grid id=grid hidden></div></div><div class=controls><button onclick=cmd('back')>BACK</button><button onclick=cmd('rotl')>-</button><button onclick=cmd('press')>OK</button><button onclick=cmd('rotr')>+</button><button onclick=cmd('home')>HOME</button></div><div class=sw><button onclick=cmd('up')>SWIPE UP</button><button onclick=cmd('down')>DOWN</button><button onclick=cmd('left')>LEFT</button><button onclick=cmd('right')>RIGHT</button></div><div class=url>Local browser remote - no cloud</div></div><script>const names=['LOCK','WATCH FACE','APPS','NOTIFICATIONS','QUICK SETTINGS','SETTINGS','CLOCK','CONNECTIVITY','ABOUT','TILES','MEDIA','CLOCK TOOLS','WI-FI','BLUETOOTH','DISPLAY','SYSTEM','DATE & TIME','SOUND','GESTURES','ACCESSIBILITY','SECURITY','APPS & NOTIFICATIONS','DEVELOPER OPTIONS','NETWORKS','WI-FI PASSWORD','WI-FI CONNECT','APPS LIST','ASSISTANT'];const apps=['CLK','MED','SET','NET','INFO','AI'];async function cmd(e){try{await fetch('/event?e='+e,{cache:'no-store'})}catch(_){}}function hhmm(m){return String(Math.floor(m/60)).padStart(2,'0')+':'+String(m%60).padStart(2,'0')}function render(s){clock.textContent=hhmm(s.time||0);title.textContent=names[s.screen]||'AOSP WEAR';net.textContent=s.ip||'online';wifi.className='chip '+(s.wifi?'on':'');bt.className='chip '+(s.bt?'on':'');adb.className='chip '+(s.adb?'on':'');let x=[];if(s.screen==1)x.push(s.valid?'SOFTWARE RTC':'SOFTWARE RTC - SYNC PENDING');if(s.screen==12||s.screen==23||s.screen==24||s.screen==25)x.push((s.ssid||'Wi-Fi')+'  '+(s.rssi>-120?s.rssi+' dBm':''));if(s.screen!=1&&s.screen!=2)x.push('cursor '+s.cursor);meta.textContent=x.join(' | ')||'Encoder remote';grid.hidden=s.screen!=2;if(s.screen==2){grid.innerHTML=apps.map((a,i)=>'<div class="app '+(i==s.cursor?'sel':'')+'">'+a+'</div>').join('')}}async function tick(){try{let r=await fetch('/state?'+Date.now(),{cache:'no-store'});render(await r.json())}catch(e){net.textContent='reconnecting'}setTimeout(tick,650)}addEventListener('keydown',e=>{if(e.key=='ArrowLeft')cmd('rotl');else if(e.key=='ArrowRight')cmd('rotr');else if(e.key=='Enter')cmd('press');else if(e.key=='Escape')cmd('back')});tick()</script></body></html>"#;

const NTP_PORT: u16 = 123;
const NTP_LOCAL_PORT: u16 = 49152;
const NTP_UNIX_EPOCH_DELTA: u32 = 2_208_988_800;
const NTP_RETRY_MS: u64 = 15_000;
const NTP_RESYNC_MS: u64 = 6 * 60 * 60 * 1000;
// Google Public NTP time1..time4.google.com anycast IPv4 endpoints.
const NTP_SERVERS: [Ipv4Address; 4] = [
    Ipv4Address::new(216, 239, 35, 0),
    Ipv4Address::new(216, 239, 35, 4),
    Ipv4Address::new(216, 239, 35, 8),
    Ipv4Address::new(216, 239, 35, 12),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkState {
    Down,
    Associating,
    LinkUp,
    Dhcp,
    Online,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebRemoteEvent {
    Rotate(i8),
    Press,
    Back,
    Home,
    Swipe(i8),
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
    ntp_handle: SocketHandle,
    web_handle: SocketHandle,
    adb_rx: AdbRx,
    adb_stream: AdbStream,
    next_adb_local_id: u32,
    ip: Option<Ipv4Address>,
    link: LinkState,
    wadb_listening: bool,
    wadb_seen_client: bool,
    ntp_next_request_ms: u64,
    ntp_request_started_ms: u64,
    ntp_server_index: u8,
    ntp_pending: bool,
    ntp_unix_seconds: Option<u32>,
    web_event: Option<WebRemoteEvent>,
    web_frame: Option<MaterialFrame>,
    web_request: HVec<u8, 1024>,
    web_listening: bool,
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

        let ntp_rx_meta = &mut NTP_RX_META.init([udp::PacketMetadata::EMPTY; 2])[..];
        let ntp_tx_meta = &mut NTP_TX_META.init([udp::PacketMetadata::EMPTY; 2])[..];
        let ntp_rx_data = &mut NTP_RX_BUF.init([0; 128])[..];
        let ntp_tx_data = &mut NTP_TX_BUF.init([0; 128])[..];
        let ntp_socket = udp::Socket::new(
            udp::PacketBuffer::new(ntp_rx_meta, ntp_rx_data),
            udp::PacketBuffer::new(ntp_tx_meta, ntp_tx_data),
        );
        let ntp_handle = sockets.add(ntp_socket);

        let web_rx = &mut WEB_RX_BUF.init([0; 1536])[..];
        let web_tx = &mut WEB_TX_BUF.init([0; 8192])[..];
        let mut web_socket = tcp::Socket::new(
            tcp::SocketBuffer::new(web_rx),
            tcp::SocketBuffer::new(web_tx),
        );
        web_socket.set_nagle_enabled(false);
        web_socket.set_timeout(Some(SmolDuration::from_secs(20)));
        let web_handle = sockets.add(web_socket);

        Self {
            iface,
            sockets,
            dhcp_handle,
            adb_handle,
            ntp_handle,
            web_handle,
            adb_rx: AdbRx::new(),
            adb_stream: AdbStream::new(),
            next_adb_local_id: 1,
            ip: None,
            link: LinkState::Down,
            wadb_listening: false,
            wadb_seen_client: false,
            ntp_next_request_ms: 0,
            ntp_request_started_ms: 0,
            ntp_server_index: 0,
            ntp_pending: false,
            ntp_unix_seconds: None,
            web_event: None,
            web_frame: None,
            web_request: HVec::new(),
            web_listening: false,
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
    pub fn take_network_time(&mut self) -> Option<u32> { self.ntp_unix_seconds.take() }
    pub fn request_time_sync(&mut self) { self.ntp_pending = false; self.ntp_next_request_ms = 0; }
    pub fn take_web_event(&mut self) -> Option<WebRemoteEvent> { self.web_event.take() }
    pub fn set_web_frame(&mut self, frame: &RuntimeFrame) {
        if let RuntimeFrame::Material(m) = frame { self.web_frame = Some(*m); }
    }

    pub fn reset(&mut self) {
        self.link = LinkState::Down;
        self.clear_ip();
        self.adb_rx.reset();
        self.adb_stream.clear();
        let socket = self.sockets.get_mut::<tcp::Socket>(self.adb_handle);
        if socket.is_open() { socket.abort(); }
        self.wadb_listening = false;
        self.wadb_seen_client = false;
        self.ntp_pending = false;
        self.ntp_next_request_ms = 0;
        self.ntp_request_started_ms = 0;
        let ntp = self.sockets.get_mut::<udp::Socket>(self.ntp_handle);
        if ntp.is_open() { ntp.close(); }
        let web = self.sockets.get_mut::<tcp::Socket>(self.web_handle);
        if web.is_open() { web.abort(); }
        self.web_request.clear();
        self.web_event = None;
        self.web_listening = false;
    }

    pub fn poll(&mut self, device: &mut WifiDevice<'static>, wadb_enabled: bool) {
        // Do not let DHCP emit packets while the station is stopped or still
        // associating. smoltcp has no Wi-Fi association callback itself.
        if matches!(self.link, LinkState::Dhcp | LinkState::Online) {
            let now = smol_now();
            let _ = self.iface.poll(now, device, &mut self.sockets);
            self.poll_dhcp();
            self.poll_wadb(wadb_enabled);
            self.poll_ntp();
            self.poll_web(true);
        } else {
            self.poll_wadb(false);
            self.poll_web(false);
            self.ntp_pending = false;
        }
    }

    fn poll_web(&mut self, enabled: bool) {
        if !enabled || self.ip.is_none() {
            let socket = self.sockets.get_mut::<tcp::Socket>(self.web_handle);
            if socket.is_open() { socket.abort(); }
            self.web_request.clear();
            self.web_listening = false;
            return;
        }

        let need_listen = {
            let socket = self.sockets.get_mut::<tcp::Socket>(self.web_handle);
            !socket.is_open()
        };
        if need_listen {
            let result = self.sockets.get_mut::<tcp::Socket>(self.web_handle).listen(WEB_PORT);
            match result {
                Ok(()) => {
                    self.web_listening = true;
                    self.web_request.clear();
                    if let Some(ip) = self.ip { println!("web: remote UI listening on http://{}/", ip); }
                }
                Err(e) => {
                    self.web_listening = false;
                    println!("web: listen failed: {:?}", e);
                    return;
                }
            }
        }

        let mut incoming = [0u8; 768];
        let n = {
            let socket = self.sockets.get_mut::<tcp::Socket>(self.web_handle);
            if socket.can_recv() { socket.recv_slice(&mut incoming).unwrap_or(0) } else { 0 }
        };
        if n != 0 {
            let room = 1024usize.saturating_sub(self.web_request.len());
            let take = n.min(room);
            let _ = self.web_request.extend_from_slice(&incoming[..take]);
        }
        if !self.web_request.as_slice().windows(4).any(|w| w == b"\r\n\r\n") { return; }

        let mut first_line = HString::<192>::new();
        if let Ok(request) = core::str::from_utf8(self.web_request.as_slice()) {
            if let Some(line) = request.lines().next() { let _ = first_line.push_str(line); }
        }
        let first = first_line.as_str();
        let mut status = "200 OK";
        let mut content_type = "application/json; charset=utf-8";
        let mut state_body = HString::<768>::new();
        let body: &str;

        if first.starts_with("GET /state") {
            self.write_web_state(&mut state_body);
            body = state_body.as_str();
        } else if first.starts_with("GET /event?e=") {
            let value = first.strip_prefix("GET /event?e=").and_then(|v| v.split_whitespace().next()).unwrap_or("");
            self.web_event = match value {
                "rotl" => Some(WebRemoteEvent::Rotate(-1)),
                "rotr" => Some(WebRemoteEvent::Rotate(1)),
                "press" => Some(WebRemoteEvent::Press),
                "back" => Some(WebRemoteEvent::Back),
                "home" => Some(WebRemoteEvent::Home),
                "up" => Some(WebRemoteEvent::Swipe(-1)),
                "down" => Some(WebRemoteEvent::Swipe(1)),
                "left" => Some(WebRemoteEvent::Swipe(-2)),
                "right" => Some(WebRemoteEvent::Swipe(2)),
                _ => None,
            };
            let _ = state_body.push_str("{\"ok\":1}");
            body = state_body.as_str();
        } else if first.starts_with("GET / ") || first.starts_with("GET /HTTP") {
            content_type = "text/html; charset=utf-8";
            body = WEB_HTML;
        } else {
            status = "404 Not Found";
            let _ = state_body.push_str("{\"error\":404}");
            body = state_body.as_str();
        }

        let socket = self.sockets.get_mut::<tcp::Socket>(self.web_handle);
        let _ = send_http_response(socket, status, content_type, body.as_bytes());
        socket.close();
        self.web_request.clear();
    }

    fn write_web_state(&self, out: &mut HString<768>) {
        let Some(f) = self.web_frame else {
            let _ = out.push_str("{\"screen\":1,\"cursor\":0,\"time\":0,\"valid\":false,\"wifi\":false,\"bt\":false,\"adb\":false,\"ssid\":\"\",\"rssi\":-127}");
            return;
        };
        let _ = write!(out,
            "{{\"screen\":{},\"cursor\":{},\"time\":{},\"valid\":{},\"wifi\":{},\"bt\":{},\"adb\":{},\"link\":{},\"rssi\":{},\"ip\":\"",
            f.screen, f.cursor, f.time_minutes, if f.time_valid { "true" } else { "false" },
            if f.wifi { "true" } else { "false" }, if f.bt { "true" } else { "false" },
            if f.adb { "true" } else { "false" }, f.wifi_link_state, f.wifi_ap_rssi);
        if let Some(ip) = self.ip { let _ = write!(out, "{}", ip); }
        let _ = out.push_str("\",\"ssid\":\"");
        for &b in &f.wifi_ap_ssid[..f.wifi_ap_ssid_len as usize] {
            let ch = if b.is_ascii_alphanumeric() || matches!(b, b' ' | b'-' | b'_' | b'(' | b')') { b as char } else { '?' };
            let _ = out.push(ch);
        }
        let _ = out.push_str("\"}");
    }

    fn poll_ntp(&mut self) {
        if self.ip.is_none() { return; }
        let now = now_ms();
        let socket = self.sockets.get_mut::<udp::Socket>(self.ntp_handle);
        if !socket.is_open() {
            if let Err(e) = socket.bind(NTP_LOCAL_PORT) {
                println!("time: NTP bind failed: {:?}", e);
                self.ntp_next_request_ms = now.saturating_add(NTP_RETRY_MS);
                return;
            }
        }

        if socket.can_recv() {
            let mut packet = [0u8; 64];
            if let Ok((n, _meta)) = socket.recv_slice(&mut packet) {
                if n >= 48 {
                    let mode = packet[0] & 0x07;
                    let stratum = packet[1];
                    let ntp_seconds = u32::from_be_bytes([packet[40], packet[41], packet[42], packet[43]]);
                    if (mode == 4 || mode == 5) && stratum != 0 && ntp_seconds > NTP_UNIX_EPOCH_DELTA {
                        let unix = ntp_seconds - NTP_UNIX_EPOCH_DELTA;
                        self.ntp_unix_seconds = Some(unix);
                        self.ntp_pending = false;
                        self.ntp_next_request_ms = now.saturating_add(NTP_RESYNC_MS);
                        println!("time: NTP sync received unix={}", unix);
                        return;
                    }
                }
            }
        }

        if self.ntp_pending {
            if now.saturating_sub(self.ntp_request_started_ms) < 5_000 { return; }
            self.ntp_pending = false;
            self.ntp_server_index = (self.ntp_server_index + 1) % NTP_SERVERS.len() as u8;
            self.ntp_next_request_ms = now.saturating_add(1_000);
        }
        if now < self.ntp_next_request_ms || !socket.can_send() { return; }

        let mut request = [0u8; 48];
        request[0] = 0x23; // LI=0, SNTP/NTP v4, client mode.
        let server = NTP_SERVERS[self.ntp_server_index as usize];
        let endpoint = IpEndpoint::new(IpAddress::Ipv4(server), NTP_PORT);
        match socket.send_slice(&request, endpoint) {
            Ok(()) => {
                self.ntp_pending = true;
                self.ntp_request_started_ms = now;
                self.ntp_next_request_ms = now.saturating_add(NTP_RETRY_MS);
                println!("time: requesting NTP from {}", server);
            }
            Err(e) => {
                println!("time: NTP request failed: {:?}", e);
                self.ntp_next_request_ms = now.saturating_add(NTP_RETRY_MS);
            }
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
                    self.ntp_next_request_ms = 0;
                    self.ntp_pending = false;
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
        self.ntp_pending = false;
        self.ntp_next_request_ms = 0;
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

fn send_http_response(socket: &mut tcp::Socket<'_>, status: &str, content_type: &str, body: &[u8]) -> bool {
    let mut header = HString::<192>::new();
    let _ = write!(header,
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status, content_type, body.len());
    let needed = header.len().saturating_add(body.len());
    let free = socket.send_capacity().saturating_sub(socket.send_queue());
    if free < needed { return false; }
    if !matches!(socket.send_slice(header.as_bytes()), Ok(n) if n == header.len()) { return false; }
    matches!(socket.send_slice(body), Ok(n) if n == body.len())
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
