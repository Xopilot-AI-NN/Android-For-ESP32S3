extern crate alloc;

use alloc::{string::String, vec::Vec};
use embedded_sdmmc::BlockDevice;

use crate::{
    gpt::Partition,
    lp::{self, LogicalPartition, LpError},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpioError { Io, Invalid, NotFound, TooLarge, Utf8 }

pub fn extract_text<D: BlockDevice>(
    dev: &D,
    super_part: Partition,
    logical: LogicalPartition,
    wanted: &str,
    max_bytes: usize,
) -> Result<String, CpioError> {
    let data = extract_file(dev, super_part, logical, wanted, max_bytes)?;
    String::from_utf8(data).map_err(|_| CpioError::Utf8)
}

pub fn extract_file<D: BlockDevice>(
    dev: &D,
    super_part: Partition,
    logical: LogicalPartition,
    wanted: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, CpioError> {
    let mut cursor = 0u64;
    while cursor + 110 <= logical.size_bytes() {
        let mut header = [0u8; 110];
        read(dev, super_part, logical, cursor, &mut header)?;
        if &header[..6] != b"070701" && &header[..6] != b"070702" {
            return Err(CpioError::Invalid);
        }
        let file_size = parse_hex_u32(&header[54..62]).ok_or(CpioError::Invalid)? as u64;
        let name_size = parse_hex_u32(&header[94..102]).ok_or(CpioError::Invalid)? as u64;
        if name_size == 0 || name_size > 384 { return Err(CpioError::Invalid); }

        let name_offset = cursor + 110;
        let data_offset = align_up(name_offset + name_size, 4);
        if data_offset.checked_add(file_size).ok_or(CpioError::Invalid)? > logical.size_bytes() {
            return Err(CpioError::Invalid);
        }
        let mut name_buf = [0u8; 384];
        read(dev, super_part, logical, name_offset, &mut name_buf[..name_size as usize])?;
        let name_len = name_buf[..name_size as usize]
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(name_size as usize);
        let name = core::str::from_utf8(&name_buf[..name_len]).map_err(|_| CpioError::Invalid)?;
        if name == "TRAILER!!!" { break; }

        if normalize(name) == normalize(wanted) {
            if file_size as usize > max_bytes { return Err(CpioError::TooLarge); }
            let mut out = Vec::new();
            out.resize(file_size as usize, 0);
            read(dev, super_part, logical, data_offset, &mut out)?;
            return Ok(out);
        }
        cursor = align_up(data_offset + file_size, 4);
    }
    Err(CpioError::NotFound)
}

fn read<D: BlockDevice>(
    dev: &D,
    super_part: Partition,
    logical: LogicalPartition,
    offset: u64,
    out: &mut [u8],
) -> Result<(), CpioError> {
    lp::read_logical(dev, super_part, logical, offset, out).map_err(map_lp)
}

fn map_lp(err: LpError) -> CpioError {
    match err { LpError::Io => CpioError::Io, _ => CpioError::Invalid }
}

fn normalize(name: &str) -> &str {
    name.strip_prefix("./").or_else(|| name.strip_prefix('/')).unwrap_or(name)
}

fn parse_hex_u32(input: &[u8]) -> Option<u32> {
    let mut value = 0u32;
    for &b in input {
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => return None,
        };
        value = value.checked_mul(16)?.checked_add(digit as u32)?;
    }
    Some(value)
}

fn align_up(value: u64, align: u64) -> u64 { (value + align - 1) & !(align - 1) }
