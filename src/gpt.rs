// Модуль чтения и проверки GPT.
use embedded_sdmmc::{Block, BlockDevice, BlockIdx};

/// Размер сектора блочного протокола, байт.
pub const SECTOR_SIZE: usize = 512;
/// Параметр GPT_SIGNATURE компонента чтения и проверки GPT.
const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";
/// Верхняя граница GPT_ENTRIES для ограничения размера или количества данных.
const MAX_GPT_ENTRIES: u32 = 256;

#[derive(Clone, Copy, Debug)]
/// Проверенные границы и имя физического GPT-раздела компонента чтения и проверки GPT.
pub struct Partition {
    /// Первый сектор раздела.
    pub first_lba: u64,
    /// Последний сектор раздела включительно.
    pub last_lba: u64,
    /// Имя компонента, устройства или записи.
    name: [u8; 36],
    /// Число значащих байтов буфера name.
    name_len: u8,
}

impl Partition {
    /// Возвращает длину раздела в 512-байтных секторах.
    pub fn blocks(self) -> u64 {
        self.last_lba - self.first_lba + 1
    }
    /// Возвращает размер области в байтах.
    pub fn size_bytes(self) -> u64 {
        self.blocks() * SECTOR_SIZE as u64
    }

    /// Сравнивает имя раздела с ожидаемой строкой.
    pub fn name_eq(&self, expected: &str) -> bool {
        self.name_len as usize == expected.len()
            && &self.name[..self.name_len as usize] == expected.as_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Ошибки операций компонента чтения и проверки GPT.
pub enum GptError {
    Io,
    BadSignature,
    BadHeader,
    BadHeaderCrc,
    BadEntryArrayCrc,
    Unsupported,
    PartitionNotFound,
    OutOfBounds,
}

#[derive(Clone, Copy)]
/// Разобранные поля заголовка формата компонента чтения и проверки GPT.
struct Header {
    /// Первый сектор массива записей GPT.
    entries_lba: u64,
    /// Количество записей в проверяемой таблице.
    entry_count: u32,
    /// Размер одной записи таблицы в байтах.
    entry_size: u32,
    /// Ожидаемая CRC32 массива записей GPT.
    entries_crc32: u32,
}

/// Проверяет доступность и базовую структуру GPT.
pub fn probe<D: BlockDevice>(dev: &D) -> Result<(), GptError> {
    let _ = read_header(dev)?;
    Ok(())
}

/// Ищет раздел по имени после проверки CRC таблицы GPT.
pub fn find_partition<D: BlockDevice>(dev: &D, wanted: &str) -> Result<Partition, GptError> {
    let header = read_header(dev)?;
    verify_entry_array_crc(dev, &header)?;

    let mut entry = [0u8; 512];
    let mut sector = Block { contents: [0; SECTOR_SIZE] };
    let mut cached_lba = u64::MAX;
    for index in 0..header.entry_count {
        let absolute = header
            .entries_lba
            .checked_mul(SECTOR_SIZE as u64)
            .and_then(|v| v.checked_add(index as u64 * header.entry_size as u64))
            .ok_or(GptError::OutOfBounds)?;
        let lba = absolute / SECTOR_SIZE as u64;
        let offset = (absolute % SECTOR_SIZE as u64) as usize;
        let size = header.entry_size as usize;
        if offset + size <= SECTOR_SIZE && lba <= u32::MAX as u64 {
            if cached_lba != lba {
                dev.read(core::slice::from_mut(&mut sector), BlockIdx(lba as u32)).map_err(|_| GptError::Io)?;
                cached_lba = lba;
            }
            entry[..size].copy_from_slice(&sector.contents[offset..offset + size]);
        } else { read_absolute(dev, absolute, &mut entry[..size])?; }

        if entry[..16].iter().all(|b| *b == 0) {
            continue;
        }
        let first_lba = le_u64(&entry[32..40]);
        let last_lba = le_u64(&entry[40..48]);
        if first_lba == 0 || last_lba < first_lba {
            continue;
        }

        let mut name = [0u8; 36];
        let mut name_len = 0usize;
        for pair in entry[56..128].chunks_exact(2) {
            let cp = u16::from_le_bytes([pair[0], pair[1]]);
            if cp == 0 {
                break;
            }
            if cp > 0x7f || name_len == name.len() {
                break;
            }
            name[name_len] = cp as u8;
            name_len += 1;
        }

        let partition = Partition {
            first_lba,
            last_lba,
            name,
            name_len: name_len as u8,
        };
        if partition.name_eq(wanted) {
            return Ok(partition);
        }
    }
    Err(GptError::PartitionNotFound)
}

/// Читает диапазон внутри проверенных границ раздела.
pub fn read_partition<D: BlockDevice>(
    dev: &D,
    part: Partition,
    offset: u64,
    out: &mut [u8],
) -> Result<(), GptError> {
    let end = offset
        .checked_add(out.len() as u64)
        .ok_or(GptError::OutOfBounds)?;
    if end > part.size_bytes() {
        return Err(GptError::OutOfBounds);
    }
    let absolute = part
        .first_lba
        .checked_mul(SECTOR_SIZE as u64)
        .and_then(|v| v.checked_add(offset))
        .ok_or(GptError::OutOfBounds)?;
    read_absolute(dev, absolute, out)
}

/// Пишет диапазон раздела, сохраняя соседние байты неполного сектора.
pub fn write_partition<D: BlockDevice>(
    dev: &D,
    part: Partition,
    offset: u64,
    data: &[u8],
) -> Result<(), GptError> {
    let end = offset
        .checked_add(data.len() as u64)
        .ok_or(GptError::OutOfBounds)?;
    if end > part.size_bytes() {
        return Err(GptError::OutOfBounds);
    }

    let mut copied = 0usize;
    while copied < data.len() {
        let absolute = part.first_lba * SECTOR_SIZE as u64 + offset + copied as u64;
        let lba = absolute / SECTOR_SIZE as u64;
        let in_sector = (absolute % SECTOR_SIZE as u64) as usize;
        if lba > u32::MAX as u64 {
            return Err(GptError::OutOfBounds);
        }

        let mut block = Block {
            contents: [0u8; SECTOR_SIZE],
        };
        let n = core::cmp::min(SECTOR_SIZE - in_sector, data.len() - copied);
        // A complete sector replaces every byte. Only partial-sector updates
        // need a read to preserve neighboring data (e.g. A/B metadata).
        if in_sector != 0 || n != SECTOR_SIZE {
            dev.read(core::slice::from_mut(&mut block), BlockIdx(lba as u32))
                .map_err(|_| GptError::Io)?;
        }
        block.contents[in_sector..in_sector + n].copy_from_slice(&data[copied..copied + n]);
        dev.write(core::slice::from_ref(&block), BlockIdx(lba as u32))
            .map_err(|_| GptError::Io)?;
        copied += n;
    }
    Ok(())
}

/// Читает и проверяет заголовок GPT.
fn read_header<D: BlockDevice>(dev: &D) -> Result<Header, GptError> {
    let mut block = Block {
        contents: [0u8; SECTOR_SIZE],
    };
    dev.read(core::slice::from_mut(&mut block), BlockIdx(1))
        .map_err(|_| GptError::Io)?;
    let b = &block.contents;
    if &b[..8] != GPT_SIGNATURE {
        return Err(GptError::BadSignature);
    }

    let header_size = le_u32(&b[12..16]) as usize;
    if !(92..=SECTOR_SIZE).contains(&header_size) {
        return Err(GptError::BadHeader);
    }
    let expected_crc = le_u32(&b[16..20]);
    let mut header_copy = [0u8; SECTOR_SIZE];
    header_copy[..header_size].copy_from_slice(&b[..header_size]);
    header_copy[16..20].fill(0);
    if crc32(&header_copy[..header_size]) != expected_crc {
        return Err(GptError::BadHeaderCrc);
    }

    let entries_lba = le_u64(&b[72..80]);
    let entry_count = le_u32(&b[80..84]);
    let entry_size = le_u32(&b[84..88]);
    let entries_crc32 = le_u32(&b[88..92]);
    if entry_count == 0
        || entry_count > MAX_GPT_ENTRIES
        || entry_size < 128
        || entry_size > 512
        || entry_size % 8 != 0
    {
        return Err(GptError::Unsupported);
    }
    Ok(Header {
        entries_lba,
        entry_count,
        entry_size,
        entries_crc32,
    })
}

/// Сверяет CRC всего массива записей GPT до поиска раздела.
fn verify_entry_array_crc<D: BlockDevice>(dev: &D, header: &Header) -> Result<(), GptError> {
    let total = header.entry_count as u64 * header.entry_size as u64;
    let start = header.entries_lba * SECTOR_SIZE as u64;
    let mut remaining = total;
    let mut absolute = start;
    let mut crc = 0xffff_ffffu32;
    let mut block = [0u8; SECTOR_SIZE];
    while remaining != 0 {
        let n = core::cmp::min(remaining, SECTOR_SIZE as u64) as usize;
        read_absolute(dev, absolute, &mut block[..n])?;
        crc = crc32_update(crc, &block[..n]);
        absolute += n as u64;
        remaining -= n as u64;
    }
    let actual = !crc;
    if actual != header.entries_crc32 {
        return Err(GptError::BadEntryArrayCrc);
    }
    Ok(())
}

/// Читает байтовый диапазон по абсолютному смещению накопителя.
fn read_absolute<D: BlockDevice>(dev: &D, absolute: u64, out: &mut [u8]) -> Result<(), GptError> {
    let mut copied = 0usize;
    while copied < out.len() {
        let pos = absolute
            .checked_add(copied as u64)
            .ok_or(GptError::OutOfBounds)?;
        let lba = pos / SECTOR_SIZE as u64;
        let in_sector = (pos % SECTOR_SIZE as u64) as usize;
        if lba > u32::MAX as u64 {
            return Err(GptError::OutOfBounds);
        }
        let mut block = Block {
            contents: [0u8; SECTOR_SIZE],
        };
        dev.read(core::slice::from_mut(&mut block), BlockIdx(lba as u32))
            .map_err(|_| GptError::Io)?;
        let n = core::cmp::min(SECTOR_SIZE - in_sector, out.len() - copied);
        out[copied..copied + n].copy_from_slice(&block.contents[in_sector..in_sector + n]);
        copied += n;
    }
    Ok(())
}

/// Вычисляет CRC32 для проверки целостности данных.
pub fn crc32(data: &[u8]) -> u32 {
    !crc32_update(0xffff_ffff, data)
}

/// Продолжает CRC32 с переданным промежуточным состоянием.
pub fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    // 64 bytes in flash, two lookups per byte instead of eight bit iterations.
    const TABLE: [u32; 16] = [
        0x00000000, 0x1db71064, 0x3b6e20c8, 0x26d930ac,
        0x76dc4190, 0x6b6b51f4, 0x4db26158, 0x5005713c,
        0xedb88320, 0xf00f9344, 0xd6d6a3e8, 0xcb61b38c,
        0x9b64c2b0, 0x86d3d2d4, 0xa00ae278, 0xbdbdf21c,
    ];
    for &byte in data {
        crc ^= byte as u32;
        crc = (crc >> 4) ^ TABLE[(crc & 15) as usize];
        crc = (crc >> 4) ^ TABLE[(crc & 15) as usize];
    }
    crc
}

/// Читает 32-битное число little-endian из проверенного среза.
fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}
/// Читает 64-битное число little-endian из проверенного среза.
fn le_u64(b: &[u8]) -> u64 {
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}
