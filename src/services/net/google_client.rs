//! One cooperatively polled TLS exchange: TCP and UI keep running while the future waits.
use super::*;
use crate::{google_gateway as account, google_tls::exchange};
use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{
    cell::RefCell,
    convert::Infallible,
    future::{Future, poll_fn},
    pin::Pin,
    task::{Context, Poll, Waker},
};
use critical_section::Mutex;
use mbedtls_rs::{
    Tls, io,
    sys::{
        hook::wall_clock::{MbedtlsWallClock, hook_wall_clock},
        tm,
    },
};
use smoltcp::{socket::dns, wire::DnsQueryType};
const PIPE_LIMIT: usize = 8192;
struct Pipe {
    rx: VecDeque<u8>,
    tx: VecDeque<u8>,
    eof: bool,
    failed: bool,
}
static PIPE: Mutex<RefCell<Pipe>> = Mutex::new(RefCell::new(Pipe {
    rx: VecDeque::new(),
    tx: VecDeque::new(),
    eof: false,
    failed: false,
}));
struct Stream;
impl io::ErrorType for Stream {
    type Error = io::ErrorKind;
}
impl io::Read for Stream {
    async fn read(&mut self, out: &mut [u8]) -> Result<usize, Self::Error> {
        if out.is_empty() {
            return Ok(0);
        }
        poll_fn(|_| {
            critical_section::with(|cs| {
                let mut p = PIPE.borrow_ref_mut(cs);
                if p.failed {
                    return Poll::Ready(Err(io::ErrorKind::ConnectionAborted));
                }
                let n = out.len().min(p.rx.len());
                for b in &mut out[..n] {
                    *b = p.rx.pop_front().unwrap();
                }
                if n > 0 || p.eof {
                    Poll::Ready(Ok(n))
                } else {
                    Poll::Pending
                }
            })
        })
        .await
    }
}
impl io::Write for Stream {
    async fn write(&mut self, data: &[u8]) -> Result<usize, Self::Error> {
        poll_fn(|_| {
            critical_section::with(|cs| {
                let mut p = PIPE.borrow_ref_mut(cs);
                if p.failed {
                    return Poll::Ready(Err(io::ErrorKind::ConnectionAborted));
                }
                let n = data.len().min(PIPE_LIMIT - p.tx.len());
                p.tx.extend(data[..n].iter().copied());
                if n > 0 || data.is_empty() {
                    Poll::Ready(Ok(n))
                } else {
                    Poll::Pending
                }
            })
        })
        .await
    }
    async fn flush(&mut self) -> Result<(), Self::Error> {
        poll_fn(|_| {
            critical_section::with(|cs| {
                let p = PIPE.borrow_ref(cs);
                if p.failed {
                    Poll::Ready(Err(io::ErrorKind::ConnectionAborted))
                } else if p.tx.is_empty() {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
        })
        .await
    }
}
/// esp-radio has enabled the RF entropy source before any TLS operation starts.
struct RadioRng;
impl tls_rand::TryRng for RadioRng {
    type Error = Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        Ok(esp_hal::rng::Rng::new().random())
    }
    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        Ok(u64::from(self.try_next_u32()?) | (u64::from(self.try_next_u32()?) << 32))
    }
    fn try_fill_bytes(&mut self, out: &mut [u8]) -> Result<(), Infallible> {
        esp_hal::rng::Rng::new().read(out);
        Ok(())
    }
}
impl tls_rand::TryCryptoRng for RadioRng {}
struct Clock;
static CLOCK: Clock = Clock;
impl MbedtlsWallClock for Clock {
    fn instant(&self) -> Option<tm> {
        let seconds = i64::from(crate::rtc::unix_seconds()?);
        let days = seconds.div_euclid(86400);
        let z = days + 719468;
        let era = z.div_euclid(146097);
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = mp + if mp < 10 { 3 } else { -9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        // C tm contains integer fields/pointers: zero initializes unused timezone fields safely.
        let mut value: tm = unsafe { core::mem::zeroed() };
        value.tm_year = (year - 1900) as _;
        value.tm_mon = (month - 1) as _;
        value.tm_mday = day as _;
        value.tm_hour = (seconds / 3600 % 24) as _;
        value.tm_min = (seconds / 60 % 60) as _;
        value.tm_sec = (seconds % 60) as _;
        value.tm_wday = ((days + 4) % 7) as _;
        Some(value)
    }
}
type Exchange = Pin<Box<dyn Future<Output = Result<(u16, Vec<u8>), &'static str>>>>;
enum Phase {
    Idle,
    Dns(dns::QueryHandle),
    Connecting,
    Exchanging,
}
pub struct GoogleClient {
    dns: SocketHandle,
    tcp: SocketHandle,
    tls: &'static Tls<'static>,
    phase: Phase,
    request: Option<account::Request>,
    future: Option<Exchange>,
    deadline: u64,
}
impl GoogleClient {
    pub fn new(sockets: &mut SocketSet<'static>) -> Option<Self> {
        static RNG: StaticCell<RadioRng> = StaticCell::new();
        static TLS: StaticCell<Tls<'static>> = StaticCell::new();
        // Installed once, before parsing/validating any certificate. Missing RTC fails closed.
        unsafe {
            hook_wall_clock(Some(&CLOCK));
        }
        let tls = TLS.init(Tls::new(RNG.init(RadioRng)).ok()?);
        let dns = sockets.add(dns::Socket::new(&[IpAddress::v4(1, 1, 1, 1)], Vec::new()));
        // TCP staging does not need DMA memory; reserve internal RAM for Wi-Fi/BLE.
        let mut rx = super::weather_client::external(8192)?;
        rx.resize(8192, 0);
        let mut tx = super::weather_client::external(8192)?;
        tx.resize(8192, 0);
        let mut socket = tcp::Socket::new(tcp::SocketBuffer::new(rx), tcp::SocketBuffer::new(tx));
        socket.set_timeout(Some(SmolDuration::from_secs(20)));
        socket.set_nagle_enabled(false);
        let tcp = sockets.add(socket);
        Some(Self {
            dns,
            tcp,
            tls,
            phase: Phase::Idle,
            request: None,
            future: None,
            deadline: 0,
        })
    }
    pub fn servers(&mut self, sockets: &mut SocketSet<'static>, servers: &[IpAddress]) {
        if !servers.is_empty() {
            sockets
                .get_mut::<dns::Socket>(self.dns)
                .update_servers(servers);
        }
    }
    fn stop(&mut self, sockets: &mut SocketSet<'static>) {
        if let Phase::Dns(q) = self.phase {
            sockets.get_mut::<dns::Socket>(self.dns).cancel_query(q);
        }
        self.future = None;
        self.request = None;
        self.phase = Phase::Idle;
        sockets.get_mut::<tcp::Socket>(self.tcp).abort();
        critical_section::with(|cs| {
            let mut p = PIPE.borrow_ref_mut(cs);
            p.rx.clear();
            p.tx.clear();
            p.failed = false;
            p.eof = false;
        });
    }
    fn error(&mut self, sockets: &mut SocketSet<'static>, message: &'static str) {
        if let Some(r) = &self.request {
            account::fail(r.id, message);
        }
        self.stop(sockets);
    }
    pub fn poll(&mut self, iface: &mut Interface, sockets: &mut SocketSet<'static>, online: bool) {
        account::network(online);
        if self
            .request
            .as_ref()
            .is_some_and(|r| !account::current(r.id))
        {
            self.stop(sockets);
        }
        if !online {
            if self.request.is_some() {
                self.error(sockets, "Нет Wi-Fi · кэш без обновления");
            }
            return;
        }
        if self.request.is_some() && now_ms() >= self.deadline {
            self.error(sockets, "Gateway не ответил вовремя");
            return;
        }
        if matches!(self.phase, Phase::Idle) {
            if let Some(r) = account::take_request() {
                if crate::rtc::unix_seconds().is_none() {
                    account::fail(r.id, "Сначала синхронизируйте время");
                    return;
                }
                self.deadline = now_ms() + 25000;
                match sockets.get_mut::<dns::Socket>(self.dns).start_query(
                    iface.context(),
                    &r.config.host,
                    DnsQueryType::A,
                ) {
                    Ok(q) => {
                        self.phase = Phase::Dns(q);
                        self.request = Some(r);
                    }
                    Err(_) => account::fail(r.id, "Ошибка DNS Gateway"),
                }
            }
        }
        match self.phase {
            Phase::Dns(q) => match sockets.get_mut::<dns::Socket>(self.dns).get_query_result(q) {
                Ok(addresses) => {
                    let Some(address) = addresses.first() else {
                        self.error(sockets, "Адрес Gateway не найден");
                        return;
                    };
                    let port = self.request.as_ref().unwrap().config.port;
                    match sockets.get_mut::<tcp::Socket>(self.tcp).connect(
                        iface.context(),
                        (*address, port),
                        50001,
                    ) {
                        Ok(()) => self.phase = Phase::Connecting,
                        Err(_) => self.error(sockets, "Не удалось подключиться"),
                    }
                }
                Err(dns::GetQueryResultError::Pending) => {}
                Err(_) => self.error(sockets, "Ошибка DNS Gateway"),
            },
            Phase::Connecting => {
                if sockets.get::<tcp::Socket>(self.tcp).may_send() {
                    let r = self.request.as_ref().unwrap();
                    self.future = Some(Box::pin(exchange(
                        self.tls,
                        Stream,
                        r.config.clone(),
                        r.method,
                        r.path.clone(),
                        r.body.clone(),
                        r.authorization.clone(),
                    )));
                    self.phase = Phase::Exchanging;
                }
            }
            Phase::Exchanging => {
                let socket = sockets.get_mut::<tcp::Socket>(self.tcp);
                critical_section::with(|cs| {
                    let mut p = PIPE.borrow_ref_mut(cs);
                    if socket.can_recv() {
                        let _ = socket.recv(|data| {
                            let n = data.len().min(PIPE_LIMIT - p.rx.len()).min(2048);
                            p.rx.extend(data[..n].iter().copied());
                            (n, ())
                        });
                    }
                    if socket.can_send() && !p.tx.is_empty() {
                        let _ = socket.send(|out| {
                            let n = out.len().min(p.tx.len()).min(2048);
                            for b in &mut out[..n] {
                                *b = p.tx.pop_front().unwrap();
                            }
                            (n, ())
                        });
                    }
                    if !socket.may_recv() {
                        p.eof = true;
                    }
                    if !socket.is_active() && p.rx.is_empty() {
                        p.failed = true;
                    }
                });
                let mut context = Context::from_waker(Waker::noop());
                if let Some(future) = self.future.as_mut() {
                    if let Poll::Ready(result) = future.as_mut().poll(&mut context) {
                        let id = self.request.as_ref().unwrap().id;
                        match result {
                            Ok((status, body)) => account::complete(id, status, &body),
                            Err(error) => account::fail(id, error),
                        }
                        self.stop(sockets);
                    }
                }
            }
            Phase::Idle => {}
        }
    }
}
