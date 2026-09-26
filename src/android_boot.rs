extern crate alloc;

use alloc::{string::{String, ToString}, vec::Vec};
use embedded_sdmmc::BlockDevice;

use crate::{config::MAX_SCRIPT_BYTES, gpt::{self, Partition}};

const BOOT_MAGIC: &[u8; 8] = b"ANDROID!";
const VENDOR_BOOT_MAGIC: &[u8; 8] = b"VNDRBOOT";
const BOOT_PAGE_SIZE: u64 = 4096;
const BOOT_HEADER_V4_SIZE: u32 = 1584;
const VENDOR_BOOT_HEADER_V4_SIZE: u32 = 2128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootImageError {
    Io,
    BadMagic,
    UnsupportedHeader,
    InvalidLayout,
    RamdiskMissing,
    Cpio,
    InitMissing,
    ScriptTooLarge,
    Utf8,
}

#[derive(Clone, Debug)]
pub struct BootImageInfo {
    pub header_version: u32,
    pub os_version: u32,
    pub ramdisk_offset: u64,
    pub ramdisk_size: u32,
    pub cmdline_version: String,
}

pub fn validate_boot<D: BlockDevice>(dev: &D, part: Partition) -> Result<BootImageInfo, BootImageError> {
    let mut header = [0u8; 2048];
    gpt::read_partition(dev, part, 0, &mut header).map_err(|_| BootImageError::Io)?;
    if &header[..8] != BOOT_MAGIC { return Err(BootImageError::BadMagic); }
    let kernel_size = le_u32(&header[8..12]);
    let ramdisk_size = le_u32(&header[12..16]);
    let os_version = le_u32(&header[16..20]);
    let header_size = le_u32(&header[20..24]);
    let header_version = le_u32(&header[40..44]);
    if !(3..=4).contains(&header_version) || header_size < 1580 || header_size > BOOT_PAGE_SIZE as u32 {
        return Err(BootImageError::UnsupportedHeader);
    }
    if header_version == 4 && header_size < BOOT_HEADER_V4_SIZE { return Err(BootImageError::UnsupportedHeader); }
    let ramdisk_offset = BOOT_PAGE_SIZE + align_up(kernel_size as u64, BOOT_PAGE_SIZE);
    if ramdisk_offset + ramdisk_size as u64 > part.size_bytes() { return Err(BootImageError::InvalidLayout); }
    let cmdline_version = cmdline_value(&header[44..1580], b"androidboot.zephyr.version=")
        .unwrap_or_else(|| "0.0.0-dev".to_string());
    Ok(BootImageInfo { header_version, os_version, ramdisk_offset, ramdisk_size, cmdline_version })
}

pub fn validate_vendor_boot<D: BlockDevice>(dev: &D, part: Partition) -> Result<(), BootImageError> {
    let mut header = [0u8; VENDOR_BOOT_HEADER_V4_SIZE as usize];
    gpt::read_partition(dev, part, 0, &mut header).map_err(|_| BootImageError::Io)?;
    if &header[..8] != VENDOR_BOOT_MAGIC { return Err(BootImageError::BadMagic); }
    let header_version = le_u32(&header[8..12]);
    let page_size = le_u32(&header[12..16]);
    let header_size = le_u32(&header[2096..2100]);
    if header_version != 4
        || page_size != BOOT_PAGE_SIZE as u32
        || header_size != VENDOR_BOOT_HEADER_V4_SIZE
        || part.size_bytes() < BOOT_PAGE_SIZE
    {
        return Err(BootImageError::UnsupportedHeader);
    }
    Ok(())
}

pub fn extract_init_script<D: BlockDevice>(
    dev: &D,
    init_boot: Partition,
) -> Result<(String, String), BootImageError> {
    let info = validate_boot(dev, init_boot)?;
    if info.ramdisk_size == 0 { return Err(BootImageError::RamdiskMissing); }
    let script = extract_newc_file(dev, init_boot, info.ramdisk_offset, info.ramdisk_size as u64, "init.rhai")?;
    let script = String::from_utf8(script).map_err(|_| BootImageError::Utf8)?;
    Ok((info.cmdline_version, script))
}

fn extract_newc_file<D: BlockDevice>(
    dev: &D,
    part: Partition,
    base: u64,
    size: u64,
    wanted: &str,
) -> Result<Vec<u8>, BootImageError> {
    let mut cursor = 0u64;
    while cursor + 110 <= size {
        let mut header = [0u8; 110];
        gpt::read_partition(dev, part, base + cursor, &mut header).map_err(|_| BootImageError::Io)?;
        if &header[..6] != b"070701" && &header[..6] != b"070702" { return Err(BootImageError::Cpio); }
        let file_size = parse_hex_u32(&header[54..62]).ok_or(BootImageError::Cpio)? as u64;
        let name_size = parse_hex_u32(&header[94..102]).ok_or(BootImageError::Cpio)? as u64;
        if name_size == 0 || name_size > 256 { return Err(BootImageError::Cpio); }

        let name_offset = cursor + 110;
        let data_offset = align_up(name_offset + name_size, 4);
        if data_offset + file_size > size { return Err(BootImageError::Cpio); }
        let mut name_buf = [0u8; 256];
        gpt::read_partition(dev, part, base + name_offset, &mut name_buf[..name_size as usize])
            .map_err(|_| BootImageError::Io)?;
        let name_len = name_buf[..name_size as usize].iter().position(|b| *b == 0).unwrap_or(name_size as usize);
        let name = core::str::from_utf8(&name_buf[..name_len]).map_err(|_| BootImageError::Cpio)?;
        if name == "TRAILER!!!" { break; }

        if normalize_name(name) == wanted {
            if file_size as usize > MAX_SCRIPT_BYTES { return Err(BootImageError::ScriptTooLarge); }
            let mut out = Vec::with_capacity(file_size as usize);
            out.resize(file_size as usize, 0);
            gpt::read_partition(dev, part, base + data_offset, &mut out).map_err(|_| BootImageError::Io)?;
            return Ok(out);
        }
        cursor = align_up(data_offset + file_size, 4);
    }
    Err(BootImageError::InitMissing)
}

fn normalize_name(name: &str) -> &str {
    name.strip_prefix("./").or_else(|| name.strip_prefix('/')).unwrap_or(name)
}

fn cmdline_value(cmdline: &[u8], key: &[u8]) -> Option<String> {
    let end = cmdline.iter().position(|b| *b == 0).unwrap_or(cmdline.len());
    let line = &cmdline[..end];
    for token in line.split(|b| *b == b' ') {
        if token.starts_with(key) {
            return core::str::from_utf8(&token[key.len()..]).ok().map(ToString::to_string);
        }
    }
    None
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
fn le_u32(b: &[u8]) -> u32 { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) }
