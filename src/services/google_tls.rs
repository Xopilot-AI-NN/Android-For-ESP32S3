//! TLS handshake and HTTP exchange shared by hardware and real local-TLS regression tests.
use crate::{google_gateway as account, google_http};
use alloc::{ffi::CString, string::String, sync::Arc, vec::Vec};
use mbedtls_rs::{
    AuthMode, Certificate, ClientSessionConfig, Session, SessionConfig, Tls, TlsVersion, X509, io,
};

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use core::{
        convert::Infallible,
        future::Future,
        task::{Context, Poll, Waker},
    };
    use mbedtls_rs::sys::{
        hook::wall_clock::{MbedtlsWallClock, hook_wall_clock},
        tm,
    };
    use std::{
        fs,
        io::{BufRead, BufReader, Read as _, Write as _},
        net::TcpStream,
        process::{Child, Command, Stdio},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    struct Rng;
    impl tls_rand::TryRng for Rng {
        type Error = Infallible;
        fn try_next_u32(&mut self) -> Result<u32, Infallible> {
            let mut b = [0; 4];
            self.try_fill_bytes(&mut b)?;
            Ok(u32::from_ne_bytes(b))
        }
        fn try_next_u64(&mut self) -> Result<u64, Infallible> {
            let mut b = [0; 8];
            self.try_fill_bytes(&mut b)?;
            Ok(u64::from_ne_bytes(b))
        }
        fn try_fill_bytes(&mut self, b: &mut [u8]) -> Result<(), Infallible> {
            fs::File::open("/dev/urandom")
                .unwrap()
                .read_exact(b)
                .unwrap();
            Ok(())
        }
    }
    impl tls_rand::TryCryptoRng for Rng {}
    struct Clock(bool);
    static PRESENT: Clock = Clock(false);
    static PAST: Clock = Clock(true);
    impl MbedtlsWallClock for Clock {
        fn instant(&self) -> Option<tm> {
            let s = if self.0 {
                946684800
            } else {
                SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64
            };
            let z = s / 86400 + 719468;
            let era = z / 146097;
            let doe = z - era * 146097;
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let m = mp + if mp < 10 { 3 } else { -9 };
            let y = yoe + era * 400 + i64::from(m <= 2);
            let mut t: tm = unsafe { core::mem::zeroed() };
            t.tm_year = (y - 1900) as _;
            t.tm_mon = (m - 1) as _;
            t.tm_mday = (doy - (153 * mp + 2) / 5 + 1) as _;
            t.tm_hour = (s / 3600 % 24) as _;
            t.tm_min = (s / 60 % 60) as _;
            t.tm_sec = (s % 60) as _;
            Some(t)
        }
    }
    struct Stream(TcpStream);
    impl io::ErrorType for Stream {
        type Error = io::ErrorKind;
    }
    impl io::Read for Stream {
        async fn read(&mut self, b: &mut [u8]) -> Result<usize, Self::Error> {
            self.0.read(b).map_err(|_| io::ErrorKind::ConnectionAborted)
        }
    }
    impl io::Write for Stream {
        async fn write(&mut self, b: &[u8]) -> Result<usize, Self::Error> {
            self.0
                .write(b)
                .map_err(|_| io::ErrorKind::ConnectionAborted)
        }
        async fn flush(&mut self) -> Result<(), Self::Error> {
            self.0.flush().map_err(|_| io::ErrorKind::ConnectionAborted)
        }
    }
    struct Fixture {
        directory: std::path::PathBuf,
        child: Child,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = fs::remove_dir_all(&self.directory);
        }
    }
    fn certificate(dir: &std::path::Path, name: &str) {
        let output = Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=localhost",
                "-addext",
                "subjectAltName=DNS:localhost",
                "-keyout",
            ])
            .arg(dir.join(format!("{name}.key")))
            .arg("-out")
            .arg(dir.join(format!("{name}.pem")))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "openssl fixture certificate failed"
        );
    }
    fn run(
        tls: &'static Tls<'static>,
        config: account::Configuration,
    ) -> Result<(u16, Vec<u8>), &'static str> {
        let stream = TcpStream::connect(("127.0.0.1", config.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut future = core::pin::pin!(exchange(
            tls,
            Stream(stream),
            Arc::new(config),
            "GET",
            "/health".into(),
            Vec::new(),
            format!("Device {}", "x".repeat(43))
        ));
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(result) => return result,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }
    #[test]
    fn actual_tls_accepts_trusted_name_and_rejects_wrong_ca_hostname_unset_and_wrong_time() {
        use std::os::unix::fs::DirBuilderExt;
        let dir = std::env::temp_dir().join(format!(
            "watch-tls-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        certificate(&dir, "server");
        certificate(&dir, "other");
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../GoogleAuthGateway/tests/tls-fixture.mjs");
        let mut child = Command::new("node")
            .arg(script)
            .arg(dir.join("server.pem"))
            .arg(dir.join("server.key"))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port = line.trim().parse::<u16>().unwrap();
        let fixture = Fixture {
            directory: dir,
            child,
        };
        let config = |host: &str, ca: &str| account::Configuration {
            base: format!("https://{host}:{port}"),
            host: host.into(),
            port,
            device: "d".repeat(22),
            credential: "k".repeat(43),
            ca: fs::read_to_string(fixture.directory.join(format!("{ca}.pem"))).unwrap(),
            generation: 1,
        };
        unsafe {
            hook_wall_clock(Some(&PRESENT));
        }
        let tls = Box::leak(Box::new(Tls::new(Box::leak(Box::new(Rng))).unwrap()));
        let response = run(tls, config("localhost", "server")).unwrap();
        assert_eq!(response.0, 200);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&response.1).unwrap()["ok"],
            true
        );
        assert!(run(tls, config("wrong.example.invalid", "server")).is_err());
        assert!(run(tls, config("localhost", "other")).is_err());
        unsafe {
            hook_wall_clock(None);
        }
        assert!(run(tls, config("localhost", "server")).is_err());
        unsafe {
            hook_wall_clock(Some(&PAST));
        }
        assert!(run(tls, config("localhost", "server")).is_err());
        unsafe {
            hook_wall_clock(Some(&PRESENT));
        }
    }
}
pub(crate) async fn exchange<T: io::Read + io::Write>(
    tls: &'static Tls<'static>,
    stream: T,
    config: Arc<account::Configuration>,
    method: &'static str,
    path: String,
    body: Vec<u8>,
    authorization: String,
) -> Result<(u16, Vec<u8>), &'static str> {
    let name = CString::new(config.host.as_str()).map_err(|_| "Неверное имя Gateway")?;
    let ca = CString::new(config.ca.as_str()).map_err(|_| "Неверный CA")?;
    let certificate = Certificate::new(X509::PEM(&ca)).map_err(|_| "Не удалось прочитать CA")?;
    let cfg = SessionConfig::Client(ClientSessionConfig {
        ca_chain: Some(certificate),
        server_name: Some(&name),
        auth_mode: AuthMode::Required,
        min_version: TlsVersion::Tls1_2,
        ..ClientSessionConfig::new()
    });
    let mut session =
        Session::new(tls.reference(), stream, &cfg).map_err(|_| "Ошибка создания TLS")?;
    if let Err(error) = session.connect().await {
        // The error enum contains only library codes/IO kinds, never URLs or credentials.
        crate::watch_println!(
            "GoogleTLS: handshake rejected {:?}, verify={:08x}",
            error,
            session.tls_verification_details()
        );
        return Err("TLS: ошибка соединения или проверки сертификата");
    }
    if session.tls_verification_details() != 0 {
        return Err("TLS: сертификат отклонён");
    }
    let authority = config
        .base
        .strip_prefix("https://")
        .ok_or("Требуется HTTPS")?;
    let mut request=alloc::format!("{method} {path} HTTP/1.1\r\nHost: {authority}\r\nAuthorization: {authorization}\r\nX-Device-ID: {}\r\nAccept: application/json\r\nAccept-Encoding: identity\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",config.device,body.len()).into_bytes();
    request.extend_from_slice(&body);
    let mut at = 0;
    while at < request.len() {
        let n = session
            .write(&request[at..])
            .await
            .map_err(|_| "Ошибка отправки HTTPS")?;
        if n == 0 {
            return Err("HTTPS соединение закрыто");
        }
        at += n;
    }
    request.fill(0);
    session.flush().await.map_err(|_| "Ошибка отправки HTTPS")?;
    let mut raw = Vec::new();
    let mut buffer = [0; 1024];
    loop {
        let n = session
            .read(&mut buffer)
            .await
            .map_err(|_| "Ошибка чтения HTTPS")?;
        if n == 0 {
            return Err("Неполный ответ Gateway");
        }
        raw.extend_from_slice(&buffer[..n]);
        if let Some(result) = google_http::response(&raw)? {
            return Ok(result);
        }
    }
}
