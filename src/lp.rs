extern crate alloc;

use alloc::vec::Vec;
use embedded_sdmmc::BlockDevice;
use sha2::{Digest, Sha256};

use crate::gpt::{self, Partition};

const GEOMETRY_MAGIC: u32 = 0x616c_4467;
const HEADER_MAGIC: u32 = 0x414c_5030;
const RESERVED_BYTES: u64 = 4096;
const GEOMETRY_SIZE: u64 = 4096;
const GEOMETRY_STRUCT_SIZE: usize = 52;
const HEADER_V1_SIZE: usize = 128;
const HEADER_V12_SIZE: usize = 256;
const TARGET_LINEAR: u32 = 0;

#[derive(Clone, Copy, Debug)]
pub struct LogicalPartition {
    pub first_sector: u64,
    pub num_sectors: u64,
}

impl LogicalPartition {
    pub fn size_bytes(self) -> u64 { self.num_sectors * 512 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LpError {
    Io,
    BadGeometry,
    BadGeometryChecksum,
    BadHeader,
    BadHeaderChecksum,
    BadTablesChecksum,
    Unsupported,
    PartitionNotFound,
    OutOfBounds,
}

pub fn find_logical<D: BlockDevice>(
    dev: &D,
    super_part: Partition,
    metadata_slot: u32,
    wanted: &str,
) -> Result<LogicalPartition, LpError> {
    let geometry = read_geometry(dev, super_part)?;
    if metadata_slot >= geometry.slot_count { return Err(LpError::Unsupported); }

    let metadata_offset = RESERVED_BYTES
        + GEOMETRY_SIZE * 2
        + u64::from(geometry.metadata_max_size) * u64::from(metadata_slot);

    let mut first = [0u8; HEADER_V12_SIZE];
    gpt::read_partition(dev, super_part, metadata_offset, &mut first)
        .map_err(|_| LpError::Io)?;

    if le_u32(&first[0..4]) != HEADER_MAGIC { return Err(LpError::BadHeader); }
    let major = le_u16(&first[4..6]);
    let minor = le_u16(&first[6..8]);
    let header_size = le_u32(&first[8..12]) as usize;
    if major != 10 || minor > 2 || !matches!(header_size, HEADER_V1_SIZE | HEADER_V12_SIZE) {
        return Err(LpError::Unsupported);
    }
    if header_size as u32 > geometry.metadata_max_size { return Err(LpError::BadHeader); }

    let mut header = [0u8; HEADER_V12_SIZE];
    header[..header_size].copy_from_slice(&first[..header_size]);
    let expected_header_hash = &header[12..44];
    let mut check_header = header;
    check_header[12..44].fill(0);
    let actual_header_hash = Sha256::digest(&check_header[..header_size]);
    if actual_header_hash.as_slice() != expected_header_hash {
        return Err(LpError::BadHeaderChecksum);
    }

    let tables_size = le_u32(&header[44..48]) as usize;
    if tables_size == 0
        || header_size.checked_add(tables_size).ok_or(LpError::OutOfBounds)?
            > geometry.metadata_max_size as usize
    {
        return Err(LpError::BadHeader);
    }

    let mut tables = Vec::new();
    tables.resize(tables_size, 0);
    gpt::read_partition(
        dev,
        super_part,
        metadata_offset + header_size as u64,
        &mut tables,
    )
    .map_err(|_| LpError::Io)?;
    if Sha256::digest(&tables).as_slice() != &header[48..80] {
        return Err(LpError::BadTablesChecksum);
    }

    let partitions = table_desc(&header[80..92], tables_size)?;
    let extents = table_desc(&header[92..104], tables_size)?;
    if partitions.entry_size < 52 || extents.entry_size < 24 {
        return Err(LpError::Unsupported);
    }

    for index in 0..partitions.count {
        let entry = table_entry(&tables, partitions, index)?;
        let name_end = entry[..36].iter().position(|b| *b == 0).unwrap_or(36);
        let name = core::str::from_utf8(&entry[..name_end]).map_err(|_| LpError::Unsupported)?;
        if name != wanted { continue; }

        let first_extent = le_u32(&entry[40..44]);
        let num_extents = le_u32(&entry[44..48]);
        // v1 runtime intentionally supports one contiguous linear extent.
        if num_extents != 1 { return Err(LpError::Unsupported); }
        let extent = table_entry(&tables, extents, first_extent)?;
        let num_sectors = le_u64(&extent[0..8]);
        let target_type = le_u32(&extent[8..12]);
        let target_data = le_u64(&extent[12..20]);
        let target_source = le_u32(&extent[20..24]);
        if target_type != TARGET_LINEAR || target_source != 0 || num_sectors == 0 {
            return Err(LpError::Unsupported);
        }
        let end = target_data
            .checked_add(num_sectors)
            .and_then(|v| v.checked_mul(512))
            .ok_or(LpError::OutOfBounds)?;
        if end > super_part.size_bytes() { return Err(LpError::OutOfBounds); }
        return Ok(LogicalPartition { first_sector: target_data, num_sectors });
    }
    Err(LpError::PartitionNotFound)
}

pub fn read_logical<D: BlockDevice>(
    dev: &D,
    super_part: Partition,
    logical: LogicalPartition,
    offset: u64,
    out: &mut [u8],
) -> Result<(), LpError> {
    let end = offset.checked_add(out.len() as u64).ok_or(LpError::OutOfBounds)?;
    if end > logical.size_bytes() { return Err(LpError::OutOfBounds); }
    let base = logical.first_sector.checked_mul(512).ok_or(LpError::OutOfBounds)?;
    gpt::read_partition(dev, super_part, base + offset, out).map_err(|_| LpError::Io)
}

#[derive(Clone, Copy)]
struct Geometry {
    metadata_max_size: u32,
    slot_count: u32,
}

fn read_geometry<D: BlockDevice>(dev: &D, super_part: Partition) -> Result<Geometry, LpError> {
    let mut raw = [0u8; GEOMETRY_STRUCT_SIZE];
    gpt::read_partition(dev, super_part, RESERVED_BYTES, &mut raw).map_err(|_| LpError::Io)?;
    if le_u32(&raw[0..4]) != GEOMETRY_MAGIC
        || le_u32(&raw[4..8]) as usize != GEOMETRY_STRUCT_SIZE
    {
        return Err(LpError::BadGeometry);
    }
    let expected = raw[8..40].to_vec();
    let mut check = raw;
    check[8..40].fill(0);
    if Sha256::digest(check).as_slice() != expected.as_slice() {
        return Err(LpError::BadGeometryChecksum);
    }
    let metadata_max_size = le_u32(&raw[40..44]);
    let slot_count = le_u32(&raw[44..48]);
    let logical_block_size = le_u32(&raw[48..52]);
    if metadata_max_size == 0
        || metadata_max_size % 512 != 0
        || slot_count == 0
        || slot_count > 4
        || logical_block_size == 0
        || logical_block_size % 512 != 0
    {
        return Err(LpError::Unsupported);
    }
    Ok(Geometry { metadata_max_size, slot_count })
}

#[derive(Clone, Copy)]
struct TableDesc { offset: usize, count: u32, entry_size: usize }

fn table_desc(raw: &[u8], tables_size: usize) -> Result<TableDesc, LpError> {
    let offset = le_u32(&raw[0..4]) as usize;
    let count = le_u32(&raw[4..8]);
    let entry_size = le_u32(&raw[8..12]) as usize;
    let bytes = (count as usize).checked_mul(entry_size).ok_or(LpError::OutOfBounds)?;
    if entry_size == 0 || offset.checked_add(bytes).ok_or(LpError::OutOfBounds)? > tables_size {
        return Err(LpError::BadHeader);
    }
    Ok(TableDesc { offset, count, entry_size })
}

fn table_entry<'a>(tables: &'a [u8], desc: TableDesc, index: u32) -> Result<&'a [u8], LpError> {
    if index >= desc.count { return Err(LpError::OutOfBounds); }
    let start = desc.offset + index as usize * desc.entry_size;
    let end = start + desc.entry_size;
    tables.get(start..end).ok_or(LpError::OutOfBounds)
}

fn le_u16(b: &[u8]) -> u16 { u16::from_le_bytes([b[0], b[1]]) }
fn le_u32(b: &[u8]) -> u32 { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) }
fn le_u64(b: &[u8]) -> u64 { u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) }
