// Модуль радиосервисов Wi-Fi/BLE.
//! Native ESP32-S3 connectivity services for AOSP Wear OS.
//!
//! Wi-Fi uses esp-radio for the 802.11 station and smoltcp for DHCP/TCP.
//! Bluetooth exposes a real BLE controller: the watch advertises itself and can
//! perform active discovery using raw HCI without requiring a second heap-heavy
//! host stack just for scanning.

use core::{fmt::Write, time::Duration};

use crate::watch_println as println;
use esp_hal::{
    delay::Delay,
    peripherals::{BT, WIFI},
    time::Instant,
};
use esp_radio::{
    Controller,
    ble::controller::BleConnector,
    wifi::{ClientConfig, ModeConfig, ScanConfig, ScanTypeConfig, WifiController, WifiDevice},
};
use heapless::{String as HString, Vec as HVec};
use static_cell::StaticCell;

use crate::{
    desktop::WifiCredentials,
    net::{LinkState, NetworkService},
    runtime::RuntimeFrame,
};

/// Параметр RADIO компонента радиосервисов Wi-Fi/BLE.
static RADIO: StaticCell<Controller<'static>> = StaticCell::new();

/// Минимум свободной внутренней RAM для старта Wi-Fi.
const MIN_WIFI_INTERNAL_FREE: usize = 32 * 1024;
/// Минимум свободной внутренней RAM перед созданием BLE-контроллера.
const MIN_BLE_INTERNAL_FREE: usize = 24 * 1024;
/// Интервал/срок WIFI_CONNECT_TIMEOUT в миллисекундах.
const WIFI_CONNECT_TIMEOUT_MS: u64 = 30_000;
/// Интервал/срок WIFI_RECONNECT_BACKOFF в миллисекундах.
const WIFI_RECONNECT_BACKOFF_MS: u64 = 5_000;
/// Интервал/срок WIFI_DRIVER_SETTLE в миллисекундах.
const WIFI_DRIVER_SETTLE_MS: u32 = 150;
#[cfg(not(feature = "ble-hid"))]
/// Интервал/срок BLE_SCAN в миллисекундах.
const BLE_SCAN_MS: u64 = 6_000;

/// Возвращает свободную внутреннюю память, необходимую драйверам радио.
fn internal_free_bytes() -> usize {
    let stats = esp_alloc::HEAP.stats();
    stats
        .region_stats
        .iter()
        .filter_map(|region| region.as_ref())
        .filter(|region| {
            region
                .capabilities
                .contains(esp_alloc::MemoryCapability::Internal)
        })
        .map(|region| region.free)
        .sum()
}

/// Возвращает монотонное время в миллисекундах.
fn now_ms() -> u64 {
    Instant::now().duration_since_epoch().as_millis()
}

#[derive(Clone, Debug)]
/// Структура данных компонента радиосервисов Wi-Fi/BLE.
struct WifiAp {
    /// Имя Wi-Fi-сети.
    ssid: HString<32>,
    /// Измеренный уровень сигнала, dBm.
    rssi: i8,
    /// Канал сообщений между владельцами компонента.
    channel: u8,
}

#[cfg(not(feature = "ble-hid"))]
#[derive(Clone, Debug)]
/// Структура данных компонента радиосервисов Wi-Fi/BLE.
struct BleDevice {
    /// Адрес подключаемого или обслуживаемого устройства.
    address: [u8; 6],
    /// Тип Bluetooth-адреса, заданный протоколом.
    address_type: u8,
    /// Измеренный уровень сигнала, dBm.
    rssi: i8,
    /// Имя компонента, устройства или записи.
    name: HString<24>,
}

#[cfg(not(feature = "ble-hid"))]
#[derive(Clone, Copy, Debug)]
/// Структура данных компонента радиосервисов Wi-Fi/BLE.
struct BleConnection {
    /// Адрес подключаемого или обслуживаемого устройства.
    address: [u8; 6],
    handle: u16,
}

/// Владелец Wi-Fi/BLE-драйверов и их неблокирующих состояний компонента радиосервисов Wi-Fi/BLE.
pub struct RadioServices {
    /// Владелец аппаратного контроллера или его интерфейс.
    controller: &'static Controller<'static>,
    /// Системный флаг либо драйвер станции Wi-Fi.
    wifi: WifiController<'static>,
    /// Сетевой интерфейс станции, которым владеет радиосервис.
    wifi_device: WifiDevice<'static>,
    net: NetworkService,
    /// Ещё не переданный драйверу аппаратный BT-периферийный блок.
    bt_device: Option<BT<'static>>,
    #[cfg(not(feature = "ble-hid"))]
    ble: Option<BleConnector<'static>>,
    #[cfg(feature = "ble-hid")]
    /// Единственный BLE HID-host в соответствующей сборке.
    hid: Option<crate::ble_host::BleHost>,
    #[cfg(feature = "ble-hid")]
    /// Ключи сопряжения восстановлены до первого опроса host.
    pub(crate) bonds_loaded: bool,
    #[cfg(feature = "ble-hid")]
    /// Признак отказа создания или работы HID-host.
    hid_error: bool,
    /// Требуемое пользователем состояние Wi-Fi.
    wifi_requested: bool,
    /// Требуемое пользователем состояние Bluetooth.
    bt_requested: bool,
    /// Последний подтверждённый профиль Wi-Fi.
    saved_wifi: Option<WifiCredentials>,
    // Candidate credentials are not committed to persistent storage until the
    // station actually associates. This prevents a one-key keyboard typo from
    // replacing the last known-good profile and creating an endless retry loop.
    connecting_wifi: Option<WifiCredentials>,
    /// Проверенный профиль, ожидающий записи на носитель.
    persist_wifi: Option<WifiCredentials>,
    /// Монотонное начало текущей попытки Wi-Fi-подключения.
    wifi_connect_started: Option<u64>,
    /// Монотонный срок следующей разрешённой попытки Wi-Fi.
    wifi_reconnect_at: u64,
    /// Число подряд неудачных попыток подключения.
    wifi_failures: u8,
    /// Ограниченный кэш найденных точек доступа.
    wifi_aps: HVec<WifiAp, 10>,
    #[cfg(not(feature = "ble-hid"))]
    ble_devices: HVec<BleDevice, 10>,
    #[cfg(not(feature = "ble-hid"))]
    ble_scanning_until: Option<u64>,
    #[cfg(not(feature = "ble-hid"))]
    ble_advertising: bool,
    #[cfg(not(feature = "ble-hid"))]
    ble_pending_address: Option<[u8; 6]>,
    #[cfg(not(feature = "ble-hid"))]
    ble_pending_since: Option<u64>,
    #[cfg(not(feature = "ble-hid"))]
    ble_connection: Option<BleConnection>,
}

impl RadioServices {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
    pub fn new(wifi: WIFI<'static>, bt: BT<'static>) -> Option<Self> {
        let controller: &'static Controller<'static> = match esp_radio::init() {
            Ok(c) => &*RADIO.init(c),
            Err(e) => {
                println!("radio: init failed: {:?}", e);
                return None;
            }
        };

        let (wifi_controller, interfaces) =
            match esp_radio::wifi::new(controller, wifi, Default::default()) {
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
            #[cfg(not(feature = "ble-hid"))]
            ble: None,
            #[cfg(feature = "ble-hid")]
            hid: None,
            #[cfg(feature = "ble-hid")]
            bonds_loaded: false,
            #[cfg(feature = "ble-hid")]
            hid_error: false,
            wifi_requested: false,
            bt_requested: false,
            saved_wifi: None,
            connecting_wifi: None,
            persist_wifi: None,
            wifi_connect_started: None,
            wifi_reconnect_at: 0,
            wifi_failures: 0,
            wifi_aps: HVec::new(),
            #[cfg(not(feature = "ble-hid"))]
            ble_devices: HVec::new(),
            #[cfg(not(feature = "ble-hid"))]
            ble_scanning_until: None,
            #[cfg(not(feature = "ble-hid"))]
            ble_advertising: false,
            #[cfg(not(feature = "ble-hid"))]
            ble_pending_address: None,
            #[cfg(not(feature = "ble-hid"))]
            ble_pending_since: None,
            #[cfg(not(feature = "ble-hid"))]
            ble_connection: None,
        })
    }

    /// Передаёт сохранённый Wi-Fi-профиль радиосервису.
    pub fn set_saved_wifi(&mut self, credentials: WifiCredentials) {
        self.saved_wifi = Some(credentials);
    }

    /// Возвращает текущий этап подключения к сети.
    pub fn network_state(&self) -> LinkState {
        self.net.link_state()
    }
    /// Возвращает выданный IP-адрес, если он уже известен.
    pub fn ip_address(&self) -> Option<smoltcp::wire::Ipv4Address> {
        self.net.ip()
    }
    /// Забирает полученное по NTP время для синхронизации RTC.
    pub fn take_network_time(&mut self) -> Option<u32> {
        self.net.take_network_time()
    }
    /// Запрашивает обновление сетевого времени.
    pub fn request_network_time_sync(&mut self) {
        self.net.request_time_sync();
    }
    /// Забирает только успешно проверенный профиль для записи в настройки.
    pub fn take_connected_wifi_for_persist(&mut self) -> Option<WifiCredentials> {
        self.persist_wifi.take()
    }
    /// Возвращает количество найденных точек доступа.
    pub fn wifi_ap_count(&self) -> usize {
        self.wifi_aps.len()
    }
    /// Возвращает сведения об одной найденной точке доступа.
    pub fn wifi_ap_info(&self, index: usize) -> Option<(&str, i8)> {
        self.wifi_aps
            .get(index)
            .map(|ap| (ap.ssid.as_str(), ap.rssi))
    }

    /// Применяет системные флаги интерфейса к радиосервисам.
    pub fn sync_frame(&mut self, frame: &RuntimeFrame) {
        let RuntimeFrame::Material(m) = frame else {
            return;
        };
        if m.wifi != self.wifi_requested {
            self.set_wifi(m.wifi);
        }
        if m.bt != self.bt_requested {
            self.set_bluetooth(m.bt);
        }
    }

    /// Продвигает состояние сервиса по монотонному времени.
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
                        println!(
                            "radio: three failed associations; rejecting unverified candidate and resetting STA"
                        );
                        if self.connecting_wifi.is_some() && self.connecting_wifi != self.saved_wifi
                        {
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
                        println!(
                            "radio: reconnecting {} Wi-Fi profile",
                            if self.connecting_wifi.is_some() {
                                "candidate"
                            } else {
                                "saved"
                            }
                        );
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

#[cfg(not(feature = "ble-hid"))]
#[path = "radio/ble.rs"]
mod ble;
#[cfg(not(feature = "ble-hid"))]
#[path = "radio/hci.rs"]
mod hci;
#[cfg(feature = "ble-hid")]
#[path = "radio/hid.rs"]
mod hid;
#[path = "radio/wifi.rs"]
mod wifi;
#[cfg(not(feature = "ble-hid"))]
use hci::{hci_command, parse_ble_event};

/// Добавляет ограниченный текст без управляющих символов.
fn push_safe_text<const N: usize>(out: &mut HString<N>, text: &str) {
    for ch in text.chars() {
        let safe = if ch == '\r' || ch == '\n' || ch == '|' {
            '?'
        } else {
            ch
        };
        if out.push(safe).is_err() {
            break;
        }
    }
}
