use embedded_sdmmc::{Block, BlockDevice, BlockIdx};

pub const SECTOR_SIZE: usize = 512;
const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";
const MAX_GPT_ENTRIES: u32 = 256;

#[derive(Clone, Copy, Debug)]
pub struct Partition {
    pub first_lba: u64,
    pub last_lba: u64,
    name: [u8; 36],
    name_len: u8,
}

impl Partition {
    pub fn blocks(self) -> u64 { self.last_lba - self.first_lba + 1 }
    pub fn size_bytes(self) -> u64 { self.blocks() * SECTOR_SIZE as u64 }

    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len as usize]).unwrap_or("?")
    }

    pub fn name_eq(&self, expected: &str) -> bool {
        self.name_len as usize == expected.len()
            && &self.name[..self.name_len as usize] == expected.as_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
struct Header {
    entries_lba: u64,
    entry_count: u32,
    entry_size: u32,
    entries_crc32: u32,
}

pub fn probe<D: BlockDevice>(dev: &D) -> Result<(), GptError> {
    let _ = read_header(dev)?;
    Ok(())
}

pub fn find_partition<D: BlockDevice>(dev: &D, wanted: &str) -> Result<Partition, GptError> {
    let header = read_header(dev)?;
    verify_entry_array_crc(dev, &header)?;

    let mut entry = [0u8; 512];
    for index in 0..header.entry_count {
        let absolute = header
            .entries_lba
            .checked_mul(SECTOR_SIZE as u64)
            .and_then(|v| v.checked_add(index as u64 * header.entry_size as u64))
            .ok_or(GptError::OutOfBounds)?;
        read_absolute(dev, absolute, &mut entry[..header.entry_size as usize])?;

        if entry[..16].iter().all(|b| *b == 0) { continue; }
        let first_lba = le_u64(&entry[32..40]);
        let last_lba = le_u64(&entry[40..48]);
        if first_lba == 0 || last_lba < first_lba { continue; }

        let mut name = [0u8; 36];
        let mut name_len = 0usize;
        for pair in entry[56..128].chunks_exact(2) {
            let cp = u16::from_le_bytes([pair[0], pair[1]]);
            if cp == 0 { break; }
            if cp > 0x7f || name_len == name.len() { break; }
            name[name_len] = cp as u8;
            name_len += 1;
        }

        let partition = Partition {
            first_lba,
            last_lba,
            name,
            name_len: name_len as u8,
        };
        if partition.name_eq(wanted) { return Ok(partition); }
    }
    Err(GptError::PartitionNotFound)
}

pub fn read_partition<D: BlockDevice>(
    dev: &D,
    part: Partition,
    offset: u64,
    out: &mut [u8],
) -> Result<(), GptError> {
    let end = offset.checked_add(out.len() as u64).ok_or(GptError::OutOfBounds)?;
    if end > part.size_bytes() { return Err(GptError::OutOfBounds); }
    let absolute = part
        .first_lba
        .checked_mul(SECTOR_SIZE as u64)
        .and_then(|v| v.checked_add(offset))
        .ok_or(GptError::OutOfBounds)?;
    read_absolute(dev, absolute, out)
}

pub fn write_partition<D: BlockDevice>(
    dev: &D,
    part: Partition,
    offset: u64,
    data: &[u8],
) -> Result<(), GptError> {
    let end = offset.checked_add(data.len() as u64).ok_or(GptError::OutOfBounds)?;
    if end > part.size_bytes() { return Err(GptError::OutOfBounds); }

    let mut copied = 0usize;
    while copied < data.len() {
        let absolute = part.first_lba * SECTOR_SIZE as u64 + offset + copied as u64;
        let lba = absolute / SECTOR_SIZE as u64;
        let in_sector = (absolute % SECTOR_SIZE as u64) as usize;
        if lba > u32::MAX as u64 { return Err(GptError::OutOfBounds); }

        let mut block = Block { contents: [0u8; SECTOR_SIZE] };
        dev.read(core::slice::from_mut(&mut block), BlockIdx(lba as u32))
            .map_err(|_| GptError::Io)?;
        let n = core::cmp::min(SECTOR_SIZE - in_sector, data.len() - copied);
        block.contents[in_sector..in_sector + n].copy_from_slice(&data[copied..copied + n]);
        dev.write(core::slice::from_ref(&block), BlockIdx(lba as u32))
            .map_err(|_| GptError::Io)?;
        copied += n;
    }
    Ok(())
}

fn read_header<D: BlockDevice>(dev: &D) -> Result<Header, GptError> {
    let mut block = Block { contents: [0u8; SECTOR_SIZE] };
    dev.read(core::slice::from_mut(&mut block), BlockIdx(1)).map_err(|_| GptError::Io)?;
    let b = &block.contents;
    if &b[..8] != GPT_SIGNATURE { return Err(GptError::BadSignature); }

    let header_size = le_u32(&b[12..16]) as usize;
    if !(92..=SECTOR_SIZE).contains(&header_size) { return Err(GptError::BadHeader); }
    let expected_crc = le_u32(&b[16..20]);
    let mut header_copy = [0u8; SECTOR_SIZE];
    header_copy[..header_size].copy_from_slice(&b[..header_size]);
    header_copy[16..20].fill(0);
    if crc32(&header_copy[..header_size]) != expected_crc { return Err(GptError::BadHeaderCrc); }

    let entries_lba = le_u64(&b[72..80]);
    let entry_count = le_u32(&b[80..84]);
    let entry_size = le_u32(&b[84..88]);
    let entries_crc32 = le_u32(&b[88..92]);
    if entry_count == 0 || entry_count > MAX_GPT_ENTRIES || entry_size < 128 || entry_size > 512 || entry_size % 8 != 0 {
        return Err(GptError::Unsupported);
    }
    Ok(Header { entries_lba, entry_count, entry_size, entries_crc32 })
}

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
    if actual != header.entries_crc32 { return Err(GptError::BadEntryArrayCrc); }
    Ok(())
}

fn read_absolute<D: BlockDevice>(dev: &D, absolute: u64, out: &mut [u8]) -> Result<(), GptError> {
    let mut copied = 0usize;
    while copied < out.len() {
        let pos = absolute.checked_add(copied as u64).ok_or(GptError::OutOfBounds)?;
        let lba = pos / SECTOR_SIZE as u64;
        let in_sector = (pos % SECTOR_SIZE as u64) as usize;
        if lba > u32::MAX as u64 { return Err(GptError::OutOfBounds); }
        let mut block = Block { contents: [0u8; SECTOR_SIZE] };
        dev.read(core::slice::from_mut(&mut block), BlockIdx(lba as u32))
            .map_err(|_| GptError::Io)?;
        let n = core::cmp::min(SECTOR_SIZE - in_sector, out.len() - copied);
        out[copied..copied + n].copy_from_slice(&block.contents[in_sector..in_sector + n]);
        copied += n;
    }
    Ok(())
}

pub fn crc32(data: &[u8]) -> u32 { !crc32_update(0xffff_ffff, data) }

fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    crc
}

fn le_u32(b: &[u8]) -> u32 { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) }
fn le_u64(b: &[u8]) -> u64 { u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) }
