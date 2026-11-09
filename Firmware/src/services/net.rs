// Модуль IPv4, DHCP, NTP и ADB.
//! Minimal IPv4/DHCP + wireless ADB transport for AOSP Wear OS.
//!
//! esp-radio owns the 802.11 station, smoltcp owns IPv4/DHCP/TCP and this
//! module exposes an adbd-compatible TCP listener on port 5555.  v0.6 keeps a
//! real ADB stream state so one-shot commands terminate correctly and
//! `adb shell` can stay interactive instead of hanging forever.

extern crate alloc;

use crate::watch_println as println;
#[path = "net/google_client.rs"]
mod google_client;
#[path = "net/weather_client.rs"]
mod weather_client;
use alloc::vec;
use core::fmt::Write;
use esp_hal::time::Instant as HalInstant;
use esp_radio::wifi::WifiDevice;
use heapless::String as HString;
use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet},
    socket::{dhcpv4, tcp, udp},
    time::{Duration as SmolDuration, Instant},
    wire::{DhcpOption, EthernetAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address},
};
use static_cell::StaticCell;

use crate::adb::{self, A_CLSE, A_CNXN, A_OKAY, A_OPEN, A_WRTE, Header};

/// Параметр ADB_MAX_PAYLOAD компонента IPv4, DHCP, NTP и ADB.
const ADB_MAX_PAYLOAD: usize = 1024;
/// Версия двоичного формата ADB.
const ADB_VERSION: u32 = 0x0100_0001;
/// Параметр ADB_MAX_DATA компонента IPv4, DHCP, NTP и ADB.
const ADB_MAX_DATA: u32 = ADB_MAX_PAYLOAD as u32;
/// TCP-порт встроенного беспроводного ADB.
const WADB_PORT: u16 = 5555;
/// Интервал/срок ADB_CLOSE_GRACE в миллисекундах.
const ADB_CLOSE_GRACE_MS: u64 = 1_200;
/// Имя станции, передаваемое в DHCP.
static DHCP_HOSTNAME: &[u8] = crate::device_info::HOSTNAME.as_bytes();
/// Дополнительные DHCP-параметры станции.
static DHCP_OPTIONS: [DhcpOption<'static>; 1] = [DhcpOption {
    kind: 12,
    data: DHCP_HOSTNAME,
}];

/// Ограниченный рабочий буфер/метаданные WADB_RX_BUF компонента IPv4, DHCP, NTP и ADB.
static WADB_RX_BUF: StaticCell<[u8; 4096]> = StaticCell::new();
/// Ограниченный рабочий буфер/метаданные WADB_TX_BUF компонента IPv4, DHCP, NTP и ADB.
static WADB_TX_BUF: StaticCell<[u8; 4096]> = StaticCell::new();
/// Ограниченный рабочий буфер/метаданные NTP_RX_META компонента IPv4, DHCP, NTP и ADB.
static NTP_RX_META: StaticCell<[udp::PacketMetadata; 2]> = StaticCell::new();
/// Ограниченный рабочий буфер/метаданные NTP_TX_META компонента IPv4, DHCP, NTP и ADB.
static NTP_TX_META: StaticCell<[udp::PacketMetadata; 2]> = StaticCell::new();
/// Ограниченный рабочий буфер/метаданные NTP_RX_BUF компонента IPv4, DHCP, NTP и ADB.
static NTP_RX_BUF: StaticCell<[u8; 128]> = StaticCell::new();
/// Ограниченный рабочий буфер/метаданные NTP_TX_BUF компонента IPv4, DHCP, NTP и ADB.
static NTP_TX_BUF: StaticCell<[u8; 128]> = StaticCell::new();

/// Стандартный UDP-порт сервера NTP.
const NTP_PORT: u16 = 123;
/// Параметр NTP_LOCAL_PORT компонента IPv4, DHCP, NTP и ADB.
const NTP_LOCAL_PORT: u16 = 49152;
/// Разница эпох NTP и Unix, секунды.
const NTP_UNIX_EPOCH_DELTA: u32 = 2_208_988_800;
/// Интервал/срок NTP_RETRY в миллисекундах.
const NTP_RETRY_MS: u64 = 15_000;
/// Интервал/срок NTP_RESYNC в миллисекундах.
const NTP_RESYNC_MS: u64 = 6 * 60 * 60 * 1000;
// Google Public NTP time1..time4.google.com anycast IPv4 endpoints.
const NTP_SERVERS: [Ipv4Address; 4] = [
    Ipv4Address::new(216, 239, 35, 0),
    Ipv4Address::new(216, 239, 35, 4),
    Ipv4Address::new(216, 239, 35, 8),
    Ipv4Address::new(216, 239, 35, 12),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Этап подключения станции и получения IP-адреса компонента IPv4, DHCP, NTP и ADB.
pub enum LinkState {
    Down,
    Associating,
    LinkUp,
    Dhcp,
    Online,
}

/// Проверенный заголовок и заимствованный payload ADB компонента IPv4, DHCP, NTP и ADB.
struct AdbMessage<'a> {
    /// Разобранный заголовок текущего пакета.
    header: Header,
    /// Полезные данные пакета без заголовка.
    payload: &'a [u8],
}

/// Накопитель заголовка и payload входящего ADB-пакета компонента IPv4, DHCP, NTP и ADB.
struct AdbRx {
    /// Байты ещё не разобранного заголовка пакета.
    header_raw: [u8; 24],
    /// Число значащих байтов буфера header.
    header_len: usize,
    /// Разобранный заголовок текущего пакета.
    header: Option<Header>,
    /// Полезные данные пакета без заголовка.
    payload: [u8; ADB_MAX_PAYLOAD],
    /// Число значащих байтов буфера payload.
    payload_len: usize,
}

impl AdbRx {
    /// Подготавливает сетевое состояние, сокеты и ограниченные буферы выбранного компонента.
    const fn new() -> Self {
        Self {
            header_raw: [0; 24],
            header_len: 0,
            header: None,
            payload: [0; ADB_MAX_PAYLOAD],
            payload_len: 0,
        }
    }

    /// Сбрасывает временное состояние разборщика или соединения.
    fn reset(&mut self) {
        self.header_len = 0;
        self.header = None;
        self.payload_len = 0;
    }

    /// Добавляет очередную порцию входных данных в разборщик.
    fn push(&mut self, byte: u8) -> Option<AdbMessage<'_>> {
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
                return Some(AdbMessage {
                    header,
                    payload: &[],
                });
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

        let len = self.payload_len;
        // ADB protocol 0x01000001 (advertised by this device) deliberately
        // skips payload checksums. Modern adb therefore sends data_check=0.
        // Keep accepting the legacy checksum too for older hosts.
        let valid =
            header.data_check == 0 || adb::checksum(&self.payload[..len]) == header.data_check;
        self.reset();
        if valid {
            Some(AdbMessage {
                header,
                payload: &self.payload[..len],
            })
        } else {
            println!("adbd: packet checksum mismatch");
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Режим обслуживания ADB-потока компонента IPv4, DHCP, NTP и ADB.
enum AdbStreamMode {
    Closed,
    OneShot,
    Interactive,
    Logcat,
    Sync,
}

/// Состояние одного ACK-paced ADB-потока компонента IPv4, DHCP, NTP и ADB.
struct AdbStream {
    /// Идентификатор ADB-потока на стороне компьютера.
    host_id: u32,
    /// Локальный идентификатор потока/устройства в соответствующем протоколе.
    device_id: u32,
    /// Текущий режим компонента; переходы выполняет обработчик событий.
    mode: AdbStreamMode,
    /// Клиент должен подтвердить предыдущий ADB-пакет до следующей передачи.
    waiting_for_okay: bool,
    sync_ack_pending: bool,
    /// Поток закрывается после подтверждения завершающего ответа.
    close_after_okay: bool,
    /// Монотонное время/интервал close_deadline в миллисекундах.
    close_deadline_ms: u64,
    /// Входные данные или временно введённые символы.
    input: HString<256>,
    /// Подготовленный результат или очередь исходящих данных.
    output: alloc::boxed::Box<HString<2048>>,
    /// Позиция уже переданного фрагмента выходной очереди.
    output_pos: usize,
    logcat: Option<adb::logcat::Session>,
    sync: Option<adb::sync::Receiver>,
}

impl AdbStream {
    /// Подготавливает сетевое состояние, сокеты и ограниченные буферы выбранного компонента.
    fn new() -> Self {
        Self {
            host_id: 0,
            device_id: 0,
            mode: AdbStreamMode::Closed,
            waiting_for_okay: false,
            sync_ack_pending: false,
            close_after_okay: false,
            close_deadline_ms: 0,
            input: HString::new(),
            output: alloc::boxed::Box::new(HString::new()),
            output_pos: 0,
            logcat: None,
            sync: None,
        }
    }

    /// Очищает временное состояние или буфер компонента.
    fn clear(&mut self) {
        self.host_id = 0;
        self.device_id = 0;
        self.mode = AdbStreamMode::Closed;
        self.waiting_for_okay = false;
        self.sync_ack_pending = false;
        self.close_after_okay = false;
        self.close_deadline_ms = 0;
        self.input.clear();
        self.output.clear();
        self.output_pos = 0;
        self.logcat = None;
        self.sync = None;
    }

    /// Открывает выбранный режим и подготавливает его состояние.
    fn open(&mut self, host_id: u32, device_id: u32, mode: AdbStreamMode) {
        self.clear();
        self.host_id = host_id;
        self.device_id = device_id;
        self.mode = mode;
    }

    /// Проверяет совпадение идентификаторов или проверяемого значения.
    fn matches(&self, host_id: u32, device_id: u32) -> bool {
        self.mode != AdbStreamMode::Closed && self.host_id == host_id && self.device_id == device_id
    }
}

/// Владелец IP-интерфейса и сетевых сокетов компонента IPv4, DHCP, NTP и ADB.
pub struct NetworkService {
    /// Сетевой IPv4-интерфейс smoltcp.
    iface: Interface,
    /// Коллекция принадлежащих сервису сокетов.
    sockets: SocketSet<'static>,
    /// Дескриптор DHCP-сокета.
    dhcp_handle: SocketHandle,
    /// Дескриптор TCP-сокета ADB.
    adb_handle: SocketHandle,
    adb_listener: SocketHandle,
    /// Дескриптор UDP-сокета NTP.
    ntp_handle: SocketHandle,
    /// Последний проверенный погодный снимок.
    weather: Option<weather_client::WeatherClient>,
    google: Option<google_client::GoogleClient>,
    /// Накопитель входящего ADB-пакета.
    adb_rx: AdbRx,
    /// Состояние единственного текущего ADB-потока.
    adb_streams: alloc::vec::Vec<AdbStream>,
    /// Источник локальных ID новых ADB-потоков.
    next_adb_local_id: u32,
    /// Последний выданный IP-адрес.
    ip: Option<Ipv4Address>,
    /// Канал связи с устройством/сервером.
    link: LinkState,
    /// Признак открытого TCP-listener ADB.
    wadb_listening: bool,
    /// Признак того, что listener уже принял клиента.
    wadb_seen_client: bool,
    /// Монотонное время/интервал ntp_next_request в миллисекундах.
    ntp_next_request_ms: u64,
    /// Монотонное время/интервал ntp_request_started в миллисекундах.
    ntp_request_started_ms: u64,
    /// Индекс сервера в списке резервных источников NTP.
    ntp_server_index: u8,
    /// Признак незавершённого запроса времени.
    ntp_pending: bool,
    /// Полученное сетевое время, ожидающее передачи RTC.
    ntp_unix_seconds: Option<u32>,
}

impl NetworkService {
    /// Подготавливает сетевое состояние, сокеты и ограниченные буферы выбранного компонента.
    pub fn new(device: &mut WifiDevice<'static>) -> Self {
        let mac = device.mac_address();
        let mut cfg = Config::new(EthernetAddress(mac).into());
        cfg.random_seed =
            u64::from_le_bytes([mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], 0x5a, 0x33]);
        let iface = Interface::new(cfg, device, Instant::ZERO);

        let mut sockets = SocketSet::new(vec![]);
        let mut dhcp = dhcpv4::Socket::new();
        // DHCP option 12 makes Android/MIUI hotspot client lists show a real
        // hostname instead of falling back to the station MAC address.
        dhcp.set_outgoing_options(&DHCP_OPTIONS);
        let dhcp_handle = sockets.add(dhcp);
        let rx = &mut WADB_RX_BUF.init([0; 4096])[..];
        let tx = &mut WADB_TX_BUF.init([0; 4096])[..];
        let mut adb_socket =
            tcp::Socket::new(tcp::SocketBuffer::new(rx), tcp::SocketBuffer::new(tx));
        // ADB sends many tiny 24-byte control packets. Nagle adds avoidable
        // latency to OKAY/WRTE/CLSE handshakes on a local Wi-Fi network.
        adb_socket.set_nagle_enabled(false);
        adb_socket.set_keep_alive(Some(SmolDuration::from_secs(15)));
        adb_socket.set_timeout(None);
        let adb_handle = sockets.add(adb_socket);
        let mut listener = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0; 4096]),
            tcp::SocketBuffer::new(vec![0; 4096]),
        );
        listener.set_nagle_enabled(false);
        listener.set_keep_alive(Some(SmolDuration::from_secs(15)));
        let adb_listener = sockets.add(listener);

        let ntp_rx_meta = &mut NTP_RX_META.init([udp::PacketMetadata::EMPTY; 2])[..];
        let ntp_tx_meta = &mut NTP_TX_META.init([udp::PacketMetadata::EMPTY; 2])[..];
        let ntp_rx_data = &mut NTP_RX_BUF.init([0; 128])[..];
        let ntp_tx_data = &mut NTP_TX_BUF.init([0; 128])[..];
        let ntp_socket = udp::Socket::new(
            udp::PacketBuffer::new(ntp_rx_meta, ntp_rx_data),
            udp::PacketBuffer::new(ntp_tx_meta, ntp_tx_data),
        );
        let ntp_handle = sockets.add(ntp_socket);

        let weather = weather_client::WeatherClient::new(&mut sockets);
        let google = google_client::GoogleClient::new(&mut sockets);
        Self {
            iface,
            weather,
            google,
            sockets,
            dhcp_handle,
            adb_handle,
            adb_listener,
            ntp_handle,
            adb_rx: AdbRx::new(),
            adb_streams: (0..4).map(|_| AdbStream::new()).collect(),
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
        }
    }

    /// Обновляет состояние сети и связанные операции.
    pub fn set_link_state(&mut self, state: LinkState) {
        self.link = state;
        if matches!(
            state,
            LinkState::Down | LinkState::Associating | LinkState::LinkUp
        ) {
            self.clear_ip();
        }
    }

    /// Возвращает текущее состояние сети.
    pub fn link_state(&self) -> LinkState {
        self.link
    }
    /// Возвращает текущий IPv4-адрес интерфейса.
    pub fn ip(&self) -> Option<Ipv4Address> {
        self.ip
    }
    /// Сообщает, активен ли беспроводной ADB.
    pub fn wadb_active(&self) -> bool {
        self.wadb_listening
    }
    /// Сообщает, подключался ли клиент беспроводного ADB.
    pub fn wadb_seen_client(&self) -> bool {
        self.wadb_seen_client
    }
    /// Забирает полученное по NTP время для синхронизации RTC.
    pub fn take_network_time(&mut self) -> Option<u32> {
        self.ntp_unix_seconds.take()
    }
    /// Ставит синхронизацию NTP в очередь.
    pub fn request_time_sync(&mut self) {
        self.ntp_pending = false;
        self.ntp_next_request_ms = 0;
    }
    /// Сбрасывает временное состояние разборщика или соединения.
    pub fn reset(&mut self) {
        self.link = LinkState::Down;
        self.clear_ip();
        self.adb_rx.reset();
        for stream in &mut self.adb_streams {
            stream.clear();
        }
        let socket = self.sockets.get_mut::<tcp::Socket>(self.adb_handle);
        if socket.is_open() {
            socket.abort();
        }
        self.sockets
            .get_mut::<tcp::Socket>(self.adb_listener)
            .abort();
        self.wadb_listening = false;
        self.wadb_seen_client = false;
        self.ntp_pending = false;
        self.ntp_next_request_ms = 0;
        self.ntp_request_started_ms = 0;
        let ntp = self.sockets.get_mut::<udp::Socket>(self.ntp_handle);
        if ntp.is_open() {
            ntp.close();
        }
    }

    /// Выполняет очередной неблокирующий шаг обработки и проверяет готовые результаты.
    pub fn poll(&mut self, device: &mut WifiDevice<'static>, wadb_enabled: bool) {
        // Do not let DHCP emit packets while the station is stopped or still
        // associating. smoltcp has no Wi-Fi association callback itself.
        if matches!(self.link, LinkState::Dhcp | LinkState::Online) {
            let now = smol_now();
            let _ = self.iface.poll(now, device, &mut self.sockets);
            self.poll_dhcp();
            self.poll_wadb(wadb_enabled);
            self.poll_ntp();
            if let Some(google) = self.google.as_mut() {
                google.poll(&mut self.iface, &mut self.sockets, self.ip.is_some());
            }
            if let Some(weather) = self.weather.as_mut() {
                weather.poll(&mut self.iface, &mut self.sockets, self.ip.is_some());
            }
        } else {
            self.poll_wadb(false);
            if let Some(google) = self.google.as_mut() {
                google.poll(&mut self.iface, &mut self.sockets, false);
            }
            if let Some(weather) = self.weather.as_mut() {
                weather.poll(&mut self.iface, &mut self.sockets, false);
            }
            self.ntp_pending = false;
        }
    }

    /// Продвигает обмен NTP с ограничениями времени и размера.
    fn poll_ntp(&mut self) {
        if self.ip.is_none() {
            return;
        }
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
                    let ntp_seconds =
                        u32::from_be_bytes([packet[40], packet[41], packet[42], packet[43]]);
                    if (mode == 4 || mode == 5)
                        && stratum != 0
                        && ntp_seconds > NTP_UNIX_EPOCH_DELTA
                    {
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
            if now.saturating_sub(self.ntp_request_started_ms) < 5_000 {
                return;
            }
            self.ntp_pending = false;
            self.ntp_server_index = (self.ntp_server_index + 1) % NTP_SERVERS.len() as u8;
            self.ntp_next_request_ms = now.saturating_add(1_000);
        }
        if now < self.ntp_next_request_ms || !socket.can_send() {
            return;
        }

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

    /// Обрабатывает выдачу или потерю DHCP-конфигурации.
    fn poll_dhcp(&mut self) {
        let event = self
            .sockets
            .get_mut::<dhcpv4::Socket>(self.dhcp_handle)
            .poll();
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
                let servers: alloc::vec::Vec<IpAddress> = config
                    .dns_servers
                    .iter()
                    .copied()
                    .map(IpAddress::Ipv4)
                    .collect();
                if let Some(weather) = self.weather.as_mut() {
                    weather.servers(&mut self.sockets, &servers);
                }
                if let Some(google) = self.google.as_mut() {
                    google.servers(&mut self.sockets, &servers);
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

    /// Удаляет устаревшую IP-конфигурацию после потери сети.
    fn clear_ip(&mut self) {
        self.ip = None;
        self.ntp_pending = false;
        self.ntp_next_request_ms = 0;
        self.iface.update_ip_addrs(|addrs| addrs.clear());
        self.iface.routes_mut().remove_default_ipv4_route();
        self.sockets
            .get_mut::<dhcpv4::Socket>(self.dhcp_handle)
            .reset();
    }

    /// Обслуживает TCP/ADB без блокирующего ожидания клиента.
    fn poll_wadb(&mut self, enabled: bool) {
        let online = self.ip.is_some();
        // A listening socket remains available while the current TCP peer is active.
        // An explicit reconnect replaces the old transport; ordinary shell/logcat/SYNC
        // channels multiplex on that transport instead of closing each other.
        if !enabled || !online {
            self.sockets
                .get_mut::<tcp::Socket>(self.adb_listener)
                .abort();
        } else {
            let listener = self.sockets.get_mut::<tcp::Socket>(self.adb_listener);
            if !listener.is_open() {
                let _ = listener.listen(WADB_PORT);
            }
            if listener.can_recv() {
                core::mem::swap(&mut self.adb_handle, &mut self.adb_listener);
                self.sockets
                    .get_mut::<tcp::Socket>(self.adb_listener)
                    .abort();
                self.adb_rx.reset();
                for stream in &mut self.adb_streams {
                    stream.clear();
                }
                println!("adbd: replacing TCP transport after explicit reconnect");
            }
        }
        let socket = self.sockets.get_mut::<tcp::Socket>(self.adb_handle);

        if !enabled || !online {
            if socket.is_open() {
                socket.abort();
            }
            self.wadb_listening = false;
            self.adb_rx.reset();
            for stream in &mut self.adb_streams {
                stream.clear();
            }
            return;
        }

        if matches!(socket.state(), tcp::State::CloseWait | tcp::State::TimeWait)
            && !socket.can_recv()
        {
            socket.abort();
            self.adb_rx.reset();
            for stream in &mut self.adb_streams {
                stream.clear();
            }
        }
        if !socket.is_open() {
            match socket.listen(WADB_PORT) {
                Ok(()) => {
                    self.wadb_listening = true;
                    self.wadb_seen_client = false;
                    for stream in &mut self.adb_streams {
                        stream.clear();
                    }
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
        // One RX batch per OS iteration keeps a busy ADB host from starving UI.
        if socket.can_recv() {
            let Ok(n) = socket.recv_slice(&mut incoming) else {
                return;
            };
            if n == 0 {
                return;
            }
            for &byte in &incoming[..n] {
                if let Some(message) = self.adb_rx.push(byte) {
                    let slot = match message.header.command {
                        A_CNXN => {
                            for stream in &mut self.adb_streams {
                                stream.clear();
                            }
                            Some(0)
                        }
                        A_OPEN => self
                            .adb_streams
                            .iter()
                            .position(|s| s.mode == AdbStreamMode::Closed),
                        _ => self
                            .adb_streams
                            .iter()
                            .position(|s| s.matches(message.header.arg0, message.header.arg1)),
                    };
                    if let Some(index) = slot {
                        handle_adb(
                            socket,
                            message,
                            &mut self.adb_streams[index],
                            &mut self.next_adb_local_id,
                        );
                    } else {
                        let _ = send_packet(socket, A_CLSE, 0, message.header.arg0, &[]);
                    }
                }
            }
        }

        for stream in &mut self.adb_streams {
            service_stream(socket, stream);
            if stream.mode == AdbStreamMode::OneShot
                && stream.close_after_okay
                && stream.close_deadline_ms != 0
                && now_ms() >= stream.close_deadline_ms
            {
                let _ = send_packet(socket, A_CLSE, stream.device_id, stream.host_id, &[]);
                stream.clear();
            }
        }
    }
}

/// Преобразует монотонное время в формат smoltcp.
fn smol_now() -> Instant {
    Instant::from_millis(HalInstant::now().duration_since_epoch().as_millis() as i64)
}

/// Возвращает монотонное время в миллисекундах.
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
        // Normal TCP backpressure: retry later without producing log feedback.
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
    if !payload.is_empty() && !matches!(socket.send_slice(payload), Ok(n) if n == payload.len()) {
        // This should not happen after the capacity check; if it ever does the
        // transport is already unusable, so make it visible in the log.
        println!("adbd: failed to queue complete payload");
        return false;
    }
    true
}

/// Выдаёт следующий ненулевой идентификатор ADB-потока.
fn next_local_id(next: &mut u32) -> u32 {
    let id = (*next).max(1);
    *next = next.wrapping_add(1).max(1);
    id
}

#[path = "net/adb_transport.rs"]
mod adb_transport;
use adb_transport::{handle_adb, service_stream};
