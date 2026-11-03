//! 64-КиБ кольцевой журнал в userdata+1,5 МиБ, вне обоев, снимков, документов и swap.
//! Каждый сектор — законченная запись с CRC; метаданные не нужны, восстановление по sequence.
use crate::{
    gpt::{self, Partition},
    logging,
};
use alloc::{string::String, vec::Vec};
use core::{
    cell::RefCell,
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering},
};
use critical_section::Mutex;
use embedded_sdmmc::BlockDevice;
const BASE: u64 = 1536 * 1024;
pub const CAPACITY: usize = 64 * 1024;
const COUNT: u64 = (CAPACITY / 512) as u64;
#[derive(Clone, Copy)]
struct State {
    part: Partition,
    next: u64,
    cursor: u64,
}
static STATE: Mutex<RefCell<Option<State>>> = Mutex::new(RefCell::new(None));
static DISABLED: AtomicBool = AtomicBool::new(false);
static CLEAR: AtomicBool = AtomicBool::new(false);
/// Отложенная очистка выполняется только владельцем диска, не внутри logcat/IRQ.
pub fn request_clear() {
    CLEAR.store(true, Ordering::Relaxed);
}
fn valid(bytes: &[u8; 512]) -> bool {
    &bytes[..8] == b"WLOGv1\0\0"
        && usize::from(u16::from_le_bytes([bytes[16], bytes[17]])) <= 488
        && gpt::crc32(&bytes[..508]) == u32::from_le_bytes(bytes[508..].try_into().unwrap())
}
/// Ищет последнее целое поколение; незнакомые данные никогда не форматируются.
fn mount<D: BlockDevice>(dev: &D) -> Option<State> {
    let part = gpt::find_partition(dev, "userdata").ok()?;
    if part.size_bytes() < BASE + CAPACITY as u64 {
        return None;
    }
    let mut next = 1;
    for index in 0..COUNT {
        let mut bytes = [0; 512];
        gpt::read_partition(dev, part, BASE + index * 512, &mut bytes).ok()?;
        // A torn record with our signature is recoverable; foreign nonzero sectors
        // must never be overwritten just because another sector belongs to us.
        if bytes.iter().any(|b| *b != 0) && &bytes[..8] != b"WLOGv1\0\0" {
            return None;
        }
        if valid(&bytes) {
            next = next.max(u64::from_le_bytes(bytes[8..16].try_into().ok()?).saturating_add(1));
        }
    }
    Some(State {
        part,
        next,
        cursor: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reset() {
        logging::clear();
        critical_section::with(|cs| *STATE.borrow_ref_mut(cs) = None);
        DISABLED.store(false, Ordering::Relaxed);
        CLEAR.store(false, Ordering::Relaxed);
    }
    #[test]
    fn ring_wrap_remount_torn_record_and_clear_preserve_bounds() {
        reset();
        let disk = crate::file_store::tests::fixture();
        for index in 0..140 {
            logging::emit(format_args!("Journal: record-{index:03}"));
            service(&disk);
        }
        let text = String::from_utf8(export(&disk).unwrap()).unwrap();
        assert!(!text.contains("record-011"));
        assert!(text.contains("record-012"));
        assert!(text.ends_with("record-139\n"));
        let state = mount(&disk).unwrap();
        assert_eq!(state.next, 141);
        // Break the last record CRC; remount must skip it without a panic.
        let position = BASE + 11 * 512;
        let mut sector = [0; 512];
        gpt::read_partition(&disk, state.part, position, &mut sector).unwrap();
        sector[30] ^= 1;
        gpt::write_partition(&disk, state.part, position, &sector).unwrap();
        assert!(
            !String::from_utf8(export(&disk).unwrap())
                .unwrap()
                .contains("record-139")
        );
        request_clear();
        service(&disk);
        assert!(export(&disk).unwrap().is_empty());
        reset();
    }
    #[test]
    fn foreign_sector_is_rejected_even_beside_valid_records() {
        reset();
        let disk = crate::file_store::tests::fixture();
        logging::emit(format_args!("Journal: valid"));
        service(&disk);
        let part = mount(&disk).unwrap().part;
        gpt::write_partition(&disk, part, BASE + 512, &[0x77; 512]).unwrap();
        assert!(mount(&disk).is_none());
        reset();
    }
}
/// Сохраняет максимум одну запись за итерацию, без дискового I/O под global critical section.
pub fn service<D: BlockDevice>(dev: &D) {
    if DISABLED.load(Ordering::Relaxed) {
        return;
    }
    let cached = critical_section::with(|cs| *STATE.borrow_ref(cs));
    let Some(mut state) = cached.or_else(|| mount(dev)) else {
        DISABLED.store(true, Ordering::Relaxed);
        return;
    };
    if CLEAR.swap(false, Ordering::Relaxed) {
        for index in 0..COUNT {
            if gpt::write_partition(dev, state.part, BASE + index * 512, &[0; 512]).is_err() {
                return;
            }
        }
        state.next = 1;
        state.cursor = logging::bounds().1;
    }
    if let Some(record) = logging::next(&mut state.cursor) {
        let mut text = heapless::String::<480>::new();
        let _ = write!(
            text,
            "{} {} {}: {}\n",
            record.uptime_ms, record.priority as char, record.tag, record.message
        );
        let mut bytes = [0; 512];
        bytes[..8].copy_from_slice(b"WLOGv1\0\0");
        bytes[8..16].copy_from_slice(&state.next.to_le_bytes());
        bytes[16..18].copy_from_slice(&(text.len() as u16).to_le_bytes());
        bytes[20..20 + text.len()].copy_from_slice(text.as_bytes());
        let crc = gpt::crc32(&bytes[..508]);
        bytes[508..].copy_from_slice(&crc.to_le_bytes());
        if gpt::write_partition(
            dev,
            state.part,
            BASE + ((state.next - 1) % COUNT) * 512,
            &bytes,
        )
        .is_err()
        {
            return;
        }
        state.next = state.next.saturating_add(1);
    }
    critical_section::with(|cs| *STATE.borrow_ref_mut(cs) = Some(state));
}
/// Виртуальный файл /data/log/watch.log: хронологический UTF-8, без пустого хвоста.
pub fn export<D: BlockDevice>(dev: &D) -> Result<Vec<u8>, crate::file_store::Error> {
    let state = mount(dev).ok_or(crate::file_store::Error::Unavailable)?;
    let mut rows = Vec::new();
    for index in 0..COUNT {
        let mut bytes = [0; 512];
        gpt::read_partition(dev, state.part, BASE + index * 512, &mut bytes)
            .map_err(|_| crate::file_store::Error::Io)?;
        if valid(&bytes) {
            let sequence = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
            let len = u16::from_le_bytes([bytes[16], bytes[17]]) as usize;
            if let Ok(text) = core::str::from_utf8(&bytes[20..20 + len]) {
                rows.push((sequence, String::from(text)));
            }
        }
    }
    rows.sort_by_key(|r| r.0);
    let mut out = Vec::with_capacity(CAPACITY);
    for (_, row) in rows {
        out.extend_from_slice(row.as_bytes());
    }
    Ok(out)
}
