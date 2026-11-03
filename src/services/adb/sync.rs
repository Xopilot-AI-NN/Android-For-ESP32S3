// Модуль обмена файлами по ADB SYNC.
//! Bounded ADB SYNC v1: wallpaper uploads and read-only built-in PNG files.
use crate::wallpaper;
use alloc::{string::{String, ToString}, sync::Arc, vec::Vec};
use crate::file_store::{self as files, Op};
/// Преобразует путь /sdcard/Documents в проверенный путь WFSv1.
fn document_path(path: &str) -> Option<String> {
    let tail = path.strip_prefix("/sdcard/Documents")?;
    if tail.is_empty() || tail == "/" { Some("/".to_string()) }
    else if tail.starts_with('/') && files::path_valid(tail) { Some(tail.to_string()) }
    else { None }
}
/// Максимальный размер файла для данного формата, байт.
const MAX_FILE: usize = 320 * 1024;
/// Ограниченный разборщик состояния передачи ADB SYNC компонента обмена файлами по ADB SYNC.
pub struct Receiver {
    /// Входные данные или временно введённые символы.
    input: Vec<u8>,
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
    /// Сохраняет код ошибки текущей операции.
    fn fail(&mut self, reason: &str) {
        if let Some((token, _)) = self.store_job.take() { files::cancel(token); }
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
        if self.managed.is_some() && !files::external_access() { self.fail("watch locked"); return; }
        if let Some((token, kind)) = self.store_job {
            if let Some(reply) = files::take_reply(token) {
                self.store_job = None;
                match reply.result {
                    Err(e) => { self.fail(e.label()); return; }
                    Ok(Some(data)) if kind == 1 => self.download = Some((Arc::new(data), 0)),
                    Ok(_) if kind == 2 => self.output.extend_from_slice(b"OKAY\0\0\0\0"),
                    Ok(_) => self.managed_info(kind),
                }
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
                    if !files::external_access() { self.fail("watch locked"); return; }
                    let data = core::mem::take(&mut self.file);
                    self.queue_store(Op::Write(path, data, None), 2);
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
                    let path = core::str::from_utf8(payload).unwrap_or("").trim_end_matches('/');
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
                    if self.sending || self.download.is_some() || self.requested.is_some() {
                        self.fail("another transfer is active");
                        return;
                    }
                    if let Some(path) = document_path(core::str::from_utf8(payload).unwrap_or("")) {
                        self.managed = Some(path.clone());
                        self.input.drain(..size + 8);
                        self.queue_store(Op::Read(path), 1);
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
                        if !files::external_access() { self.fail("watch locked"); return; }
                        self.managed = Some(path);
                        self.file.clear(); self.sending = true;
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
                    let limit = if self.managed.is_some() { files::MAX_FILE } else { MAX_FILE };
                    if !self.sending || self.file.len() + size > limit {
                        self.fail("file exceeds size limit");
                        return;
                    }
                    self.file.extend_from_slice(payload);
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
        if !files::external_access() { self.fail("watch locked"); return; }
        match files::request(op) { Ok(id) => self.store_job = Some((id, kind)), Err(e) => self.fail(e.label()) }
    }
    /// Формирует STAT/LIST по согласованному каталогу документов.
    fn managed_info(&mut self, kind: u8) {
        if !files::external_access() { self.fail("watch locked"); return; }
        let Some(catalog) = files::catalog() else { self.queue_store(Op::Mount, kind); return; };
        let path = self.managed.as_deref().unwrap_or("/");
        let entry = catalog.iter().find(|e| e.kind != 0 && e.path == path);
        if kind == 4 {
            let mode: u32 = if path == "/" || entry.is_some_and(|e| e.kind == 2) { 0o040755 } else if entry.is_some() { 0o100644 } else { 0 };
            self.output.extend_from_slice(b"STAT"); self.output.extend_from_slice(&mode.to_le_bytes());
            self.output.extend_from_slice(&u32::from(entry.map_or(0, |e| e.size)).to_le_bytes());
            self.output.extend_from_slice(&0u32.to_le_bytes());
        } else {
            if path != "/" && !entry.is_some_and(|e| e.kind == 2) { self.fail("directory not found"); return; }
            for e in catalog.iter().filter(|e| e.kind != 0 && files::parent(&e.path) == path) {
                let name = files::basename(&e.path);
                self.output.extend_from_slice(b"DENT");
                self.output.extend_from_slice(&(if e.kind == 2 { 0o040755u32 } else { 0o100644u32 }).to_le_bytes());
                self.output.extend_from_slice(&u32::from(e.size).to_le_bytes()); self.output.extend_from_slice(&0u32.to_le_bytes());
                self.output.extend_from_slice(&(name.len() as u32).to_le_bytes()); self.output.extend_from_slice(name.as_bytes());
            }
            self.output.extend_from_slice(b"DONE"); self.output.extend_from_slice(&[0; 16]);
        }
    }
}

impl Drop for Receiver {
    /// Освобождает принадлежащие экземпляру ресурсы и незавершённые запросы.
    fn drop(&mut self) {
        if let Some((token, _)) = self.store_job.take() { files::cancel(token); }
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
        let mut b = command.to_vec(); b.extend_from_slice(&(payload.len() as u32).to_le_bytes()); b.extend_from_slice(payload); b
    }
    #[test]
    /// Проверяет передачу UTF-8-документа через ADB SYNC и остановку чтения при блокировке.
    fn document_sync_roundtrip_and_lock_gate() {
        files::reset_for_test(); files::set_external_access(true);
        let disk = crate::file_store::tests::fixture();
        let mut upload = Receiver::new();
        let send = command(b"SEND", b"/sdcard/Documents/test.txt,33206");
        for fragment in send.chunks(3) { upload.push(fragment); }
        upload.push(&command(b"DATA", "Текст\n123".as_bytes())); upload.push(b"DONE\0\0\0\0");
        assert!(files::service(&disk)); upload.poll(); assert_eq!(&upload.output, b"OKAY\0\0\0\0");
        drop(upload);
        let mut download = Receiver::new(); download.push(&command(b"RECV", b"/sdcard/Documents/test.txt"));
        files::service(&disk); download.poll();
        assert_eq!(&download.output[..4], b"DATA"); assert_eq!(&download.output[8..], "Текст\n123".as_bytes());
        files::set_external_access(false); download.output.clear(); download.poll();
        assert_eq!(&download.output[..4], b"FAIL");
        files::reset_for_test();
    }
    #[test]
    /// Проверяет отмену файловой операции при закрытии ADB-сеанса.
    fn dropping_transfer_releases_queued_job() {
        files::reset_for_test(); files::set_external_access(true);
        let mut s = Receiver::new(); s.push(&command(b"RECV", b"/sdcard/Documents/a.txt")); drop(s);
        let token = files::request(Op::Mount).unwrap(); files::cancel(token); files::reset_for_test();
    }
}
