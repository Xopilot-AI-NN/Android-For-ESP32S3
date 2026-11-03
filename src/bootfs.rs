// Модуль прежнего загрузочного пути FAT.
extern crate alloc;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use embedded_hal::{delay::DelayNs, spi::SpiDevice};
use embedded_sdmmc::{Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use sha2::{Digest, Sha256};

use crate::config::MAX_SCRIPT_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Выбор загрузочного слота A/B компонента прежнего загрузочного пути FAT.
pub enum Slot { A, B }

impl Slot {
    /// Возвращает имя каталога для выбранного слота.
    pub fn dir(self) -> &'static str { match self { Slot::A => "SLOTA", Slot::B => "SLOTB" } }
    /// Возвращает противоположный слот A/B.
    pub fn other(self) -> Self { match self { Slot::A => Slot::B, Slot::B => Slot::A } }
    /// Возвращает строковое представление значения.
    pub fn as_str(self) -> &'static str { match self { Slot::A => "A", Slot::B => "B" } }
}

#[derive(Debug)]
/// Ошибки операций компонента прежнего загрузочного пути FAT.
pub enum BootFsError {
    Card,
    Filesystem,
    SlotMissing,
    ManifestMissing,
    ScriptMissing,
    ScriptTooLarge,
    Utf8,
    InvalidManifest,
    HashMismatch,
}

/// Загруженный слот, версия и входной скрипт компонента прежнего загрузочного пути FAT.
pub struct SystemImage {
    /// Выбранный загрузочный слот.
    pub slot: Slot,
    /// Прочитанная либо ожидаемая версия формата/системы.
    pub version: String,
    /// Входной скрипт или метаданные текущего элемента.
    pub entry: String,
    /// Проверенный текст скрипта для запуска.
    pub script: String,
}

#[derive(Clone, Copy)]
/// Фиксированное служебное время старого FAT-backend компонента прежнего загрузочного пути FAT.
struct FixedTime;
impl TimeSource for FixedTime {
    /// Возвращает фиксированную служебную дату старого FAT-backend, а не текущее RTC.
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 56,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

/// Загружает систему через соответствующий транспорт и проверяет её компоненты.
pub fn load_system<SPI, DELAY>(spi: SPI, delay: DELAY, preferred: Slot) -> Result<SystemImage, BootFsError>
where
    SPI: SpiDevice<u8>,
    DELAY: DelayNs,
{
    let card = SdCard::new(spi, delay);
    card.num_bytes().map_err(|_| BootFsError::Card)?;
    let manager = VolumeManager::new(card, FixedTime);
    let volume = manager.open_raw_volume(VolumeIdx(0)).map_err(|_| BootFsError::Filesystem)?;
    let root = manager.open_root_dir(volume).map_err(|_| BootFsError::Filesystem)?;

    let first = load_slot(&manager, root, preferred);
    let result = match first {
        Ok(image) => Ok(image),
        Err(_) => load_slot(&manager, root, preferred.other()),
    };

    let _ = manager.close_dir(root);
    let _ = manager.close_volume(volume);
    result
}

/// Проверяет манифест и загружает содержимое указанного слота.
fn load_slot<D, T, const DIRS: usize, const FILES: usize, const VOLUMES: usize>(
    manager: &VolumeManager<D, T, DIRS, FILES, VOLUMES>,
    root: embedded_sdmmc::RawDirectory,
    slot: Slot,
) -> Result<SystemImage, BootFsError>
where
    D: embedded_sdmmc::BlockDevice,
    T: TimeSource,
{
    let slot_dir = manager.open_dir(root, slot.dir()).map_err(|_| BootFsError::SlotMissing)?;

    let result = (|| {
        let manifest = read_file(manager, slot_dir, "MANIFEST.TXT", 2048)
            .map_err(|_| BootFsError::ManifestMissing)?;
        let manifest = core::str::from_utf8(&manifest).map_err(|_| BootFsError::Utf8)?;
        let (version, entry, expected_hash) = parse_manifest(manifest)?;

        let script_bytes = read_file(manager, slot_dir, &entry, MAX_SCRIPT_BYTES)
            .map_err(|_| BootFsError::ScriptMissing)?;
        let actual_hash = Sha256::digest(&script_bytes);
        if actual_hash[..] != expected_hash[..] {
            return Err(BootFsError::HashMismatch);
        }
        let script = String::from_utf8(script_bytes).map_err(|_| BootFsError::Utf8)?;
        Ok(SystemImage { slot, version, entry, script })
    })();

    let _ = manager.close_dir(slot_dir);
    result
}

/// Читает файл с ограничением размера.
fn read_file<D, T, const DIRS: usize, const FILES: usize, const VOLUMES: usize>(
    manager: &VolumeManager<D, T, DIRS, FILES, VOLUMES>,
    dir: embedded_sdmmc::RawDirectory,
    name: &str,
    max: usize,
) -> Result<Vec<u8>, BootFsError>
where
    D: embedded_sdmmc::BlockDevice,
    T: TimeSource,
{
    let file = manager.open_file_in_dir(dir, name, Mode::ReadOnly).map_err(|_| BootFsError::Filesystem)?;
    let result = (|| {
        let len = manager.file_length(file).map_err(|_| BootFsError::Filesystem)? as usize;
        if len > max { return Err(BootFsError::ScriptTooLarge); }

        let mut out = Vec::with_capacity(len);
        let mut chunk = [0u8; 256];
        loop {
            let n = manager.read(file, &mut chunk).map_err(|_| BootFsError::Filesystem)?;
            if n == 0 { break; }
            out.extend_from_slice(&chunk[..n]);
        }
        Ok(out)
    })();
    let _ = manager.close_file(file);
    result
}

/// Извлекает обязательные поля загрузочного манифеста.
fn parse_manifest(text: &str) -> Result<(String, String, [u8; 32]), BootFsError> {
    let mut magic_ok = false;
    let mut version: Option<String> = None;
    let mut entry: Option<String> = None;
    let mut sha256: Option<[u8; 32]> = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if line == "ZEPHYR_BOOT_V1" { magic_ok = true; continue; }
        if let Some(v) = line.strip_prefix("version=") { version = Some(v.trim().to_string()); }
        if let Some(v) = line.strip_prefix("entry=") { entry = Some(v.trim().to_ascii_uppercase()); }
        if let Some(v) = line.strip_prefix("sha256=") { sha256 = Some(parse_sha256(v.trim())?); }
    }

    let version = version.ok_or(BootFsError::InvalidManifest)?;
    let entry = entry.ok_or(BootFsError::InvalidManifest)?;
    let sha256 = sha256.ok_or(BootFsError::InvalidManifest)?;
    if !magic_ok || entry.len() > 12 || !entry.contains('.') || entry.contains('/') || entry.contains('\\') {
        return Err(BootFsError::InvalidManifest);
    }
    Ok((version, entry, sha256))
}

/// Проверяет и разбирает шестнадцатеричную запись SHA-256.
fn parse_sha256(s: &str) -> Result<[u8; 32], BootFsError> {
    if s.len() != 64 { return Err(BootFsError::InvalidManifest); }
    let bytes = s.as_bytes();
    let mut out = [0u8; 32];
    for i in 0..32 {
        let hi = hex(bytes[i * 2]).ok_or(BootFsError::InvalidManifest)?;
        let lo = hex(bytes[i * 2 + 1]).ok_or(BootFsError::InvalidManifest)?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

/// Преобразует шестнадцатеричный знак в число.
fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
