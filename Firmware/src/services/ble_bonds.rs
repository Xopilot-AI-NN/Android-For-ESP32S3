// Модуль сохранения BLE-ключей сопряжения.
//! Bounded encoding of TrouBLE 0.5.1 BLE bonds for a PageStore payload.
//! The record contains SECRET LTK/IRK bytes; store only in protected storage.
use heapless::Vec;
use trouble_host::prelude::{
    BdAddr, BondInformation, Identity, IdentityResolvingKey, LongTermKey, SecurityLevel,
};

/// Верхняя граница BONDS для ограничения размера или количества данных.
pub const MAX_BONDS: usize = 4;
/// Верхняя граница BOND_BLOB для ограничения размера или количества данных.
pub const MAX_BOND_BLOB: usize = 6 + MAX_BONDS * 41;
/// Сигнатура, отличающая собственный формат данных от чужих.
const MAGIC: [u8; 4] = *b"BND1";
/// Размер сериализованной записи каталога, байт.
const RECORD: usize = 41;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Ошибки операций компонента сохранения BLE-ключей сопряжения.
pub enum BondCodecError {
    BadFormat,
    TooManyBonds,
    BufferTooSmall,
}

/// Сериализует ограниченный набор BLE-ключей сопряжения.
pub fn encode_bonds(
    bonds: &Vec<BondInformation, MAX_BONDS>,
    out: &mut [u8],
) -> Result<usize, BondCodecError> {
    let size = 6 + bonds.len() * RECORD;
    if out.len() < size {
        return Err(BondCodecError::BufferTooSmall);
    }
    out[..size].fill(0);
    out[..4].copy_from_slice(&MAGIC);
    out[4] = 1;
    out[5] = bonds.len() as u8;
    for (i, b) in bonds.iter().enumerate() {
        let p = 6 + i * RECORD;
        out[p..p + 6].copy_from_slice(&b.identity.bd_addr.into_inner());
        if let Some(irk) = b.identity.irk {
            out[p + 6] = 1;
            out[p + 7..p + 23].copy_from_slice(&irk.to_le_bytes());
        }
        out[p + 23..p + 39].copy_from_slice(&b.ltk.to_le_bytes());
        out[p + 39] = match b.security_level {
            SecurityLevel::NoEncryption => 0,
            SecurityLevel::Encrypted => 1,
            SecurityLevel::EncryptedAuthenticated => 2,
        };
        out[p + 40] = u8::from(b.is_bonded);
    }
    Ok(size)
}

/// Проверяет формат и восстанавливает BLE-ключи сопряжения.
pub fn decode_bonds(bytes: &[u8]) -> Result<Vec<BondInformation, MAX_BONDS>, BondCodecError> {
    if bytes.len() < 6 || bytes[..4] != MAGIC || bytes[4] != 1 {
        return Err(BondCodecError::BadFormat);
    }
    let n = bytes[5] as usize;
    if n > MAX_BONDS {
        return Err(BondCodecError::TooManyBonds);
    }
    if bytes.len() != 6 + n * RECORD {
        return Err(BondCodecError::BadFormat);
    }
    let mut result = Vec::new();
    for i in 0..n {
        let p = 6 + i * RECORD;
        if bytes[p + 6] > 1 || bytes[p + 39] > 2 || bytes[p + 40] > 1 {
            return Err(BondCodecError::BadFormat);
        }
        let mut addr = [0; 6];
        addr.copy_from_slice(&bytes[p..p + 6]);
        let mut irk_bytes = [0; 16];
        irk_bytes.copy_from_slice(&bytes[p + 7..p + 23]);
        let mut key_bytes = [0; 16];
        key_bytes.copy_from_slice(&bytes[p + 23..p + 39]);
        let identity = Identity {
            bd_addr: BdAddr::new(addr),
            irk: if bytes[p + 6] == 1 {
                Some(IdentityResolvingKey::from_le_bytes(irk_bytes))
            } else {
                None
            },
        };
        let level = match bytes[p + 39] {
            0 => SecurityLevel::NoEncryption,
            1 => SecurityLevel::Encrypted,
            _ => SecurityLevel::EncryptedAuthenticated,
        };
        result
            .push(BondInformation::new(
                identity,
                LongTermKey::from_le_bytes(key_bytes),
                level,
                bytes[p + 40] == 1,
            ))
            .map_err(|_| BondCodecError::TooManyBonds)?;
    }
    Ok(result)
}
