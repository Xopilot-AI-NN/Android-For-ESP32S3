// Модуль настройки объёма ZPager.
//! Adjustable ZPager backing without moving GPT partitions or existing settings.
use crate::{
    gpt::{self, Partition},
    page_store::PageStore,
};
use core::cell::RefCell;
use critical_section::Mutex;
use embedded_sdmmc::BlockDevice;
/// Параметр CHOICES компонента настройки объёма ZPager.
pub const CHOICES: [u16; 5] = [32, 64, 128, 256, 512];
/// Количество байтов в одном МиБ.
const MIB: u64 = 1024 * 1024;
/// Постоянный номер страницы CONFIG; не пересекается с Activity-слотами.
const CONFIG_SLOT: u32 = 73;
/// Тег формата записи CONFIG для проверки типа данных.
const CONFIG_TAG: u32 = 0x5357_5043;
/// Параметр MARKER_OFFSET компонента настройки объёма ZPager.
const MARKER_OFFSET: u64 = 4 * MIB;
/// Параметр EXTRA_OFFSET компонента настройки объёма ZPager.
pub const EXTRA_OFFSET: u64 = 8 * MIB;
/// Параметр KEEP_USER_BYTES компонента настройки объёма ZPager.
const KEEP_USER_BYTES: u64 = crate::file_store::BLOB_RESERVE;
/// Сигнатура, отличающая собственный формат данных от чужих.
const MAGIC: &[u8; 8] = b"SWEXv1\0\0";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Согласованный снимок состояния компонента настройки объёма ZPager.
pub struct Snapshot {
    /// Признак активности режима.
    pub active: u16,
    /// Выбранный элемент либо признак его выделения.
    pub selected: u16,
    /// Максимально допустимый размер, рассчитанный по носителю.
    pub maximum: u16,
    /// Сообщение или его код для вывода пользователю.
    pub message: u8,
}
impl Default for Snapshot {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            active: 32,
            selected: 32,
            maximum: 32,
            message: 0,
        }
    }
}
/// Состояние очереди запросов и результатов компонента настройки объёма ZPager.
struct Control {
    /// Последний подготовленный снимок состояния.
    snapshot: Snapshot,
    /// Необработанный запрос или ожидающий результат.
    pending: Option<u16>,
}
/// Общая очередь операций и результатов; I/O выполняется вне блокировки.
static CONTROL: Mutex<RefCell<Control>> = Mutex::new(RefCell::new(Control {
    snapshot: Snapshot {
        active: 32,
        selected: 32,
        maximum: 32,
        message: 0,
    },
    pending: None,
}));
/// Возвращает согласованный снимок текущего состояния.
pub fn snapshot() -> Snapshot {
    critical_section::with(|cs| CONTROL.borrow_ref(cs).snapshot)
}
/// Ставит запрос в очередь для обработки владельцем ресурса.
pub fn request(size: u16) {
    critical_section::with(|cs| {
        let mut control = CONTROL.borrow_ref_mut(cs);
        if !CHOICES.contains(&size) || size > control.snapshot.maximum {
            control.snapshot.message = 3;
        } else {
            control.pending = Some(size);
            control.snapshot.message = 1;
        }
    });
}
/// Читает сохранённую настройку с проверкой допустимости.
fn saved<D: BlockDevice>(dev: &D, base: &PageStore) -> u16 {
    let mut bytes = [0; 4];
    match base.read(dev, CONFIG_SLOT, CONFIG_TAG, &mut bytes) {
        Ok(4) if bytes[0] == 1 && bytes[3] == 0 => {
            let size = u16::from_le_bytes([bytes[1], bytes[2]]);
            if CHOICES.contains(&size) { size } else { 32 }
        }
        _ => 32,
    }
}
/// Own-format userdata only. A filesystem or foreign reservation is never reused.
fn inspect<D: BlockDevice>(dev: &D) -> Result<(Partition, u64), ()> {
    let part = gpt::find_partition(dev, "userdata").map_err(|_| ())?;
    if part.size_bytes() < EXTRA_OFFSET + KEEP_USER_BYTES {
        return Err(());
    }
    let mut prefix = [0; 512];
    gpt::read_partition(dev, part, 0, &mut prefix).map_err(|_| ())?;
    if prefix.iter().any(|&v| v != 0) && &prefix[..8] != b"WALLv1\0\0" {
        return Err(());
    }
    if prefix.iter().all(|&v| v == 0) {
        // ext filesystems can have an empty boot sector; inspect their superblock.
        let mut superblock = [0; 512];
        gpt::read_partition(dev, part, 1024, &mut superblock).map_err(|_| ())?;
        if &superblock[56..58] == b"\x53\xef" || &superblock[..4] == b"\x10\x20\xf5\xf2" {
            return Err(());
        }
    }
    let mut marker = [0; 512];
    gpt::read_partition(dev, part, MARKER_OFFSET, &mut marker).map_err(|_| ())?;
    if marker.iter().all(|&v| v == 0) {
        return Ok((part, 0));
    }
    if &marker[..8] != MAGIC
        || u64::from_le_bytes(marker[8..16].try_into().map_err(|_| ())?) != EXTRA_OFFSET
        || gpt::crc32(&marker[..24])
            != u32::from_le_bytes(marker[24..28].try_into().map_err(|_| ())?)
    {
        return Err(());
    }
    let reserved = u64::from_le_bytes(marker[16..24].try_into().map_err(|_| ())?);
    if reserved > 480 * MIB || EXTRA_OFFSET + reserved + KEEP_USER_BYTES > part.size_bytes() {
        return Err(());
    }
    Ok((part, reserved))
}
/// Определяет максимальный допустимый размер по ёмкости раздела.
fn maximum<D: BlockDevice>(dev: &D, part: Partition) -> u16 {
    let pool = crate::file_store::Fs::mount(dev)
        .ok()
        .and_then(|fs| fs.saved_blob_base())
        .unwrap_or_else(|| part.size_bytes().saturating_sub(KEEP_USER_BYTES) / 65536 * 65536);
    CHOICES
        .into_iter()
        .rev()
        .find(|&m| EXTRA_OFFSET + u64::from(m - 32) * MIB <= pool)
        .unwrap_or(32)
}
/// Called by PageStore::open; persistent slots always stay on the base partition.
pub fn restore<D: BlockDevice>(dev: &D, base: &PageStore) -> Option<(Partition, u32)> {
    if base.base_mib() != 32 {
        return None;
    }
    let selected = saved(dev, base);
    if selected == 32 {
        return None;
    }
    let (part, reserved) = inspect(dev).ok()?;
    let extra = u64::from(selected - 32) * MIB;
    if selected > maximum(dev, part) || extra > reserved {
        return None;
    }
    Some((part, (extra / crate::page_store::PAGE_SIZE as u64) as u32))
}
/// Cache capabilities once at boot; no storage probing runs while the watch idles.
pub fn initialize<D: BlockDevice>(dev: &D, pager: Option<&PageStore>) {
    let mut state = Snapshot::default();
    if let Some(pager) = pager {
        state.active = pager.backing_mib();
        state.selected = saved(dev, pager);
        if pager.base_mib() == 32 {
            if let Ok((part, _)) = inspect(dev) {
                state.maximum = maximum(dev, part);
            }
        }
        if state.selected != state.active {
            state.message = 4;
        }
    } else {
        state.maximum = 0;
        state.active = 0;
        state.selected = 0;
        state.message = 4;
    }
    critical_section::with(|cs| CONTROL.borrow_ref_mut(cs).snapshot = state);
}
/// Проверяет и сохраняет выбранную настройку.
fn apply<D: BlockDevice>(dev: &D, size: u16) -> Result<(), u8> {
    let base = PageStore::open_base(dev).map_err(|_| 4)?;
    if base.base_mib() != 32 {
        return Err(4);
    }
    if size > 32 {
        let (part, reserved) = inspect(dev).map_err(|_| 4)?;
        if size > maximum(dev, part) {
            return Err(3);
        }
        let mut marker = [0; 512];
        marker[..8].copy_from_slice(MAGIC);
        marker[8..16].copy_from_slice(&EXTRA_OFFSET.to_le_bytes());
        // Shrinking never releases an already reserved range to unrelated data.
        marker[16..24].copy_from_slice(&reserved.max(u64::from(size - 32) * MIB).to_le_bytes());
        let crc = gpt::crc32(&marker[..24]);
        marker[24..28].copy_from_slice(&crc.to_le_bytes());
        gpt::write_partition(dev, part, MARKER_OFFSET, &marker).map_err(|_| 5)?;
        let mut check = [0; 512];
        gpt::read_partition(dev, part, MARKER_OFFSET, &mut check).map_err(|_| 5)?;
        if marker != check {
            return Err(5);
        }
    }
    let [lo, hi] = size.to_le_bytes();
    base.write(dev, CONFIG_SLOT, CONFIG_TAG, &[1, lo, hi, 0])
        .map_err(|_| 5)?;
    let mut check = [0; 4];
    if base
        .read(dev, CONFIG_SLOT, CONFIG_TAG, &mut check)
        .map_err(|_| 5)?
        != 4
        || check != [1, lo, hi, 0]
    {
        return Err(5);
    }
    Ok(())
}
/// Apply one explicit UI request through the existing main-loop disk owner.
#[inline(never)]
pub fn service<D: BlockDevice>(dev: &D) -> bool {
    let Some(size) = critical_section::with(|cs| CONTROL.borrow_ref_mut(cs).pending.take()) else {
        return false;
    };
    let result = apply(dev, size);
    critical_section::with(|cs| {
        let mut control = CONTROL.borrow_ref_mut(cs);
        match result {
            Ok(()) => {
                control.snapshot.selected = size;
                control.snapshot.message = if size == control.snapshot.active {
                    0
                } else {
                    2
                };
            }
            Err(reason) => control.snapshot.message = reason,
        }
    });
    crate::watch_println!(
        "ZPager: requested {} MiB: {}",
        size,
        if result.is_ok() {
            "saved; active allocation changes on reboot"
        } else {
            "rejected or storage write failed"
        }
    );
    true
}
