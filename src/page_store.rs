//! Managed storage-backed page store for AOSP Wear OS.
//!
//! This is deliberately not presented as transparent MMU swap: ESP32-S3 has
//! no Linux-style virtual-memory pager.  Instead the framework/runtime can
//! explicitly evict serializable app state and caches into fixed 4 KiB pages
//! on the GPT `swap` partition and fault them back in later.

use embedded_sdmmc::BlockDevice;

use crate::gpt::{self, Partition};

pub const PAGE_SIZE: usize = 4096;
const HEADER_SIZE: usize = 16;
const PAYLOAD_SIZE: usize = PAGE_SIZE - HEADER_SIZE;
const MAGIC: [u8; 4] = *b"ZPG1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageStoreError {
    Gpt(gpt::GptError),
    TooLarge,
    BadSlot,
    Empty,
    Corrupt,
}

#[derive(Clone, Copy, Debug)]
pub struct PageStore {
    part: Partition,
    slots: u32,
}

impl PageStore {
    pub fn open<D: BlockDevice>(dev: &D) -> Result<Self, PageStoreError> {
        let part = gpt::find_partition(dev, "swap").map_err(PageStoreError::Gpt)?;
        let pages = part.size_bytes() / PAGE_SIZE as u64;
        if pages < 2 || pages - 1 > u32::MAX as u64 {
            return Err(PageStoreError::BadSlot);
        }
        Ok(Self { part, slots: (pages - 1) as u32 })
    }

    pub fn slots(&self) -> u32 { self.slots }
    pub fn capacity_bytes(&self) -> u64 { self.slots as u64 * PAYLOAD_SIZE as u64 }

    pub fn write<D: BlockDevice>(
        &self,
        dev: &D,
        slot: u32,
        tag: u32,
        payload: &[u8],
    ) -> Result<(), PageStoreError> {
        if slot >= self.slots { return Err(PageStoreError::BadSlot); }
        if payload.len() > PAYLOAD_SIZE { return Err(PageStoreError::TooLarge); }

        let mut page = [0u8; PAGE_SIZE];
        page[0..4].copy_from_slice(&MAGIC);
        page[4..8].copy_from_slice(&tag.to_le_bytes());
        page[8..10].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        page[10..12].copy_from_slice(&1u16.to_le_bytes());
        page[12..16].copy_from_slice(&gpt::crc32(payload).to_le_bytes());
        page[HEADER_SIZE..HEADER_SIZE + payload.len()].copy_from_slice(payload);
        gpt::write_partition(dev, self.part, self.slot_offset(slot), &page)
            .map_err(PageStoreError::Gpt)
    }

    pub fn read<D: BlockDevice>(
        &self,
        dev: &D,
        slot: u32,
        expected_tag: u32,
        out: &mut [u8],
    ) -> Result<usize, PageStoreError> {
        if slot >= self.slots { return Err(PageStoreError::BadSlot); }
        let mut page = [0u8; PAGE_SIZE];
        gpt::read_partition(dev, self.part, self.slot_offset(slot), &mut page)
            .map_err(PageStoreError::Gpt)?;
        if page[0..4] != MAGIC { return Err(PageStoreError::Empty); }
        let tag = u32::from_le_bytes(page[4..8].try_into().unwrap_or([0; 4]));
        if tag != expected_tag { return Err(PageStoreError::Empty); }
        let len = u16::from_le_bytes([page[8], page[9]]) as usize;
        let version = u16::from_le_bytes([page[10], page[11]]);
        if version != 1 || len > PAYLOAD_SIZE || len > out.len() {
            return Err(PageStoreError::Corrupt);
        }
        let crc = u32::from_le_bytes([page[12], page[13], page[14], page[15]]);
        let payload = &page[HEADER_SIZE..HEADER_SIZE + len];
        if gpt::crc32(payload) != crc { return Err(PageStoreError::Corrupt); }
        out[..len].copy_from_slice(payload);
        Ok(len)
    }


    pub fn write_i32<D: BlockDevice>(&self, dev: &D, slot: u32, tag: u32, value: i32) -> Result<(), PageStoreError> {
        self.write(dev, slot, tag, &value.to_le_bytes())
    }

    pub fn read_i32<D: BlockDevice>(&self, dev: &D, slot: u32, tag: u32) -> Result<i32, PageStoreError> {
        let mut out = [0u8; 4];
        let n = self.read(dev, slot, tag, &mut out)?;
        if n != 4 { return Err(PageStoreError::Corrupt); }
        Ok(i32::from_le_bytes(out))
    }

    fn slot_offset(&self, slot: u32) -> u64 {
        // Page zero is reserved for a future allocator journal/bitmap.
        (slot as u64 + 1) * PAGE_SIZE as u64
    }
}
