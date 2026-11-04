// Модуль страничного хранилища ZPager.
//! Managed storage-backed page store for AOSP Wear OS.
//!
//! This is deliberately not presented as transparent MMU swap: ESP32-S3 has
//! no Linux-style virtual-memory pager.  Instead the framework/runtime can
//! explicitly evict serializable app state and caches into fixed 4 KiB pages
//! on the GPT `swap` partition and fault them back in later.

use embedded_sdmmc::BlockDevice;

use crate::gpt::{self, Partition};

/// Параметр PAGE_SIZE компонента страничного хранилища ZPager.
pub const PAGE_SIZE: usize = 4096;
/// Параметр HEADER_SIZE компонента страничного хранилища ZPager.
const HEADER_SIZE: usize = 16;
/// Параметр PAYLOAD_SIZE компонента страничного хранилища ZPager.
const PAYLOAD_SIZE: usize = PAGE_SIZE - HEADER_SIZE;
/// Сигнатура, отличающая собственный формат данных от чужих.
const MAGIC: [u8; 4] = *b"ZPG1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Ошибки операций компонента страничного хранилища ZPager.
pub enum PageStoreError {
    Gpt(gpt::GptError),
    TooLarge,
    BadSlot,
    Empty,
    Corrupt,
}

#[derive(Clone, Copy, Debug)]
/// Страничное хранилище фиксированных слотов поверх GPT компонента страничного хранилища ZPager.
pub struct PageStore {
    /// Границы физического раздела, которому принадлежит область.
    part: Partition,
    /// Количество адресуемых страниц или слотов.
    slots: u32,
    /// Количество страниц основного раздела swap.
    base_slots: u32,
    /// Необязательное расширение хранилища в зарезервированном userdata.
    extra: Option<Partition>,
}

impl PageStore {
    /// Открывает базовый swap и подключает разрешённое расширение userdata.
    pub fn open<D: BlockDevice>(dev: &D) -> Result<Self, PageStoreError> {
        let mut store = Self::open_base(dev)?;
        if let Some((part, slots)) = crate::swap_config::restore(dev, &store) {
            store.extra = Some(part);
            store.slots += slots;
        }
        Ok(store)
    }

    /// Открывает базовый swap без расширения в userdata.
    pub(crate) fn open_base<D: BlockDevice>(dev: &D) -> Result<Self, PageStoreError> {
        let part = gpt::find_partition(dev, "swap").map_err(PageStoreError::Gpt)?;
        let pages = part.size_bytes() / PAGE_SIZE as u64;
        if pages < 2 || pages - 1 > u32::MAX as u64 {
            return Err(PageStoreError::BadSlot);
        }
        Ok(Self {
            part,
            slots: (pages - 1) as u32,
            base_slots: (pages - 1) as u32,
            extra: None,
        })
    }

    /// Возвращает размер базового раздела в МиБ.
    pub fn base_mib(&self) -> u16 {
        (self.part.size_bytes() / 1048576) as u16
    }
    /// Возвращает суммарный объём страничного хранилища в МиБ.
    pub fn backing_mib(&self) -> u16 {
        self.base_mib()
            + (((self.slots - self.base_slots) as u64 * PAGE_SIZE as u64) / 1048576) as u16
    }

    /// Возвращает число доступных файловых слотов/страниц.
    pub fn slots(&self) -> u32 {
        self.slots
    }
    /// Возвращает полезную ёмкость хранилища в байтах.
    pub fn capacity_bytes(&self) -> u64 {
        self.slots as u64 * PAYLOAD_SIZE as u64
    }

    /// Сохраняет помеченный payload и CRC в фиксированный слот без записи лишнего хвоста.
    pub fn write<D: BlockDevice>(
        &self,
        dev: &D,
        slot: u32,
        tag: u32,
        payload: &[u8],
    ) -> Result<(), PageStoreError> {
        if slot >= self.slots {
            return Err(PageStoreError::BadSlot);
        }
        if payload.len() > PAYLOAD_SIZE {
            return Err(PageStoreError::TooLarge);
        }

        let used = (HEADER_SIZE + payload.len()).div_ceil(gpt::SECTOR_SIZE) * gpt::SECTOR_SIZE;
        let mut sector = [0u8; gpt::SECTOR_SIZE];
        let mut large = alloc::vec::Vec::new();
        let page = if used == gpt::SECTOR_SIZE {
            &mut sector[..]
        } else {
            large.resize(used, 0);
            &mut large[..]
        };
        page[0..4].copy_from_slice(&MAGIC);
        page[4..8].copy_from_slice(&tag.to_le_bytes());
        page[8..10].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        page[10..12].copy_from_slice(&1u16.to_le_bytes());
        page[12..16].copy_from_slice(&gpt::crc32(payload).to_le_bytes());
        page[HEADER_SIZE..HEADER_SIZE + payload.len()].copy_from_slice(payload);
        // Slot spacing and record format stay unchanged. A four-byte cursor
        // record needs one sector, not an entire 4 KiB transfer over USB/SD.
        let (part, offset) = self.slot_location(slot);
        gpt::write_partition(dev, part, offset, &page[..used]).map_err(PageStoreError::Gpt)
    }

    /// Проверяет тег, версию, длину и CRC страницы перед возвратом payload.
    pub fn read<D: BlockDevice>(
        &self,
        dev: &D,
        slot: u32,
        expected_tag: u32,
        out: &mut [u8],
    ) -> Result<usize, PageStoreError> {
        if slot >= self.slots {
            return Err(PageStoreError::BadSlot);
        }
        let mut sector = [0u8; gpt::SECTOR_SIZE];
        let mut page = &mut sector[..];
        let mut large = alloc::vec::Vec::new();
        let (part, offset) = self.slot_location(slot);
        gpt::read_partition(dev, part, offset, &mut page[..gpt::SECTOR_SIZE])
            .map_err(PageStoreError::Gpt)?;
        if page[0..4] != MAGIC {
            return Err(PageStoreError::Empty);
        }
        let tag = u32::from_le_bytes(page[4..8].try_into().unwrap_or([0; 4]));
        if tag != expected_tag {
            return Err(PageStoreError::Empty);
        }
        let len = u16::from_le_bytes([page[8], page[9]]) as usize;
        let version = u16::from_le_bytes([page[10], page[11]]);
        if version != 1 || len > PAYLOAD_SIZE || len > out.len() {
            return Err(PageStoreError::Corrupt);
        }
        let used = (HEADER_SIZE + len).div_ceil(gpt::SECTOR_SIZE) * gpt::SECTOR_SIZE;
        if used > gpt::SECTOR_SIZE {
            large.resize(used, 0);
            large[..gpt::SECTOR_SIZE].copy_from_slice(page);
            page = &mut large[..];
            gpt::read_partition(
                dev,
                part,
                offset + gpt::SECTOR_SIZE as u64,
                &mut page[gpt::SECTOR_SIZE..used],
            )
            .map_err(PageStoreError::Gpt)?;
        }
        let crc = u32::from_le_bytes([page[12], page[13], page[14], page[15]]);
        let payload = &page[HEADER_SIZE..HEADER_SIZE + len];
        if gpt::crc32(payload) != crc {
            return Err(PageStoreError::Corrupt);
        }
        out[..len].copy_from_slice(payload);
        Ok(len)
    }

    /// Сохраняет одно i32 как помеченную запись страницы.
    pub fn write_i32<D: BlockDevice>(
        &self,
        dev: &D,
        slot: u32,
        tag: u32,
        value: i32,
    ) -> Result<(), PageStoreError> {
        self.write(dev, slot, tag, &value.to_le_bytes())
    }

    /// Читает i32 и проверяет формат помеченной записи.
    pub fn read_i32<D: BlockDevice>(
        &self,
        dev: &D,
        slot: u32,
        tag: u32,
    ) -> Result<i32, PageStoreError> {
        let mut out = [0u8; 4];
        let n = self.read(dev, slot, tag, &mut out)?;
        if n != 4 {
            return Err(PageStoreError::Corrupt);
        }
        Ok(i32::from_le_bytes(out))
    }

    /// Определяет раздел и смещение базовой либо расширенной страницы.
    fn slot_location(&self, slot: u32) -> (Partition, u64) {
        if slot >= self.base_slots {
            if let Some(part) = self.extra {
                return (
                    part,
                    crate::swap_config::EXTRA_OFFSET
                        + u64::from(slot - self.base_slots) * PAGE_SIZE as u64,
                );
            }
        }
        (self.part, self.slot_offset(slot))
    }

    /// Вычисляет смещение страницы, учитывая зарезервированную нулевую страницу.
    fn slot_offset(&self, slot: u32) -> u64 {
        // Page zero is reserved for a future allocator journal/bitmap.
        (slot as u64 + 1) * PAGE_SIZE as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{
        cell::{Cell, RefCell},
        convert::Infallible,
    };
    use embedded_sdmmc::{Block, BlockCount, BlockIdx};
    /// Модель блочного устройства для host-проверок компонента страничного хранилища ZPager.
    struct Disk {
        /// Содержимое текущего пакета, страницы или модели.
        data: RefCell<alloc::vec::Vec<[u8; 512]>>,
        reads: Cell<usize>,
        writes: Cell<usize>,
    }
    impl BlockDevice for Disk {
        /// Причины отказа операций компонента страничного хранилища ZPager.
        type Error = Infallible;
        /// Реализует секторное чтение для данного блочного backend.
        fn read(&self, b: &mut [Block], idx: BlockIdx) -> Result<(), Self::Error> {
            self.reads.set(self.reads.get() + b.len());
            for (i, x) in b.iter_mut().enumerate() {
                x.contents = self.data.borrow()[idx.0 as usize + i];
            }
            Ok(())
        }
        /// Реализует секторную запись для данного backend с его ограничениями.
        fn write(&self, b: &[Block], idx: BlockIdx) -> Result<(), Self::Error> {
            self.writes.set(self.writes.get() + b.len());
            for (i, x) in b.iter().enumerate() {
                self.data.borrow_mut()[idx.0 as usize + i] = x.contents;
            }
            Ok(())
        }
        /// Возвращает ёмкость блочного устройства в секторах по 512 байт.
        fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
            Ok(BlockCount(256))
        }
    }
    /// Создаёт изолированный тестовый накопитель с ожидаемой разметкой.
    fn fixture() -> (Disk, PageStore) {
        let mut data = alloc::vec![[0u8;512];256];
        let entry = &mut data[2];
        entry[..16].fill(1);
        entry[32..40].copy_from_slice(&64u64.to_le_bytes());
        entry[40..48].copy_from_slice(&255u64.to_le_bytes());
        for (i, b) in b"swap".iter().enumerate() {
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
        let checksum = gpt::crc32(&h[..92]);
        h[16..20].copy_from_slice(&checksum.to_le_bytes());
        let d = Disk {
            data: RefCell::new(data),
            reads: Cell::new(0),
            writes: Cell::new(0),
        };
        let p = PageStore::open(&d).unwrap();
        d.reads.set(0);
        d.writes.set(0);
        (d, p)
    }
    #[test]
    fn sector_boundaries_and_full_page_preserve_format() {
        let (disk, pager) = fixture();
        for len in [0, 1, 496, 497, 1000, 4080] {
            let payload: alloc::vec::Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            pager.write(&disk, 0, 19, &payload).unwrap();
            let mut out = alloc::vec![0xa5; 4080];
            assert_eq!(pager.read(&disk, 0, 19, &mut out).unwrap(), len);
            assert_eq!(&out[..len], payload.as_slice());
            assert!(out[len..].iter().all(|b| *b == 0xa5));
        }
        assert_eq!(
            pager.write(&disk, 0, 19, &[0; 4081]),
            Err(PageStoreError::TooLarge)
        );
    }
    #[test]
    /// Проверяет сокращение секторных операций для небольшой записи курсора.
    fn cursor_record_uses_two_sector_transfers_instead_of_twenty_four() {
        let (d, p) = fixture();
        p.write_i32(&d, 0, 0x77, 123).unwrap();
        assert_eq!(d.reads.get(), 0);
        assert_eq!(d.writes.get(), 1);
        assert_eq!(p.read_i32(&d, 0, 0x77).unwrap(), 123);
        assert_eq!(d.reads.get(), 1);
        assert_eq!(d.data.borrow()[73], [0; 512]); // Rest of the slot remains untouched.
    }
    #[test]
    /// Проверяет длинные и сокращённые страницы, их формат и контрольную сумму.
    fn long_records_and_shrinking_records_preserve_format_and_crc() {
        let (d, p) = fixture();
        let payload = alloc::vec![0x5a;1000];
        p.write(&d, 0, 1, &payload).unwrap();
        assert_eq!(d.writes.get(), 2);
        let mut out = [0; 1000];
        assert_eq!(p.read(&d, 0, 1, &mut out).unwrap(), 1000);
        assert_eq!(out.as_slice(), payload.as_slice());
        assert_eq!(d.reads.get(), 2);
        p.write(&d, 0, 1, b"new").unwrap();
        out.fill(0xa5);
        assert_eq!(p.read(&d, 0, 1, &mut out).unwrap(), 3);
        assert_eq!(&out[..3], b"new");
        assert!(out[3..].iter().all(|&b| b == 0xa5));
        d.data.borrow_mut()[72][16] ^= 1;
        assert_eq!(p.read(&d, 0, 1, &mut out), Err(PageStoreError::Corrupt));
    }
    #[test]
    /// Проверяет сохранение соседних байтов и отказ без чтения хвоста при неверной длине.
    fn partial_sector_writes_keep_neighbors_and_invalid_lengths_do_not_fetch_tail() {
        let (d, p) = fixture();
        p.write_i32(&d, 0, 1, 123).unwrap();
        let before = d.data.borrow()[72];
        gpt::write_partition(&d, p.part, p.slot_offset(0) + 100, &[1, 2, 3]).unwrap();
        let after = d.data.borrow()[72];
        assert_eq!(&before[..100], &after[..100]);
        assert_eq!(&after[100..103], &[1, 2, 3]);
        assert_eq!(&before[103..], &after[103..]);
        d.data.borrow_mut()[72][8..10].copy_from_slice(&u16::MAX.to_le_bytes());
        d.reads.set(0);
        let mut out = [0; 10];
        assert_eq!(p.read(&d, 0, 1, &mut out), Err(PageStoreError::Corrupt));
        assert_eq!(d.reads.get(), 1);
    }
}
