// Модуль обмена файлами по ADB SYNC.
//! Bounded ADB SYNC v1: wallpaper uploads and read-only built-in PNG files.
use crate::file_store::{self as files, Op};
use crate::wallpaper;
use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
/// Преобразует путь /sdcard/Documents в проверенный путь WFSv1.
fn document_path(path: &str) -> Option<String> {
    let tail = path.strip_prefix("/sdcard")?;
    if ["/Wallpapers", "/Logs"].iter().any(|prefix| {
        tail == *prefix
            || tail
                .strip_prefix(prefix)
                .is_some_and(|p| p.starts_with('/'))
    }) {
        return None;
    }
    let tail = tail
        .strip_prefix("/Documents")
        .filter(|p| p.is_empty() || p.starts_with('/'))
        .unwrap_or(tail);
    if tail.is_empty() || tail == "/" {
        Some("/".to_string())
    } else if tail.starts_with('/') && files::path_valid(tail) {
        Some(tail.to_string())
    } else {
        None
    }
}
/// Максимальный размер файла для данного формата, байт.
const MAX_FILE: usize = 320 * 1024;
/// Ограниченный разборщик состояния передачи ADB SYNC компонента обмена файлами по ADB SYNC.
pub struct Receiver {
    /// Входные данные или временно введённые символы.
    input: Vec<u8>,
    streaming: bool,
    stream_bytes: usize,
    managed_read: Option<(String, usize)>,
    /// Путь открытого документа.
    file: Vec<u8>,
    /// Подготовленный результат или очередь исходящих данных.
    pub output: Vec<u8>,
    /// Состояние текущей выдачи файла клиенту.
    download: Option<(Arc<Vec<u8>>, usize)>,
    /// Ещё не выполненный запрос ресурса.
    requested: Option<u8>,
    /// Признак приёма файла от клиента.
    sending: bool,
    /// Признак ожидания завершения внешней операции.
    waiting: bool,
    /// Признак завершения текущей операции.
    pub finished: bool,
    /// Путь WFS для передачи документа вместо системных обоев.
    managed: Option<String>,
    /// Идентификатор и назначение ожидающей файловой операции.
    store_job: Option<(u32, u8)>,
}
impl Receiver {
    /// Создаёт экземпляр и подготавливает его начальные буферы и состояние.
    pub fn new() -> Self {
        Self {
            input: Vec::new(),
            streaming: false,
            stream_bytes: 0,
            managed_read: None,
            file: Vec::new(),
            output: Vec::new(),
            download: None,
            requested: None,
            sending: false,
            waiting: false,
            finished: false,
            managed: None,
            store_job: None,
        }
    }
    /// ADB ACK задерживается, пока DATA не сохранён владельцем диска.
    pub fn can_accept_input(&self) -> bool {
        self.store_job.is_none()
    }
    /// Сохраняет код ошибки текущей операции.
    fn fail(&mut self, reason: &str) {
        if let Some((token, _)) = self.store_job.take() {
            files::cancel(token);
        }
        self.download = None;
        self.requested = None;
        self.sending = false;
        self.waiting = false;
        self.output.clear();
        self.output.extend_from_slice(b"FAIL");
        self.output
            .extend_from_slice(&(reason.len() as u32).to_le_bytes());
        self.output.extend_from_slice(reason.as_bytes());
        self.finished = true;
    }
    /// Выполняет очередной неблокирующий шаг обработки и проверяет готовые результаты.
    pub fn poll(&mut self) {
        if self.finished {
            return;
        }
        if self.managed.is_some() && !files::external_access() {
            self.fail("watch locked");
            return;
        }
        if let Some((token, kind)) = self.store_job {
            if let Some(reply) = files::take_reply(token) {
                self.store_job = None;
                match reply.result {
                    Err(e) => {
                        self.fail(e.label());
                        return;
                    }
                    Ok(Some(data)) if kind == 9 => {
                        self.output.extend_from_slice(b"STAT");
                        self.output.extend_from_slice(&0o100400u32.to_le_bytes());
                        self.output
                            .extend_from_slice(&(data.len() as u32).to_le_bytes());
                        self.output.extend_from_slice(&0u32.to_le_bytes());
                    }
                    Ok(Some(data)) if kind == 1 => self.download = Some((Arc::new(data), 0)),
                    Ok(_) if kind == 6 => {
                        let data = core::mem::take(&mut self.file);
                        self.queue_store(Op::Upload(data), 7);
                    }
                    Ok(Some(data)) if kind == 5 => {
                        if data.is_empty() {
                            self.output.extend_from_slice(b"DONE\0\0\0\0");
                            self.managed_read = None;
                        } else {
                            self.output.extend_from_slice(b"DATA");
                            self.output
                                .extend_from_slice(&(data.len() as u32).to_le_bytes());
                            self.output.extend_from_slice(&data);
                            if let Some((_, offset)) = self.managed_read.as_mut() {
                                *offset += data.len();
                            }
                        }
                    }
                    Ok(_) if kind == 7 => {}
                    Ok(_) if kind == 8 => self.output.extend_from_slice(b"OKAY\0\0\0\0"),
                    Ok(_) if kind == 2 => self.output.extend_from_slice(b"OKAY\0\0\0\0"),
                    Ok(_) => self.managed_info(kind),
                }
            }
        }
        if self.store_job.is_none() && self.output.is_empty() {
            if let Some((path, offset)) = self.managed_read.clone() {
                self.queue_store(Op::ReadRange(path, offset, 512), 5);
            } else if !self.input.is_empty() {
                self.push(&[]);
            }
        }
        if let Some(index) = self.requested {
            if let Some(bytes) = wallpaper::builtin_png(index) {
                self.download = Some((bytes, 0));
                self.requested = None;
            } else if crate::ui_resources::failed(index) {
                self.fail("system wallpaper unavailable; update system image");
                return;
            }
        }
        // ACK-paced reads keep at most 520 bytes queued, even for large PNGs.
        if self.output.is_empty() {
            if let Some((bytes, offset)) = self.download.take() {
                if offset == bytes.len() {
                    self.output.extend_from_slice(b"DONE\0\0\0\0");
                    self.download = None;
                } else {
                    let end = (offset + 512).min(bytes.len());
                    self.output.extend_from_slice(b"DATA");
                    self.output
                        .extend_from_slice(&((end - offset) as u32).to_le_bytes());
                    self.output.extend_from_slice(&bytes[offset..end]);
                    self.download = Some((bytes, end));
                }
            }
        }
        if self.waiting {
            if let Some(ok) = wallpaper::upload_result() {
                self.waiting = false;
                if ok {
                    self.output.extend_from_slice(b"OKAY\0\0\0\0");
                } else {
                    self.fail("wallpaper could not be saved to userdata");
                }
            }
        }
    }
    /// Добавляет очередную порцию входных данных в разборщик.
    pub fn push(&mut self, bytes: &[u8]) {
        if self.finished || self.waiting || self.store_job.is_some() {
            return;
        }
        if self.input.len() + bytes.len() > 66 * 1024 {
            self.fail("SYNC packet too large");
            return;
        }
        self.input.extend_from_slice(bytes);
        loop {
            if self.input.len() < 8 {
                return;
            }
            let mut command = [0; 4];
            command.copy_from_slice(&self.input[..4]);
            let size = u32::from_le_bytes(self.input[4..8].try_into().unwrap_or([0; 4])) as usize;
            if &command == b"DONE" {
                self.input.drain(..8);
                if !self.sending {
                    self.fail("SEND required before DONE");
                    return;
                }
                self.sending = false;
                if let Some(path) = self.managed.clone() {
                    if !files::external_access() {
                        self.fail("watch locked");
                        return;
                    }
                    if self.streaming {
                        self.queue_store(Op::FinishUpload, 8);
                    } else {
                        let data = core::mem::take(&mut self.file);
                        self.queue_store(Op::Write(path, data, None), 2);
                    }
                    return;
                }
                match wallpaper::queue_png(&self.file) {
                    Ok(()) => {
                        self.waiting = true;
                        self.file.clear();
                    }
                    Err(reason) => self.fail(reason),
                }
                return;
            }
            if &command == b"QUIT" {
                self.finished = true;
                self.input.clear();
                return;
            }
            let limit = if &command == b"DATA" { 65536 } else { 1024 };
            if size > limit {
                self.fail("invalid SYNC length");
                return;
            }
            if self.input.len() < size + 8 {
                return;
            }
            let payload = &self.input[8..8 + size];
            match &command {
                b"LIST" => {
                    let path = core::str::from_utf8(payload)
                        .unwrap_or("")
                        .trim_end_matches('/');
                    if let Some(path) = document_path(path) {
                        self.managed = Some(path);
                        self.input.drain(..size + 8);
                        self.managed_info(3);
                        return;
                    }
                    if self.output.len() + usize::from(wallpaper::builtin_count()) * 32 + 20 > 1024
                    {
                        self.fail("read pending SYNC response first");
                        return;
                    }
                    let path = core::str::from_utf8(payload)
                        .unwrap_or("")
                        .trim_end_matches('/');
                    if path != "/sdcard/Wallpapers" {
                        self.fail("directory not available");
                        return;
                    }
                    for index in 1..=wallpaper::builtin_count() {
                        use core::fmt::Write;
                        let mut name = heapless::String::<12>::new();
                        let _ = write!(name, "{}.png", index);
                        self.output.extend_from_slice(b"DENT");
                        self.output.extend_from_slice(&0o100444_u32.to_le_bytes());
                        self.output.extend_from_slice(
                            &(wallpaper::builtin_len(index).unwrap_or(0) as u32).to_le_bytes(),
                        );
                        self.output.extend_from_slice(&0_u32.to_le_bytes());
                        self.output
                            .extend_from_slice(&(name.len() as u32).to_le_bytes());
                        self.output.extend_from_slice(name.as_bytes());
                    }
                    self.output.extend_from_slice(b"DONE");
                    self.output.extend_from_slice(&[0; 16]);
                }
                b"STAT" => {
                    if matches!(
                        core::str::from_utf8(payload).unwrap_or(""),
                        "/data/log/watch.log" | "/sdcard/Logs/watch.log"
                    ) {
                        // STAT reports exported text length, not the reserved raw-ring capacity.
                        self.managed = Some("/data/log/watch.log".to_string());
                        self.input.drain(..size + 8);
                        self.queue_store(Op::ReadLog, 9);
                        return;
                    }
                    if let Some(path) = document_path(core::str::from_utf8(payload).unwrap_or("")) {
                        self.managed = Some(path);
                        self.input.drain(..size + 8);
                        self.managed_info(4);
                        return;
                    }
                    if self.output.len() > 1008 {
                        self.fail("read pending SYNC response first");
                        return;
                    }
                    self.output.extend_from_slice(b"STAT");
                    let bytes =
                        wallpaper::builtin_file_index(core::str::from_utf8(payload).unwrap_or(""))
                            .and_then(wallpaper::builtin_len);
                    let dir = core::str::from_utf8(payload)
                        .unwrap_or("")
                        .trim_end_matches('/')
                        == "/sdcard/Wallpapers";
                    self.output.extend_from_slice(
                        &if dir {
                            0o040755_u32
                        } else if bytes.is_some() {
                            0o100444_u32
                        } else {
                            0
                        }
                        .to_le_bytes(),
                    );
                    self.output
                        .extend_from_slice(&(bytes.unwrap_or(0) as u32).to_le_bytes());
                    self.output.extend_from_slice(&0_u32.to_le_bytes());
                }
                b"RECV" => {
                    let log = core::str::from_utf8(payload).unwrap_or("");
                    if matches!(log, "/data/log/watch.log" | "/sdcard/Logs/watch.log") {
                        self.managed = Some(log.to_string());
                        self.input.drain(..size + 8);
                        self.queue_store(Op::ReadLog, 1);
                        return;
                    }
                    if self.sending || self.download.is_some() || self.requested.is_some() {
                        self.fail("another transfer is active");
                        return;
                    }
                    if let Some(path) = document_path(core::str::from_utf8(payload).unwrap_or("")) {
                        self.managed = Some(path.clone());
                        self.input.drain(..size + 8);
                        self.managed_read = Some((path.clone(), 0));
                        self.queue_store(Op::ReadRange(path, 0, 512), 5);
                        return;
                    }
                    if let Some(index) =
                        wallpaper::builtin_file_index(core::str::from_utf8(payload).unwrap_or(""))
                    {
                        self.requested = Some(index);
                    } else {
                        self.fail("read-only files: /sdcard/Wallpapers/1.png through 16.png");
                        return;
                    }
                }
                b"SEND" => {
                    if self.download.is_some() || self.requested.is_some() {
                        self.fail("download in progress");
                        return;
                    }
                    let value = core::str::from_utf8(payload).unwrap_or("");
                    let path = value
                        .rsplit_once(',')
                        .map(|(path, _)| path)
                        .unwrap_or(value);
                    if let Some(path) = document_path(path) {
                        if !files::external_access() {
                            self.fail("watch locked");
                            return;
                        }
                        self.managed = Some(path);
                        self.file.clear();
                        self.sending = true;
                        self.streaming = false;
                        self.stream_bytes = 0;
                        self.input.drain(..size + 8);
                        continue;
                    }
                    self.managed = None;
                    if wallpaper::builtin_file_index(path).is_some() {
                        self.fail("built-in wallpaper is read-only; upload current.png");
                        return;
                    }
                    if !path.starts_with("/sdcard/Wallpapers/")
                        || !path.ends_with(".png")
                        || path.contains("..")
                    {
                        self.fail("use /sdcard/Wallpapers/name.png (240x280 PNG)");
                        return;
                    }
                    self.file.clear();
                    self.sending = true;
                }
                b"DATA" => {
                    if !self.sending {
                        self.fail("SEND required");
                        return;
                    }
                    if let Some(path) = self.managed.clone() {
                        if self.stream_bytes + size > files::MAX_BLOB {
                            self.fail("file exceeds 16 MiB limit");
                            return;
                        }
                        self.stream_bytes += size;
                        if self.streaming {
                            let data = payload.to_vec();
                            self.input.drain(..size + 8);
                            self.queue_store(Op::Upload(data), 7);
                            return;
                        }
                        self.file.extend_from_slice(payload);
                        if self.file.len() > files::MAX_FILE {
                            self.streaming = true;
                            self.input.drain(..size + 8);
                            self.queue_store(Op::BeginUpload(path), 6);
                            return;
                        }
                    } else {
                        if self.file.len() + size > MAX_FILE {
                            self.fail("wallpaper exceeds 320 KiB");
                            return;
                        }
                        self.file.extend_from_slice(payload);
                    }
                }
                _ => {
                    self.fail("only wallpaper STAT/SEND/DATA/DONE are supported");
                    return;
                }
            }
            self.input.drain(..size + 8);
        }
    }
    /// Ставит файловую операцию ADB в очередь дискового сервиса.
    fn queue_store(&mut self, op: Op, kind: u8) {
        if !files::external_access() {
            self.fail("watch locked");
            return;
        }
        match files::request(op) {
            Ok(id) => self.store_job = Some((id, kind)),
            Err(e) => self.fail(e.label()),
        }
    }
    /// Формирует STAT/LIST по согласованному каталогу документов.
    fn managed_info(&mut self, kind: u8) {
        if !files::external_access() {
            self.fail("watch locked");
            return;
        }
        let Some(catalog) = files::catalog() else {
            self.queue_store(Op::Mount, kind);
            return;
        };
        let path = self.managed.as_deref().unwrap_or("/");
        let entry = catalog.iter().find(|e| e.kind != 0 && e.path == path);
        if kind == 4 {
            let mode: u32 = if path == "/" || entry.is_some_and(|e| e.kind == 2) {
                0o040755
            } else if entry.is_some() {
                0o100644
            } else {
                0
            };
            self.output.extend_from_slice(b"STAT");
            self.output.extend_from_slice(&mode.to_le_bytes());
            self.output
                .extend_from_slice(&u32::from(entry.map_or(0, |e| e.size)).to_le_bytes());
            self.output.extend_from_slice(&0u32.to_le_bytes());
        } else {
            if path != "/" && !entry.is_some_and(|e| e.kind == 2) {
                self.fail("directory not found");
                return;
            }
            for e in catalog
                .iter()
                .filter(|e| e.kind != 0 && files::parent(&e.path) == path)
            {
                let name = files::basename(&e.path);
                self.output.extend_from_slice(b"DENT");
                self.output.extend_from_slice(
                    &(if e.kind == 2 {
                        0o040755u32
                    } else {
                        0o100644u32
                    })
                    .to_le_bytes(),
                );
                self.output
                    .extend_from_slice(&u32::from(e.size).to_le_bytes());
                self.output.extend_from_slice(&0u32.to_le_bytes());
                self.output
                    .extend_from_slice(&(name.len() as u32).to_le_bytes());
                self.output.extend_from_slice(name.as_bytes());
            }
            self.output.extend_from_slice(b"DONE");
            self.output.extend_from_slice(&[0; 16]);
        }
    }
}

impl Drop for Receiver {
    /// Освобождает принадлежащие экземпляру ресурсы и незавершённые запросы.
    fn drop(&mut self) {
        if let Some((token, _)) = self.store_job.take() {
            files::cancel(token);
        }
        if self.waiting {
            wallpaper::cancel_upload();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Передаёт команду контроллеру и проверяет транспортный результат.
    fn command(command: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut b = command.to_vec();
        b.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        b.extend_from_slice(payload);
        b
    }
    #[test]
    /// Проверяет передачу UTF-8-документа через ADB SYNC и остановку чтения при блокировке.
    fn document_sync_roundtrip_and_lock_gate() {
        files::reset_for_test();
        files::set_external_access(true);
        let disk = crate::file_store::tests::fixture();
        let mut upload = Receiver::new();
        let send = command(b"SEND", b"/sdcard/Documents/test.txt,33206");
        for fragment in send.chunks(3) {
            upload.push(fragment);
        }
        upload.push(&command(b"DATA", "Текст\n123".as_bytes()));
        upload.push(b"DONE\0\0\0\0");
        assert!(files::service(&disk));
        upload.poll();
        assert_eq!(&upload.output, b"OKAY\0\0\0\0");
        drop(upload);
        let mut download = Receiver::new();
        download.push(&command(b"RECV", b"/sdcard/Documents/test.txt"));
        files::service(&disk);
        download.poll();
        assert_eq!(&download.output[..4], b"DATA");
        assert_eq!(&download.output[8..], "Текст\n123".as_bytes());
        files::set_external_access(false);
        download.output.clear();
        download.poll();
        assert_eq!(&download.output[..4], b"FAIL");
        files::reset_for_test();
    }
    #[test]
    /// Проверяет отмену файловой операции при закрытии ADB-сеанса.
    fn dropping_transfer_releases_queued_job() {
        files::reset_for_test();
        files::set_external_access(true);
        let mut s = Receiver::new();
        s.push(&command(b"RECV", b"/sdcard/Documents/a.txt"));
        drop(s);
        let token = files::request(Op::Mount).unwrap();
        files::cancel(token);
        files::reset_for_test();
    }
    #[test]
    fn phone_sized_upload_is_disk_paced_and_downloads_without_truncation() {
        files::reset_for_test();
        files::set_external_access(true);
        let disk = files::blob_fixture();
        let mut upload = Receiver::new();
        upload.push(&command(b"SEND", b"/sdcard/photo.jpg,33206"));
        let chunk = [0x5a; 65536];
        for _ in 0..80 {
            assert!(upload.can_accept_input());
            upload.push(&command(b"DATA", &chunk));
            while !upload.can_accept_input() {
                assert!(files::service(&disk));
                upload.poll();
            }
            assert!(!upload.finished);
            assert!(upload.output.is_empty());
        }
        upload.push(b"DONE\0\0\0\0");
        while !upload.can_accept_input() {
            assert!(files::service(&disk));
            upload.poll();
        }
        assert_eq!(&upload.output, b"OKAY\0\0\0\0");
        drop(upload);
        let mut download = Receiver::new();
        download.push(&command(b"RECV", b"/sdcard/Documents/photo.jpg"));
        let mut count = 0;
        loop {
            files::service(&disk);
            download.poll();
            assert!(!download.finished);
            if download.output.is_empty() {
                continue;
            }
            if &download.output[..4] == b"DONE" {
                break;
            }
            assert_eq!(&download.output[..4], b"DATA");
            assert!(download.output.len() <= 520);
            assert!(download.output[8..].iter().all(|b| *b == 0x5a));
            count += download.output.len() - 8;
            download.output.clear();
        }
        assert_eq!(count, 5 * 1024 * 1024);
        drop(download);
        files::reset_for_test();
    }
}
