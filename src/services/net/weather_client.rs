// Модуль неблокирующего клиента погоды.
//! Small nonblocking HTTP/DNS client for public weather data. No credentials or HTML scraping.
use super::*;
use crate::weather::{self, Request, Snapshot};
use alloc::vec::Vec;
use smoltcp::{socket::dns, wire::DnsQueryType};
/// Параметр LIMIT компонента неблокирующего клиента погоды.
const LIMIT: usize = 12288;
/// Структура данных компонента неблокирующего клиента погоды.
enum Phase {
    Idle,
    Dns(dns::QueryHandle),
    Connecting,
    Receiving,
}
#[derive(Clone, Copy)]
/// Структура данных компонента неблокирующего клиента погоды.
enum Job {
    Location,
    Search,
    Forecast,
}
/// Структура данных компонента неблокирующего клиента погоды.
pub struct WeatherClient {
    /// Дескриптор DNS-сокета для разрешения имени сервиса.
    dns: SocketHandle,
    /// Дескриптор TCP-сокета текущего клиента.
    tcp: SocketHandle,
    /// Текущий этап конечного автомата.
    phase: Phase,
    /// Ожидающая операция вместе с идентификатором владельца ответа.
    job: Job,
    /// Имя удалённого узла или интерфейс BLE host.
    host: &'static str,
    /// Полный путь записи внутри управляемого хранилища.
    path: HString<512>,
    /// Рабочий буфер с ограниченным размером.
    buffer: Vec<u8>,
    deadline: u64,
    /// Метаданные компонента, выдаваемые клиенту.
    info: Snapshot,
}
/// Выделяет ограниченный буфер во внешней памяти PSRAM.
fn external(capacity: usize) -> Option<Vec<u8>> {
    let layout = core::alloc::Layout::array::<u8>(capacity).ok()?;
    // SAFETY: the global allocator is HEAP; Vec owns exactly this allocation/layout.
    let pointer =
        unsafe { esp_alloc::HEAP.alloc_caps(esp_alloc::MemoryCapability::External.into(), layout) };
    if pointer.is_null() {
        None
    } else {
        Some(unsafe { Vec::from_raw_parts(pointer, 0, capacity) })
    }
}
impl WeatherClient {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
    pub fn new(sockets: &mut SocketSet<'static>) -> Option<Self> {
        let mut rx = external(2048)?;
        rx.resize(2048, 0);
        let mut tx = external(1024)?;
        tx.resize(1024, 0);
        let buffer = external(LIMIT)?;
        let dns = sockets.add(dns::Socket::new(
            &[IpAddress::v4(1, 1, 1, 1), IpAddress::v4(8, 8, 8, 8)],
            Vec::new(),
        ));
        let mut socket = tcp::Socket::new(tcp::SocketBuffer::new(rx), tcp::SocketBuffer::new(tx));
        socket.set_timeout(Some(SmolDuration::from_secs(12)));
        let tcp = sockets.add(socket);
        Some(Self {
            dns,
            tcp,
            phase: Phase::Idle,
            job: Job::Location,
            host: "",
            path: HString::new(),
            buffer,
            deadline: 0,
            info: Snapshot::default(),
        })
    }
    /// Обновляет список DNS-серверов по данным сети.
    pub fn servers(&mut self, sockets: &mut SocketSet<'static>, servers: &[IpAddress]) {
        if !servers.is_empty() {
            sockets
                .get_mut::<dns::Socket>(self.dns)
                .update_servers(servers);
        }
    }
    /// Прекращает текущую сетевую операцию и очищает её временные данные.
    fn abort(&mut self, sockets: &mut SocketSet<'static>) {
        if let Phase::Dns(query) = self.phase {
            sockets.get_mut::<dns::Socket>(self.dns).cancel_query(query);
        }
        sockets.get_mut::<tcp::Socket>(self.tcp).abort();
        self.phase = Phase::Idle;
        self.buffer.clear();
    }
    /// Подготавливает запуск компонента и его исходное состояние.
    fn start(
        &mut self,
        iface: &mut Interface,
        sockets: &mut SocketSet<'static>,
        host: &'static str,
        path: HString<512>,
        job: Job,
    ) {
        self.abort(sockets);
        self.host = host;
        self.path = path;
        self.job = job;
        self.deadline = now_ms() + 20000;
        match sockets.get_mut::<dns::Socket>(self.dns).start_query(
            iface.context(),
            host,
            DnsQueryType::A,
        ) {
            Ok(q) => self.phase = Phase::Dns(q),
            Err(_) => weather::fail(4),
        }
    }
    /// Формирует запрос прогноза для выбранных координат.
    fn forecast(&mut self, iface: &mut Interface, sockets: &mut SocketSet<'static>) {
        let mut path = HString::new();
        let _ = write!(
            path,
            "/v1/forecast?latitude={:.6}&longitude={:.6}&current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m&wind_speed_unit=ms",
            f64::from(self.info.lat) / 1e6,
            f64::from(self.info.lon) / 1e6
        );
        self.start(iface, sockets, "api.open-meteo.com", path, Job::Forecast);
    }
    /// Выполняет очередной неблокирующий шаг обработки и проверяет готовые результаты.
    pub fn poll(&mut self, iface: &mut Interface, sockets: &mut SocketSet<'static>, online: bool) {
        weather::online(online);
        if let Some(request) = weather::take_request() {
            if !online {
                weather::fail(3);
                return;
            }
            match request {
                Request::Location => {
                    let mut path = HString::new();
                    let _ = path.push_str("/json/?fields=status,lat,lon,city&lang=ru");
                    self.info = Snapshot::default();
                    self.info.approximate = true;
                    self.start(iface, sockets, "ip-api.com", path, Job::Location);
                }
                Request::Search(query) => {
                    let mut path = HString::new();
                    let _ = path.push_str("/v1/search?count=1&language=ru&format=json&name=");
                    for b in query.trim().as_bytes() {
                        let _ = write!(path, "%{:02X}", b);
                    }
                    self.info = Snapshot::default();
                    self.start(
                        iface,
                        sockets,
                        "geocoding-api.open-meteo.com",
                        path,
                        Job::Search,
                    );
                }
                Request::Refresh => {
                    self.info = weather::snapshot();
                    if self.info.city_len > 0 {
                        self.forecast(iface, sockets);
                    } else {
                        weather::fail(5);
                    }
                }
            }
        }
        if matches!(self.phase, Phase::Idle) {
            return;
        }
        if !online || now_ms() > self.deadline {
            self.abort(sockets);
            weather::fail(if online { 6 } else { 3 });
            return;
        }
        if let Phase::Dns(query) = self.phase {
            match sockets
                .get_mut::<dns::Socket>(self.dns)
                .get_query_result(query)
            {
                Ok(addresses) => {
                    self.phase = Phase::Idle;
                    if let Some(address) = addresses.first() {
                        if sockets
                            .get_mut::<tcp::Socket>(self.tcp)
                            .connect(iface.context(), (*address, 80), 49170)
                            .is_ok()
                        {
                            self.phase = Phase::Connecting;
                        } else {
                            self.abort(sockets);
                            weather::fail(4);
                        }
                    } else {
                        self.abort(sockets);
                        weather::fail(4);
                    }
                }
                Err(dns::GetQueryResultError::Pending) => {}
                Err(_) => {
                    self.phase = Phase::Idle;
                    weather::fail(4);
                }
            }
        }
        let socket = sockets.get_mut::<tcp::Socket>(self.tcp);
        if matches!(self.phase, Phase::Connecting) && socket.can_send() {
            let mut request = HString::<768>::new();
            let _ = write!(
                request,
                "GET {} HTTP/1.0\r\nHost: {}\r\nAccept: application/json\r\nAccept-Encoding: identity\r\nConnection: close\r\nUser-Agent: AOSP-Wear/17\r\n\r\n",
                self.path, self.host
            );
            if socket.send_slice(request.as_bytes()).is_ok() {
                self.phase = Phase::Receiving;
            }
        }
        if !matches!(self.phase, Phase::Receiving) {
            return;
        }
        // Bound work per main-loop tick; do not starve the crown or radio scheduler.
        for _ in 0..4 {
            if !socket.can_recv() {
                break;
            }
            let mut chunk = [0; 512];
            if let Ok(n) = socket.recv_slice(&mut chunk) {
                if self.buffer.len() + n > LIMIT {
                    socket.abort();
                    self.phase = Phase::Idle;
                    weather::fail(7);
                    return;
                }
                self.buffer.extend_from_slice(&chunk[..n]);
            }
        }
        if socket.may_recv() || socket.can_recv() {
            return;
        }
        let result = parse_response(&mut self.buffer);
        let Ok(json) = result else {
            self.abort(sockets);
            weather::fail(7);
            return;
        };
        match self.job {
            Job::Location | Job::Search => {
                let object = if matches!(self.job, Job::Search) {
                    json.get("results").and_then(|v| v.get(0))
                } else if json.get("status").and_then(|v| v.as_str()) == Some("success") {
                    Some(&json)
                } else {
                    None
                };
                let Some(object) = object else {
                    self.abort(sockets);
                    weather::fail(5);
                    return;
                };
                let lat = number(
                    object,
                    if matches!(self.job, Job::Search) {
                        "latitude"
                    } else {
                        "lat"
                    },
                );
                let lon = number(
                    object,
                    if matches!(self.job, Job::Search) {
                        "longitude"
                    } else {
                        "lon"
                    },
                );
                let (Some(lat), Some(lon)) = (lat, lon) else {
                    self.abort(sockets);
                    weather::fail(7);
                    return;
                };
                if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                    self.abort(sockets);
                    weather::fail(7);
                    return;
                }
                self.info.lat = (lat * 1e6) as i32;
                self.info.lon = (lon * 1e6) as i32;
                weather::city(
                    &mut self.info,
                    object
                        .get(if matches!(self.job, Job::Search) {
                            "name"
                        } else {
                            "city"
                        })
                        .and_then(|v| v.as_str())
                        .unwrap_or("Выбранное место"),
                );
                self.forecast(iface, sockets);
            }
            Job::Forecast => {
                let Some(current) = json.get("current") else {
                    self.abort(sockets);
                    weather::fail(7);
                    return;
                };
                let fields = [
                    number(current, "temperature_2m"),
                    number(current, "apparent_temperature"),
                    number(current, "relative_humidity_2m"),
                    number(current, "wind_speed_10m"),
                    number(current, "weather_code"),
                ];
                if let [
                    Some(temp),
                    Some(feels),
                    Some(humidity),
                    Some(wind),
                    Some(code),
                ] = fields
                {
                    if !(-100.0..=80.0).contains(&temp)
                        || !(-150.0..=100.0).contains(&feels)
                        || !(0.0..=100.0).contains(&humidity)
                        || !(0.0..=200.0).contains(&wind)
                        || !(0.0..=99.0).contains(&code)
                    {
                        self.abort(sockets);
                        weather::fail(7);
                        return;
                    }
                    self.info.temp = (temp * 10.0) as i16;
                    self.info.feels = (feels * 10.0) as i16;
                    self.info.humidity = humidity as u8;
                    self.info.wind = (wind * 10.0) as u16;
                    self.info.code = code as u8;
                    weather::publish(self.info);
                    self.abort(sockets);
                } else {
                    self.abort(sockets);
                    weather::fail(7);
                }
            }
        }
    }
}
/// Извлекает числовое значение из ответа сервиса.
fn number(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key)?.as_f64().filter(|n| n.is_finite())
}
/// Проверяет ответ погодного сервиса и извлекает нужные данные.
fn parse_response(buffer: &mut Vec<u8>) -> Result<serde_json::Value, ()> {
    let split = buffer.windows(4).position(|w| w == b"\r\n\r\n").ok_or(())? + 4;
    let header = core::str::from_utf8(&buffer[..split]).map_err(|_| ())?;
    if !header.starts_with("HTTP/1.1 200 ") && !header.starts_with("HTTP/1.0 200 ") {
        return Err(());
    }
    let chunked = header.lines().any(|l| {
        l.to_ascii_lowercase().starts_with("transfer-encoding:")
            && l.to_ascii_lowercase().contains("chunked")
    });
    if chunked {
        let mut input = split;
        let mut output = 0;
        loop {
            let end = buffer[input..]
                .windows(2)
                .position(|w| w == b"\r\n")
                .ok_or(())?
                + input;
            let size = usize::from_str_radix(
                core::str::from_utf8(&buffer[input..end])
                    .map_err(|_| ())?
                    .split(';')
                    .next()
                    .ok_or(())?,
                16,
            )
            .map_err(|_| ())?;
            input = end + 2;
            if size == 0 {
                buffer.truncate(output);
                break;
            }
            let end = input
                .checked_add(size)
                .filter(|&n| n <= buffer.len().saturating_sub(2))
                .ok_or(())?;
            if &buffer[end..end + 2] != b"\r\n" {
                return Err(());
            }
            buffer.copy_within(input..end, output);
            output += size;
            input = end + 2;
        }
    } else {
        buffer.drain(..split);
    }
    serde_json::from_slice(buffer).map_err(|_| ())
}
