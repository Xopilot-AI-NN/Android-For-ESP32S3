// Модуль транзакционного хранилища документов WFSv1.
//! Small transactional filesystem in an explicitly reserved raw-userdata region.
//! Metadata and each file have two copies. Data is verified before publishing it.
use crate::gpt::{self, Partition};
use alloc::{string::String, sync::Arc, vec, vec::Vec};
#[cfg(test)]
pub(crate) use blob::tests::fixture as blob_fixture;
use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use critical_section::Mutex;
use embedded_sdmmc::BlockDevice;

/// Максимальное число файлов и папок в каталоге WFSv1.
pub const MAX_ENTRIES: usize = 32;
/// Максимальный размер файла для данного формата, байт.
const LEGACY_FILE: usize = 3072;
pub const MAX_FILE: usize = 64 * 1024;
/// Потоковые вложения: до 16 МиБ, без загрузки целого файла в RAM.
pub const MAX_BLOB: usize = 16 * 1024 * 1024;
/// Хвост userdata исключён из расширения swap.
pub const BLOB_RESERVE: u64 = 64 * 1024 * 1024;
const UNIT: u64 = 64 * 1024;
const UNIT_DATA: usize = 64 * 1024 - 512;
/// Максимальная длина полного UTF-8-пути, байт.
pub const MAX_PATH: usize = 80;
/// Разрешение внешних файловых операций от PIN-гейта.
static EXTERNAL_ACCESS: AtomicBool = AtomicBool::new(false);
/// Разрешает или запрещает внешний доступ к документам согласно PIN-блокировке.
pub fn set_external_access(allowed: bool) {
    EXTERNAL_ACCESS.store(allowed, Ordering::Relaxed);
}
/// Сообщает, разрешён ли сейчас внешний доступ к документам.
pub fn external_access() -> bool {
    EXTERNAL_ACCESS.load(Ordering::Relaxed)
}
/// Начальное смещение резервной области файлов в userdata, байт.
const BASE: u64 = 2 * 1024 * 1024;
/// Размер одной копии/страницы на носителе, байт.
const PAGE: usize = 4096;
/// Полный размер резервной области WFSv1, байт.
const REGION: u64 = (2 + MAX_ENTRIES * 2) as u64 * PAGE as u64;
/// Сигнатура, отличающая собственный формат данных от чужих.
const MAGIC: &[u8; 8] = b"WFSv1\0\0\0";
/// Размер сериализованной записи каталога, байт.
const RECORD: usize = 96;
/// Вычисляет CRC каталога вместе с поколением, исключая само поле CRC.
fn metadata_crc(bytes: &[u8]) -> u32 {
    !gpt::crc32_update(gpt::crc32_update(!0, &bytes[..16]), &bytes[20..])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Причины отказа операций компонента транзакционного хранилища документов WFSv1.
pub enum Error {
    Unavailable,
    Foreign,
    Corrupt,
    Io,
    InvalidPath,
    NotFound,
    Exists,
    Full,
    TooLarge,
    NotEmpty,
    Busy,
    Conflict,
}
impl Error {
    /// Возвращает подпись для отображения пользователю.
    pub fn label(self) -> &'static str {
        match self {
            Self::Unavailable => "Нет хранилища",
            Self::Foreign => "Чужой формат userdata",
            Self::Corrupt => "Ошибка целостности",
            Self::Io => "Ошибка записи/чтения",
            Self::InvalidPath => "Недопустимое имя",
            Self::NotFound => "Файл не найден",
            Self::Exists => "Имя уже занято",
            Self::Full => "Нет свободных записей",
            Self::TooLarge => "Размер недоступен для этой операции",
            Self::NotEmpty => "Папка не пуста",
            Self::Busy => "Хранилище занято",
            Self::Conflict => "Файл изменён: откройте заново",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
/// Метаданные одной записи компонента транзакционного хранилища документов WFSv1.
pub struct Entry {
    /// Вид записи или пакета; допустимые значения определяет формат компонента.
    pub kind: u8, // 0 free, 1 file, 2 directory
    /// Номер активной копии данных (0 или 1).
    pub bank: u8,
    /// Размер содержимого в байтах.
    pub size: u32,
    /// Начальный 64-КиБ extent больших данных; None оставляет старый WFD1.
    pub blob: Option<u16>,
    /// Контрольная сумма содержимого для обнаружения повреждений.
    pub crc: u32,
    /// Полный путь записи внутри управляемого хранилища.
    pub path: heapless::String<MAX_PATH>,
}
impl Default for Entry {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            kind: 0,
            bank: 0,
            size: 0,
            blob: None,
            crc: 0,
            path: heapless::String::new(),
        }
    }
}
/// Проверяет абсолютный путь, длину и отсутствие запрещённых компонентов.
pub fn path_valid(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= MAX_PATH
        && path != "/"
        && !path.ends_with('/')
        && !path.chars().any(|c| c.is_control() || c == '\\')
        && path[1..]
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
/// Возвращает родительскую папку; для верхнего уровня используется корень.
pub fn parent(path: &str) -> &str {
    path.rsplit_once('/')
        .map(|(p, _)| if p.is_empty() { "/" } else { p })
        .unwrap_or("/")
}
/// Возвращает последний компонент пути.
pub fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
/// Соединяет папку и имя, затем проверяет полученный путь.
pub fn join(dir: &str, name: &str) -> Result<String, Error> {
    if name.contains('/') || name.is_empty() {
        return Err(Error::InvalidPath);
    }
    let result = alloc::format!("{}/{}", dir.trim_end_matches('/'), name);
    if path_valid(&result) {
        Ok(result)
    } else {
        Err(Error::InvalidPath)
    }
}

/// Непубликованная потоковая запись; старый каталог остаётся целым до FinishUpload.
struct Upload {
    path: String,
    index: usize,
    start: u16,
    size: usize,
    crc: u32,
    generation: u64,
}
#[path = "file_store/blob.rs"]
mod blob;
#[path = "file_store/image.rs"]
mod image;
#[path = "file_store/png.rs"]
mod png;
/// Владелец каталога и транзакций WFSv1 компонента транзакционного хранилища документов WFSv1.
pub struct Fs {
    /// Границы физического раздела, которому принадлежит область.
    part: Partition,
    /// Таблица записей с проверенными именами и метаданными.
    entries: Vec<Entry>,
    /// Поколение состояния, позволяющее отличить новую версию от старой.
    generation: u64,
    /// Номер активной копии каталога.
    meta_bank: u8,
    /// Признак ещё не инициализированной резервной области.
    virgin: bool,
    upload: Option<Upload>,
    pool_base: Option<u64>,
}
impl Fs {
    /// Фиксированная граница данных сохраняется и после увеличения userdata.
    pub fn saved_blob_base(&self) -> Option<u64> {
        self.pool_base
    }
    /// Читает метаданные файлового хранилища и выбирает целую копию каталога.
    pub fn mount<D: BlockDevice>(dev: &D) -> Result<Self, Error> {
        let part = gpt::find_partition(dev, "userdata").map_err(|_| Error::Unavailable)?;
        if part.size_bytes() < BASE + REGION {
            return Err(Error::Unavailable);
        }
        let mut sector = [0; 512];
        gpt::read_partition(dev, part, 0, &mut sector).map_err(|_| Error::Io)?;
        if sector.iter().any(|&v| v != 0) && &sector[..8] != b"WALLv1\0\0" {
            return Err(Error::Foreign);
        }
        if sector.iter().all(|&v| v == 0) {
            gpt::read_partition(dev, part, 1024, &mut sector).map_err(|_| Error::Io)?;
            if &sector[56..58] == b"\x53\xef" || &sector[..4] == b"\x10\x20\xf5\xf2" {
                return Err(Error::Foreign);
            }
        }
        let mut selected = None;
        let mut blank = true;
        for bank in 0..2 {
            let mut bytes = vec![0; PAGE];
            gpt::read_partition(dev, part, BASE + bank * PAGE as u64, &mut bytes)
                .map_err(|_| Error::Io)?;
            blank &= bytes.iter().all(|&v| v == 0);
            if let Ok((generation, entries)) = Self::decode(&bytes) {
                if selected.as_ref().is_none_or(|(g, _, _, _)| generation > *g) {
                    selected = Some((
                        generation,
                        bank as u8,
                        entries,
                        u64::from_le_bytes(bytes[20..28].try_into().unwrap()),
                    ));
                }
            }
        }
        if let Some((generation, meta_bank, entries, base)) = selected {
            if base != 0 && (base < BASE + REGION || base + BLOB_RESERVE > part.size_bytes()) {
                return Err(Error::Corrupt);
            }
            Ok(Self {
                part,
                entries,
                generation,
                meta_bank,
                virgin: false,
                upload: None,
                pool_base: if base == 0 { None } else { Some(base) },
            })
        } else if blank {
            Ok(Self {
                part,
                entries: vec![Entry::default(); MAX_ENTRIES],
                generation: 0,
                meta_bank: 1,
                virgin: true,
                upload: None,
                pool_base: None,
            })
        } else {
            Err(Error::Corrupt)
        }
    }
    /// Проверяет двоичное представление и восстанавливает типизированные данные.
    fn decode(bytes: &[u8]) -> Result<(u64, Vec<Entry>), Error> {
        if bytes.len() != PAGE
            || &bytes[..8] != MAGIC
            || metadata_crc(bytes) != u32::from_le_bytes(bytes[16..20].try_into().unwrap())
        {
            return Err(Error::Corrupt);
        }
        let generation = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let mut entries = Vec::<Entry>::with_capacity(MAX_ENTRIES);
        for i in 0..MAX_ENTRIES {
            let b = &bytes[32 + i * RECORD..32 + (i + 1) * RECORD];
            if b[0] > 2 || b[1] > 1 || b[8] as usize > MAX_PATH {
                return Err(Error::Corrupt);
            }
            let path =
                core::str::from_utf8(&b[9..9 + b[8] as usize]).map_err(|_| Error::Corrupt)?;
            if b[0] != 0
                && (!path_valid(path) || entries.iter().any(|e| e.kind != 0 && e.path == path))
            {
                return Err(Error::Corrupt);
            }
            let blob = if b[89] == 1 {
                Some(u16::from_le_bytes([b[94], b[95]]))
            } else if b[89] == 0 {
                None
            } else {
                return Err(Error::Corrupt);
            };
            let size = if blob.is_some() {
                u32::from_le_bytes(b[90..94].try_into().unwrap())
            } else {
                u32::from(u16::from_le_bytes([b[2], b[3]]))
            };
            if (blob.is_none() && size as usize > LEGACY_FILE)
                || size as usize > MAX_BLOB
                || (b[0] == 2 && size != 0)
            {
                return Err(Error::Corrupt);
            }
            entries.push(Entry {
                kind: b[0],
                bank: b[1],
                size,
                blob,
                crc: u32::from_le_bytes(b[4..8].try_into().unwrap()),
                path: heapless::String::try_from(path).map_err(|_| Error::Corrupt)?,
            });
        }
        for e in entries.iter().filter(|e| e.kind != 0) {
            let p = parent(&e.path);
            if p != "/" && !entries.iter().any(|e| e.kind == 2 && e.path == p) {
                return Err(Error::Corrupt);
            }
        }
        Ok((generation, entries))
    }
    /// Находит индекс существующей записи по полному пути.
    fn index(&self, path: &str) -> Result<usize, Error> {
        self.entries
            .iter()
            .position(|e| e.kind != 0 && e.path == path)
            .ok_or(Error::NotFound)
    }
    /// Проверяет путь назначения и наличие родительской папки.
    fn destination(&self, path: &str) -> Result<(), Error> {
        if !path_valid(path) {
            return Err(Error::InvalidPath);
        }
        let p = parent(path);
        if p != "/" && !self.entries.iter().any(|e| e.kind == 2 && e.path == p) {
            return Err(Error::NotFound);
        }
        Ok(())
    }
    /// Рассчитывает смещение выбранной копии файла внутри userdata.
    fn data_offset(index: usize, bank: u8) -> u64 {
        BASE + (2 + index * 2 + bank as usize) as u64 * PAGE as u64
    }
    /// Читает файл и сверяет длину и CRC с активным каталогом.
    pub fn read<D: BlockDevice>(&self, dev: &D, path: &str) -> Result<Vec<u8>, Error> {
        let index = self.index(path)?;
        let e = &self.entries[index];
        if e.blob.is_some() {
            if e.size as usize > 320 * 1024 {
                return Err(Error::TooLarge);
            }
            let mut data = Vec::with_capacity(e.size as usize);
            while data.len() < e.size as usize {
                data.extend(self.read_range(dev, path, data.len(), 4096)?);
            }
            if gpt::crc32(&data) != e.crc {
                return Err(Error::Corrupt);
            }
            return Ok(data);
        }
        if e.kind != 1 {
            return Err(Error::InvalidPath);
        }
        let used = (e.size as usize + 16).div_ceil(512) * 512;
        let mut bytes = vec![0; used];
        gpt::read_partition(dev, self.part, Self::data_offset(index, e.bank), &mut bytes)
            .map_err(|_| Error::Io)?;
        if &bytes[..4] != b"WFD1"
            || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != u32::from(e.size)
            || gpt::crc32(&bytes[16..16 + e.size as usize]) != e.crc
        {
            return Err(Error::Corrupt);
        }
        Ok(bytes[16..16 + e.size as usize].to_vec())
    }
    /// Проверяет пустоту резервной области до первого создания хранилища.
    fn check_virgin<D: BlockDevice>(&mut self, dev: &D) -> Result<(), Error> {
        if self.virgin {
            let mut page = vec![0; PAGE];
            for off in (0..REGION).step_by(PAGE) {
                gpt::read_partition(dev, self.part, BASE + off, &mut page)
                    .map_err(|_| Error::Io)?;
                if page.iter().any(|&v| v != 0) {
                    return Err(Error::Foreign);
                }
            }
            self.virgin = false;
            // Establish an empty owned volume before writing the first data page.
            // A power loss after this point is recoverable as an empty filesystem.
            self.commit(dev, self.entries.clone())?;
        }
        Ok(())
    }
    /// Записывает и проверяет новую копию каталога до её публикации.
    fn commit<D: BlockDevice>(&mut self, dev: &D, entries: Vec<Entry>) -> Result<(), Error> {
        self.check_virgin(dev)?;
        let generation = self.generation.checked_add(1).ok_or(Error::Full)?;
        let bank = 1 - self.meta_bank;
        let mut bytes = vec![0; PAGE];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..16].copy_from_slice(&generation.to_le_bytes());
        let pool = if entries.iter().any(|e| e.blob.is_some()) {
            Some(self.blob_base()?)
        } else {
            self.pool_base
        };
        bytes[20..28].copy_from_slice(&pool.unwrap_or(0).to_le_bytes());
        for (i, e) in entries.iter().enumerate() {
            let b = &mut bytes[32 + i * RECORD..32 + (i + 1) * RECORD];
            b[0] = e.kind;
            b[1] = e.bank;
            b[2..4].copy_from_slice(
                &(if e.blob.is_some() {
                    u16::MAX
                } else {
                    e.size as u16
                })
                .to_le_bytes(),
            );
            if let Some(start) = e.blob {
                b[89] = 1;
                b[90..94].copy_from_slice(&e.size.to_le_bytes());
                b[94..96].copy_from_slice(&start.to_le_bytes());
            }
            b[4..8].copy_from_slice(&e.crc.to_le_bytes());
            b[8] = e.path.len() as u8;
            b[9..9 + e.path.len()].copy_from_slice(e.path.as_bytes());
        }
        let crc = metadata_crc(&bytes);
        bytes[16..20].copy_from_slice(&crc.to_le_bytes());
        gpt::write_partition(dev, self.part, BASE + u64::from(bank) * PAGE as u64, &bytes)
            .map_err(|_| Error::Io)?;
        let mut check = vec![0; PAGE];
        gpt::read_partition(
            dev,
            self.part,
            BASE + u64::from(bank) * PAGE as u64,
            &mut check,
        )
        .map_err(|_| Error::Io)?;
        if check != bytes {
            return Err(Error::Io);
        }
        self.pool_base = pool;
        self.entries = entries;
        self.generation = generation;
        self.meta_bank = bank;
        Ok(())
    }
    /// Проверяет конфликт версии, записывает запасную копию и только затем публикует каталог.
    pub fn write<D: BlockDevice>(
        &mut self,
        dev: &D,
        path: &str,
        data: &[u8],
        expected: Option<u32>,
    ) -> Result<(), Error> {
        if data.len() > MAX_FILE {
            return Err(Error::TooLarge);
        }
        self.destination(path)?;
        let index = match self.index(path) {
            Ok(i) if self.entries[i].kind == 1 => {
                if expected.is_some_and(|crc| crc != self.entries[i].crc) {
                    return Err(Error::Conflict);
                }
                i
            }
            Ok(_) => return Err(Error::Exists),
            Err(_) if expected.is_some() => return Err(Error::Conflict),
            Err(_) => self
                .entries
                .iter()
                .position(|e| e.kind == 0)
                .ok_or(Error::Full)?,
        };
        if data.len() > LEGACY_FILE {
            self.begin_upload(dev, path)?;
            self.upload_chunk(dev, data)?;
            return self.finish_upload(dev);
        }
        self.check_virgin(dev)?;
        let bank = 1 - self.entries[index].bank;
        let mut bytes = vec![0; (16 + data.len()).div_ceil(512) * 512];
        bytes[..4].copy_from_slice(b"WFD1");
        bytes[4..8].copy_from_slice(&(data.len() as u32).to_le_bytes());
        bytes[16..16 + data.len()].copy_from_slice(data);
        gpt::write_partition(dev, self.part, Self::data_offset(index, bank), &bytes)
            .map_err(|_| Error::Io)?;
        let mut check = vec![0; bytes.len()];
        gpt::read_partition(dev, self.part, Self::data_offset(index, bank), &mut check)
            .map_err(|_| Error::Io)?;
        if check != bytes {
            return Err(Error::Io);
        }
        let mut entries = self.entries.clone();
        entries[index] = Entry {
            kind: 1,
            bank,
            size: data.len() as u32,
            blob: None,
            crc: gpt::crc32(data),
            path: heapless::String::try_from(path).map_err(|_| Error::InvalidPath)?,
        };
        self.commit(dev, entries)
    }
    /// Создаёт папку без перезаписи занятого имени.
    pub fn mkdir<D: BlockDevice>(&mut self, dev: &D, path: &str) -> Result<(), Error> {
        self.destination(path)?;
        if self.index(path).is_ok() {
            return Err(Error::Exists);
        }
        let mut entries = self.entries.clone();
        let e = entries
            .iter_mut()
            .find(|e| e.kind == 0)
            .ok_or(Error::Full)?;
        e.kind = 2;
        e.size = 0;
        e.crc = 0;
        e.path = heapless::String::try_from(path).map_err(|_| Error::InvalidPath)?;
        self.commit(dev, entries)
    }
    /// Удаляет файл или пустую папку через новую версию каталога.
    pub fn remove<D: BlockDevice>(&mut self, dev: &D, path: &str) -> Result<(), Error> {
        let index = self.index(path)?;
        if self
            .entries
            .iter()
            .any(|e| e.kind != 0 && e.path.starts_with(&alloc::format!("{}/", path)))
        {
            return Err(Error::NotEmpty);
        }
        let mut entries = self.entries.clone();
        entries[index].kind = 0;
        entries[index].size = 0;
        entries[index].path.clear();
        self.commit(dev, entries)
    }
    /// Меняет путь записи и всех вложенных элементов одной публикацией каталога.
    pub fn rename<D: BlockDevice>(&mut self, dev: &D, from: &str, to: &str) -> Result<(), Error> {
        self.index(from)?;
        self.destination(to)?;
        if from == to {
            return Ok(());
        }
        if self.index(to).is_ok() {
            return Err(Error::Exists);
        }
        let prefix = alloc::format!("{}/", from);
        if to.starts_with(&prefix) {
            return Err(Error::InvalidPath);
        }
        let mut entries = self.entries.clone();
        for e in &mut entries {
            if e.kind != 0 && (e.path == from || e.path.starts_with(&prefix)) {
                let path = alloc::format!("{}{}", to, &e.path[from.len()..]);
                if !path_valid(&path) {
                    return Err(Error::InvalidPath);
                }
                e.path =
                    heapless::String::try_from(path.as_str()).map_err(|_| Error::InvalidPath)?;
            }
        }
        self.commit(dev, entries)
    }
    /// Копирует файл или дерево папок, не перезаписывая существующее назначение.
    pub fn copy<D: BlockDevice>(&mut self, dev: &D, from: &str, to: &str) -> Result<(), Error> {
        let source = self.index(from)?;
        self.destination(to)?;
        if self.index(to).is_ok() {
            return Err(Error::Exists);
        }
        let prefix = alloc::format!("{}/", from);
        if to.starts_with(&prefix) {
            return Err(Error::InvalidPath);
        }
        if self.entries[source].kind == 1 {
            if self.entries[source].blob.is_some() {
                let size = self.entries[source].size as usize;
                self.begin_upload(dev, to)?;
                let mut at = 0;
                while at < size {
                    let chunk = self.read_range(dev, from, at, 4096)?;
                    self.upload_chunk(dev, &chunk)?;
                    at += chunk.len();
                }
                return self.finish_upload(dev);
            }
            let bytes = self.read(dev, from)?;
            return self.write(dev, to, &bytes, None);
        }
        let mut originals: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.kind != 0 && (e.path == from || e.path.starts_with(&prefix)))
            .cloned()
            .collect();
        if self.entries.iter().filter(|e| e.kind == 0).count() < originals.len() {
            return Err(Error::Full);
        }
        originals.sort_by_key(|e| e.path.len());
        for entry in &originals {
            let target = alloc::format!("{}{}", to, &entry.path[from.len()..]);
            if !path_valid(&target) {
                return Err(Error::InvalidPath);
            }
            if self.index(&target).is_ok() {
                return Err(Error::Exists);
            }
        }
        for entry in originals {
            let target = alloc::format!("{}{}", to, &entry.path[from.len()..]);
            if entry.kind == 2 {
                self.mkdir(dev, &target)?;
            } else {
                self.copy(dev, &entry.path, &target)?;
            }
        }
        Ok(())
    }
}

/// Разрешённая операция над файловым хранилищем компонента транзакционного хранилища документов WFSv1.
pub enum Op {
    Thumbnail(String),
    ReadLog,
    BeginUpload(String),
    Upload(Vec<u8>),
    FinishUpload,
    ReadRange(String, usize, usize),
    Mount,
    Read(String),
    Create(String, Vec<u8>),
    Write(String, Vec<u8>, Option<u32>),
    Mkdir(String),
    Remove(String),
    Rename(String, String),
    Copy(String, String),
}
/// Результат завершённой дисковой операции компонента транзакционного хранилища документов WFSv1.
pub struct Reply {
    /// Результат операции либо точная причина её отказа.
    pub result: Result<Option<Vec<u8>>, Error>,
}
/// Состояние очереди запросов и результатов компонента транзакционного хранилища документов WFSv1.
struct Control {
    /// Открытый экземпляр WFS для последовательных дисковых операций.
    fs: Option<Fs>,
    /// Ожидающая операция вместе с идентификатором владельца ответа.
    job: Option<(u32, Op)>,
    /// Ещё не забранный результат запроса.
    reply: Option<(u32, Reply)>,
    /// Общий неизменяемый снимок каталога для интерфейса.
    catalog: Option<Arc<Vec<Entry>>>,
    /// Ошибка последней операции или признак ошибки.
    error: Option<Error>,
    /// Признак выполняющегося I/O; не допускает второго владельца транзакции.
    processing: bool,
}
/// Общая очередь операций и результатов; I/O выполняется вне блокировки.
static CONTROL: Mutex<RefCell<Control>> = Mutex::new(RefCell::new(Control {
    fs: None,
    job: None,
    reply: None,
    catalog: None,
    error: None,
    processing: false,
}));
/// Источник идентификаторов запросов без повторного использования в сессии.
static SEQUENCE: AtomicU32 = AtomicU32::new(1);
/// Счётчик изменений, по которому обновляются зависимые кэши.
static REVISION: AtomicU32 = AtomicU32::new(0);
/// Возвращает счётчик изменений для обновления зависимых кэшей.
pub fn revision() -> u32 {
    REVISION.load(Ordering::Relaxed)
}
/// Ставит запрос в очередь для обработки владельцем ресурса.
pub fn request(op: Op) -> Result<u32, Error> {
    critical_section::with(|cs| {
        let mut c = CONTROL.borrow_ref_mut(cs);
        if c.processing || c.job.is_some() || c.reply.is_some() {
            return Err(Error::Busy);
        }
        let token = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        c.job = Some((token, op));
        Ok(token)
    })
}
/// Забирает результат только для указанного идентификатора запроса.
pub fn take_reply(token: u32) -> Option<Reply> {
    critical_section::with(|cs| {
        let mut c = CONTROL.borrow_ref_mut(cs);
        if c.reply.as_ref().is_some_and(|(id, _)| *id == token) {
            c.reply.take().map(|(_, r)| r)
        } else {
            None
        }
    })
}
/// Возвращает общий снимок каталога без чтения диска.
pub fn catalog() -> Option<Arc<Vec<Entry>>> {
    critical_section::with(|cs| CONTROL.borrow_ref(cs).catalog.clone())
}
#[cfg(test)]
/// Сбрасывает общий стенд перед независимым тестом.
pub(crate) fn reset_for_test() {
    critical_section::with(|cs| {
        let mut c = CONTROL.borrow_ref_mut(cs);
        c.fs = None;
        c.job = None;
        c.reply = None;
        c.catalog = None;
        c.error = None;
        c.processing = false;
    });
    REVISION.fetch_add(1, Ordering::Relaxed);
}
/// Отменяет ещё ожидающую операцию или незабранный результат.
pub fn cancel(token: u32) {
    critical_section::with(|cs| {
        let mut c = CONTROL.borrow_ref_mut(cs);
        if c.job.as_ref().is_some_and(|(id, _)| *id == token) {
            c.job = None;
        }
        if c.reply.as_ref().is_some_and(|(id, _)| *id == token) {
            c.reply = None;
        }
    });
}
/// Обрабатывает отложенный дисковый запрос и публикует результат.
#[inline(never)]
pub fn service<D: BlockDevice>(dev: &D) -> bool {
    let Some((token, op, fs)) = critical_section::with(|cs| {
        let mut c = CONTROL.borrow_ref_mut(cs);
        let (token, op) = c.job.take()?;
        c.processing = true;
        Some((token, op, c.fs.take()))
    }) else {
        return false;
    };
    let mounted = fs.map(Ok).unwrap_or_else(|| Fs::mount(dev));
    let (fs, result) = match mounted {
        Err(e) => (None, Err(e)),
        Ok(mut fs) => {
            let result = match op {
                Op::BeginUpload(path) => fs.begin_upload(dev, &path).map(|_| None),
                Op::Upload(data) => fs.upload_chunk(dev, &data).map(|_| None),
                Op::FinishUpload => fs.finish_upload(dev).map(|_| None),
                Op::ReadRange(path, offset, len) => {
                    fs.read_range(dev, &path, offset, len).map(Some)
                }
                Op::ReadLog => crate::persistent_log::export(dev).map(Some),
                Op::Thumbnail(path) => fs.thumbnail(dev, &path).map(Some),
                Op::Mount => Ok(None),
                Op::Read(path) => fs.read(dev, &path).map(Some),
                Op::Create(path, data) => {
                    if fs.index(&path).is_ok() {
                        Err(Error::Exists)
                    } else {
                        fs.write(dev, &path, &data, None).map(|_| None)
                    }
                }
                Op::Write(path, data, crc) => fs.write(dev, &path, &data, crc).map(|_| None),
                Op::Mkdir(path) => fs.mkdir(dev, &path).map(|_| None),
                Op::Remove(path) => fs.remove(dev, &path).map(|_| None),
                Op::Rename(from, to) => fs.rename(dev, &from, &to).map(|_| None),
                Op::Copy(from, to) => fs.copy(dev, &from, &to).map(|_| None),
            };
            (Some(fs), result)
        }
    };
    critical_section::with(|cs| {
        let mut c = CONTROL.borrow_ref_mut(cs);
        c.error = result.as_ref().err().copied();
        if let Some(fs) = &fs {
            if c.catalog
                .as_ref()
                .is_none_or(|old| old.as_slice() != fs.entries.as_slice())
            {
                c.catalog = Some(Arc::new(fs.entries.clone()));
                REVISION.fetch_add(1, Ordering::Relaxed);
            }
        }
        c.fs = fs;
        c.reply = Some((token, Reply { result }));
        c.processing = false;
    });
    true
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use core::cell::Cell;
    use embedded_sdmmc::{Block, BlockCount, BlockIdx};
    /// Модель блочного устройства для host-проверок компонента транзакционного хранилища документов WFSv1.
    pub(crate) struct Disk {
        /// Содержимое текущего пакета, страницы или модели.
        pub data: RefCell<Vec<[u8; 512]>>,
        fail: Cell<Option<usize>>,
        reads: Cell<usize>,
    }
    impl BlockDevice for Disk {
        /// Причины отказа операций компонента транзакционного хранилища документов WFSv1.
        type Error = std::io::Error;
        /// Возвращает ёмкость блочного устройства в секторах по 512 байт.
        fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
            Ok(BlockCount(self.data.borrow().len() as u32))
        }
        /// Реализует секторное чтение для данного блочного backend.
        fn read(&self, blocks: &mut [Block], index: BlockIdx) -> Result<(), Self::Error> {
            self.reads.set(self.reads.get() + 1);
            for (i, block) in blocks.iter_mut().enumerate() {
                block.contents = self.data.borrow()[index.0 as usize + i];
            }
            Ok(())
        }
        /// Реализует секторную запись для данного backend с его ограничениями.
        fn write(&self, blocks: &[Block], index: BlockIdx) -> Result<(), Self::Error> {
            for (i, block) in blocks.iter().enumerate() {
                if let Some(left) = self.fail.get() {
                    if left == 0 {
                        return Err(std::io::Error::other("power cut"));
                    }
                    self.fail.set(Some(left - 1));
                }
                self.data.borrow_mut()[index.0 as usize + i] = block.contents;
            }
            Ok(())
        }
    }
    /// Создаёт изолированный тестовый накопитель с ожидаемой разметкой.
    pub(crate) fn fixture() -> Disk {
        let mut data = vec![[0; 512]; 8192];
        let entry = &mut data[2];
        entry[..16].fill(1);
        entry[32..40].copy_from_slice(&64u64.to_le_bytes());
        entry[40..48].copy_from_slice(&8191u64.to_le_bytes());
        for (i, b) in b"userdata".iter().enumerate() {
            entry[56 + i * 2] = *b;
        }
        let entries_crc = gpt::crc32(&entry[..128]);
        let h = &mut data[1];
        h[..8].copy_from_slice(b"EFI PART");
        h[12..16].copy_from_slice(&92u32.to_le_bytes());
        h[72..80].copy_from_slice(&2u64.to_le_bytes());
        h[80..84].copy_from_slice(&1u32.to_le_bytes());
        h[84..88].copy_from_slice(&128u32.to_le_bytes());
        h[88..92].copy_from_slice(&entries_crc.to_le_bytes());
        let crc = gpt::crc32(&h[..92]);
        h[16..20].copy_from_slice(&crc.to_le_bytes());
        Disk {
            data: RefCell::new(data),
            fail: Cell::new(None),
            reads: Cell::new(0),
        }
    }
    #[test]
    /// Проверяет повторное использование одного сектора при поиске четырёх записей GPT.
    fn gpt_reuses_one_sector_for_four_directory_entries() {
        let disk = fixture();
        {
            let mut data = disk.data.borrow_mut();
            let original = data[2][..128].to_vec();
            for i in 0..4 {
                data[2][i * 128..(i + 1) * 128].copy_from_slice(&original);
                if i != 3 {
                    data[2][i * 128 + 56] = b'x';
                }
            }
            let crc = gpt::crc32(&data[2]);
            let h = &mut data[1];
            h[80..84].copy_from_slice(&4u32.to_le_bytes());
            h[88..92].copy_from_slice(&crc.to_le_bytes());
            h[16..20].fill(0);
            let crc = gpt::crc32(&h[..92]);
            h[16..20].copy_from_slice(&crc.to_le_bytes());
        }
        assert!(gpt::find_partition(&disk, "userdata").is_ok());
        assert_eq!(disk.reads.get(), 3); // header, verified array, one cached scan sector
    }
    #[test]
    /// Проверяет UTF-8, папки, копирование, конфликты и ограничения после повторного открытия WFS.
    fn folders_utf8_rename_conflicts_and_limits_survive_mount() {
        let disk = fixture();
        let mut fs = Fs::mount(&disk).unwrap();
        fs.mkdir(&disk, "/Docs").unwrap();
        fs.write(&disk, "/Docs/a.txt", "Привет\nмир".as_bytes(), None)
            .unwrap();
        assert_eq!(fs.remove(&disk, "/Docs"), Err(Error::NotEmpty));
        assert_eq!(
            fs.write(&disk, "/Docs/a.txt", b"x", Some(1)),
            Err(Error::Conflict)
        );
        fs.rename(&disk, "/Docs", "/Notes").unwrap();
        fs.copy(&disk, "/Notes", "/Backup").unwrap();
        let mut restored = Fs::mount(&disk).unwrap();
        assert_eq!(
            restored.read(&disk, "/Notes/a.txt").unwrap(),
            "Привет\nмир".as_bytes()
        );
        assert_eq!(
            restored.read(&disk, "/Backup/a.txt").unwrap(),
            "Привет\nмир".as_bytes()
        );
        assert_eq!(
            restored.write(&disk, "/Notes/../x", b"x", None),
            Err(Error::InvalidPath)
        );
        assert_eq!(
            restored.write(&disk, "/Notes/huge", &vec![0; MAX_FILE + 1], None),
            Err(Error::TooLarge)
        );
        assert_eq!(
            restored.rename(&disk, "/Notes", "/Notes/child"),
            Err(Error::InvalidPath)
        );
    }
    #[test]
    /// Моделирует обрыв записи по секторам и проверяет сохранность целой старой либо новой версии.
    fn interrupted_file_and_directory_commits_keep_a_complete_version() {
        let disk = fixture();
        let mut fs = Fs::mount(&disk).unwrap();
        fs.write(&disk, "/n.txt", b"old", None).unwrap();
        let original = disk.data.borrow().clone();
        let new = vec![7; 1024];
        for cut in 0..16 {
            *disk.data.borrow_mut() = original.clone();
            disk.fail.set(None);
            let mut fs = Fs::mount(&disk).unwrap();
            disk.fail.set(Some(cut));
            let _ = fs.write(&disk, "/n.txt", &new, None);
            disk.fail.set(None);
            let restored = Fs::mount(&disk).unwrap();
            let value = restored.read(&disk, "/n.txt").unwrap();
            assert!(value == b"old" || value == new, "cut at sector {}", cut);
        }
    }
    #[test]
    /// Проверяет отказ форматирования резервной области с неизвестными данными.
    fn unknown_reserved_data_is_never_formatted() {
        let disk = fixture();
        let sector = 64 + (BASE as usize / 512) + 30;
        disk.data.borrow_mut()[sector][4] = 99;
        let mut fs = Fs::mount(&disk).unwrap();
        assert_eq!(fs.mkdir(&disk, "/Docs"), Err(Error::Foreign));
        assert_eq!(disk.data.borrow()[sector][4], 99);
    }
    #[test]
    /// Сверяет табличную CRC32 с известным вектором и побитовой реализацией.
    fn crc_matches_standard_and_bitwise_reference() {
        assert_eq!(gpt::crc32(b"123456789"), 0xcbf43926);
        let bytes: Vec<_> = (0..4096).map(|i| (i * 37) as u8).collect();
        let mut crc = !0u32;
        for &b in &bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320 & (crc & 1).wrapping_neg());
            }
        }
        assert_eq!(gpt::crc32(&bytes), !crc);
    }
}

/// Контрольная сумма документа для обнаружения изменения при сохранении.
pub fn content_crc(data: &[u8]) -> u32 {
    gpt::crc32(data)
}

/// Decode a bounded downloaded image using the same native row decoders as WFS.
pub(crate) fn network_thumbnail(bytes: &[u8]) -> Option<Vec<u16>> {
    if bytes.len() > 192*1024 { return None; }
    image::from_slice(bytes).ok()
}
