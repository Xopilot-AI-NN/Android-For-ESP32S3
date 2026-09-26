use embedded_sdmmc::BlockDevice;
use sha2::{Digest, Sha256};

use crate::gpt::{self, Partition};

const AVB_HEADER_SIZE: u64 = 256;
const AVB_DESCRIPTOR_TAG_HASH: u64 = 2;
const AVB_ALGORITHM_NONE: u32 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AvbError {
    Io,
    BadMagic,
    UnsupportedVersion,
    UnsupportedAlgorithm,
    VerificationDisabled,
    InvalidMetadata,
    DescriptorMissing,
    UnsupportedHash,
    HashMismatch,
    OutOfBounds,
}

#[derive(Clone, Copy, Debug)]
pub struct AvbInfo {
    pub rollback_index: u64,
    pub unsigned_development_vbmeta: bool,
}

pub fn verify_partition<D: BlockDevice>(
    dev: &D,
    vbmeta: Partition,
    target: Partition,
    descriptor_name: &str,
) -> Result<AvbInfo, AvbError> {
    let mut header = [0u8; 256];
    gpt::read_partition(dev, vbmeta, 0, &mut header).map_err(|_| AvbError::Io)?;
    if &header[..4] != b"AVB0" { return Err(AvbError::BadMagic); }

    let major = be_u32(&header[4..8]);
    if major != 1 { return Err(AvbError::UnsupportedVersion); }
    let auth_size = be_u64(&header[12..20]);
    let aux_size = be_u64(&header[20..28]);
    let algorithm = be_u32(&header[28..32]);
    if algorithm != AVB_ALGORITHM_NONE { return Err(AvbError::UnsupportedAlgorithm); }
    let descriptors_offset = be_u64(&header[96..104]);
    let descriptors_size = be_u64(&header[104..112]);
    let rollback_index = be_u64(&header[112..120]);
    let flags = be_u32(&header[120..124]);
    if flags & 0x02 != 0 { return Err(AvbError::VerificationDisabled); }
    if auth_size != 0 { return Err(AvbError::InvalidMetadata); }

    let aux_base = AVB_HEADER_SIZE.checked_add(auth_size).ok_or(AvbError::OutOfBounds)?;
    let aux_end = aux_base.checked_add(aux_size).ok_or(AvbError::OutOfBounds)?;
    let desc_start = aux_base.checked_add(descriptors_offset).ok_or(AvbError::OutOfBounds)?;
    let desc_end = desc_start.checked_add(descriptors_size).ok_or(AvbError::OutOfBounds)?;
    if desc_end > aux_end || aux_end > vbmeta.size_bytes() { return Err(AvbError::OutOfBounds); }

    let mut cursor = desc_start;
    while cursor < desc_end {
        let mut descriptor_header = [0u8; 16];
        gpt::read_partition(dev, vbmeta, cursor, &mut descriptor_header).map_err(|_| AvbError::Io)?;
        let tag = be_u64(&descriptor_header[..8]);
        let following = be_u64(&descriptor_header[8..16]);
        let total = 16u64.checked_add(following).ok_or(AvbError::OutOfBounds)?;
        if total < 16 || cursor + total > desc_end { return Err(AvbError::InvalidMetadata); }

        if tag == AVB_DESCRIPTOR_TAG_HASH {
            if verify_hash_descriptor(dev, vbmeta, cursor, total, target, descriptor_name)? {
                return Ok(AvbInfo { rollback_index, unsigned_development_vbmeta: true });
            }
        }
        cursor += total;
    }
    Err(AvbError::DescriptorMissing)
}

fn verify_hash_descriptor<D: BlockDevice>(
    dev: &D,
    vbmeta: Partition,
    offset: u64,
    total: u64,
    target: Partition,
    descriptor_name: &str,
) -> Result<bool, AvbError> {
    if total < 132 || total > 512 { return Err(AvbError::InvalidMetadata); }
    let mut raw = [0u8; 512];
    gpt::read_partition(dev, vbmeta, offset, &mut raw[..total as usize]).map_err(|_| AvbError::Io)?;

    let image_size = be_u64(&raw[16..24]);
    let algorithm = cstr(&raw[24..56]);
    if algorithm != b"sha256" { return Err(AvbError::UnsupportedHash); }
    let name_len = be_u32(&raw[56..60]) as usize;
    let salt_len = be_u32(&raw[60..64]) as usize;
    let digest_len = be_u32(&raw[64..68]) as usize;
    if digest_len != 32 { return Err(AvbError::InvalidMetadata); }
    let var_start = 132usize;
    let var_end = var_start
        .checked_add(name_len)
        .and_then(|v| v.checked_add(salt_len))
        .and_then(|v| v.checked_add(digest_len))
        .ok_or(AvbError::OutOfBounds)?;
    if var_end > total as usize { return Err(AvbError::InvalidMetadata); }

    let name = &raw[var_start..var_start + name_len];
    if name != descriptor_name.as_bytes() { return Ok(false); }
    if image_size > target.size_bytes() { return Err(AvbError::OutOfBounds); }
    let salt_start = var_start + name_len;
    let digest_start = salt_start + salt_len;
    let salt = &raw[salt_start..digest_start];
    let expected = &raw[digest_start..digest_start + digest_len];

    let mut hasher = Sha256::new();
    hasher.update(salt);
    let mut pos = 0u64;
    let mut chunk = [0u8; 512];
    while pos < image_size {
        let n = core::cmp::min((image_size - pos) as usize, chunk.len());
        gpt::read_partition(dev, target, pos, &mut chunk[..n]).map_err(|_| AvbError::Io)?;
        hasher.update(&chunk[..n]);
        pos += n as u64;
    }
    let actual = hasher.finalize();
    if !constant_time_eq(actual.as_slice(), expected) { return Err(AvbError::HashMismatch); }
    Ok(true)
}

fn cstr(input: &[u8]) -> &[u8] {
    let end = input.iter().position(|b| *b == 0).unwrap_or(input.len());
    &input[..end]
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff = 0u8;
    for i in 0..a.len() { diff |= a[i] ^ b[i]; }
    diff == 0
}

fn be_u32(b: &[u8]) -> u32 { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) }
fn be_u64(b: &[u8]) -> u64 { u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) }
