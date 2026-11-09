// Модуль системных иконок и обоев.
//! System-owned graphics. Only icon masks and one requested PNG are cached in RAM.
use crate::{
    cpio,
    gpt::{self, Partition},
    lp::{self, LogicalPartition},
};
use alloc::{sync::Arc, vec::Vec};
use core::{
    cell::RefCell,
    sync::atomic::{AtomicU32, Ordering},
};
use critical_section::Mutex;
use embedded_sdmmc::BlockDevice;
/// Количество элементов соответствующей таблицы/меню.
const COUNT: usize = 18;
/// Максимальный размер файла для данного формата, байт.
const MAX_FILE: usize = 320 * 1024;
#[derive(Clone, Copy, Default)]
/// Метаданные одной записи компонента системных иконок и обоев.
struct Entry {
    /// Смещение в байтах относительно начала соответствующей области.
    offset: u32,
    /// Размер содержимого в байтах.
    size: u32,
    /// Контрольная сумма содержимого для обнаружения повреждений.
    crc: u32,
}
#[derive(Clone, Copy)]
/// Расположение данных внутри логического раздела компонента системных иконок и обоев.
struct Location {
    /// Границы физического раздела, которому принадлежит область.
    part: Partition,
    /// Отображение логического раздела внутри Android super.
    logical: LogicalPartition,
    /// Базовое смещение области или пакета ресурсов.
    base: u64,
}
/// Кэш системных графических ресурсов компонента системных иконок и обоев.
struct Cache {
    /// Расположение ресурса в системном образе.
    location: Option<Location>,
    /// Таблица записей с проверенными именами и метаданными.
    entries: [Entry; COUNT],
    /// Общий кэш атласа пиктограмм.
    icons: Option<Arc<Vec<u8>>>,
    launcher: Option<Arc<Vec<u8>>>,
    /// Кэш выбранных пользовательских/системных обоев.
    wallpaper: Option<(u8, Arc<Vec<u8>>)>,
    /// Необработанный запрос или ожидающий результат.
    pending: Option<u8>,
    /// Номер ресурса/признак операции с завершившимся отказом.
    failed: Option<u8>,
}
/// Параметр CACHE компонента системных иконок и обоев.
static CACHE: Mutex<RefCell<Cache>> = Mutex::new(RefCell::new(Cache {
    location: None,
    entries: [Entry {
        offset: 0,
        size: 0,
        crc: 0,
    }; COUNT],
    icons: None,
    launcher: None,
    wallpaper: None,
    pending: None,
    failed: None,
}));
/// Счётчик изменений, по которому обновляются зависимые кэши.
static REVISION: AtomicU32 = AtomicU32::new(0);
/// Сообщает о наличии отложенной работы.
pub fn pending() -> bool {
    critical_section::with(|cs| CACHE.borrow_ref(cs).pending.is_some())
}
/// Возвращает счётчик изменений для обновления зависимых кэшей.
pub fn revision() -> u32 {
    REVISION.load(Ordering::Acquire)
}
/// Читает ресурс, проверяет CRC и размещает данные в PSRAM.
fn read_entry<D: BlockDevice>(
    dev: &D,
    location: Location,
    entry: Entry,
) -> Result<Arc<Vec<u8>>, &'static str> {
    // Graphic buffers belong in PSRAM: internal RAM is needed by Wi-Fi/BLE stacks.
    let size = entry.size as usize;
    let layout = core::alloc::Layout::array::<u8>(size).map_err(|_| "resource layout invalid")?;
    // SAFETY: HEAP is the global allocator. This exclusive allocation uses exactly
    // the Vec capacity/layout below; dropping the Vec returns it to the same HEAP.
    let pointer =
        unsafe { esp_alloc::HEAP.alloc_caps(esp_alloc::MemoryCapability::External.into(), layout) };
    if pointer.is_null() {
        return Err("resource PSRAM unavailable");
    }
    let mut bytes = unsafe { Vec::from_raw_parts(pointer, 0, size) };
    bytes.resize(size, 0);
    lp::read_logical(
        dev,
        location.part,
        location.logical,
        location.base + u64::from(entry.offset),
        &mut bytes,
    )
    .map_err(|_| "resource read failed")?;
    if gpt::crc32(&bytes) != entry.crc {
        return Err("resource CRC mismatch");
    }
    Ok(Arc::new(bytes))
}
/// Mount the active system slot. A missing pack never disables recovery.
pub fn mount<D: BlockDevice>(dev: &D, slot: crate::boot_control::Slot) -> Result<(), &'static str> {
    let part = gpt::find_partition(dev, "super").map_err(|_| "super unavailable")?;
    let metadata_slot = if slot == crate::boot_control::Slot::A {
        0
    } else {
        1
    };
    let name = if metadata_slot == 0 {
        "system_a"
    } else {
        "system_b"
    };
    let logical =
        lp::find_logical(dev, part, metadata_slot, name).map_err(|_| "system unavailable")?;
    let (base, len) = cpio::locate_file(
        dev,
        part,
        logical,
        "usr/share/aosp-wear/ui-resources.bin",
        COUNT * MAX_FILE + 512,
    )
    .map_err(|_| "system image needs UI resource update")?;
    if len < 512 {
        return Err("short resource header");
    }
    let mut header = [0; 512];
    lp::read_logical(dev, part, logical, base, &mut header)
        .map_err(|_| "resource header read failed")?;
    if &header[..8] != b"WRESv1\0\0"
        || u32::from_le_bytes(header[8..12].try_into().unwrap()) != COUNT as u32
    {
        return Err("unsupported resource package");
    }
    let mut entries = [Entry::default(); COUNT];
    let mut expected = 512_u64;
    for (i, entry) in entries.iter_mut().enumerate() {
        let p = 16 + i * 12;
        *entry = Entry {
            offset: u32::from_le_bytes(header[p..p + 4].try_into().unwrap()),
            size: u32::from_le_bytes(header[p + 4..p + 8].try_into().unwrap()),
            crc: u32::from_le_bytes(header[p + 8..p + 12].try_into().unwrap()),
        };
        if u64::from(entry.offset) != expected || entry.size == 0 || entry.size as usize > MAX_FILE
        {
            return Err("invalid resource index");
        }
        expected += u64::from(entry.size);
        if expected > len as u64 {
            return Err("resource outside package");
        }
    }
    if ![31, 35, 40].into_iter().any(|count| {
        entries[0].size == count * 24 * 24 / 2 && entries[1].size == count * 30 * 30 / 2
    }) {
        return Err("incompatible icon atlas");
    }
    let location = Location {
        part,
        logical,
        base,
    };
    let icons = read_entry(dev, location, entries[0])?;
    let launcher = read_entry(dev, location, entries[1])?;
    critical_section::with(|cs| {
        let mut cache = CACHE.borrow_ref_mut(cs);
        cache.location = Some(location);
        cache.entries = entries;
        cache.icons = Some(icons);
        cache.launcher = Some(launcher);
    });
    REVISION.fetch_add(1, Ordering::Release);
    Ok(())
}
/// Возвращает общий атлас обычных или крупных иконок.
pub fn icons(large: bool) -> Option<Arc<Vec<u8>>> {
    critical_section::with(|cs| {
        let cache = CACHE.borrow_ref(cs);
        if large {
            cache.launcher.clone()
        } else {
            cache.icons.clone()
        }
    })
}
/// Возвращает известный размер выбранного системного PNG.
pub fn wallpaper_len(index: u8) -> Option<usize> {
    if !(1..=16).contains(&index) {
        return None;
    }
    critical_section::with(|cs| {
        let cache = CACHE.borrow_ref(cs);
        cache
            .location
            .map(|_| cache.entries[usize::from(index) + 1].size as usize)
    })
}
/// Queue a bounded read; painting and ADB callbacks never access the disk directly.
pub fn wallpaper(index: u8) -> Option<Arc<Vec<u8>>> {
    if !(1..=16).contains(&index) {
        return None;
    }
    critical_section::with(|cs| {
        let mut cache = CACHE.borrow_ref_mut(cs);
        if let Some((cached, bytes)) = &cache.wallpaper {
            if *cached == index {
                return Some(bytes.clone());
            }
        }
        if cache.location.is_some() && cache.failed != Some(index) {
            cache.pending = Some(index);
        }
        None
    })
}
/// Сообщает, завершилось ли чтение ресурса ошибкой.
pub fn failed(index: u8) -> bool {
    critical_section::with(|cs| {
        let cache = CACHE.borrow_ref(cs);
        cache.location.is_none() || cache.failed == Some(index)
    })
}
/// Main-loop disk owner services at most one image, outside any critical section.
#[inline(never)]
pub fn service<D: BlockDevice>(dev: &D) -> bool {
    let job = critical_section::with(|cs| {
        let mut cache = CACHE.borrow_ref_mut(cs);
        let index = cache.pending.take()?;
        let location = cache.location?;
        cache.wallpaper = None;
        Some((index, location, cache.entries[usize::from(index) + 1]))
    });
    let Some((index, location, entry)) = job else {
        return false;
    };
    let result = read_entry(dev, location, entry);
    if let Err(reason) = &result {
        crate::watch_println!("Resources: wallpaper {}: {}", index, reason);
    }
    critical_section::with(|cs| {
        let mut cache = CACHE.borrow_ref_mut(cs);
        match result {
            Ok(bytes) => {
                cache.wallpaper = Some((index, bytes));
                cache.failed = None;
            }
            Err(_) => cache.failed = Some(index),
        }
    });
    REVISION.fetch_add(1, Ordering::Release);
    true
}
