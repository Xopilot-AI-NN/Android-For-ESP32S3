//! Аварийная запись в RTC RAM: не обращается к SD, heap, Mutex и файловым службам.
//! После software reset предыдущая паника переносится в обычный постоянный журнал.
use core::fmt::Write;
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut LAST: [u8; 512] = [0; 512];
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let mut text = heapless::String::<480>::new();
    let _ = write!(text, "{}", info);
    let trace = esp_backtrace::Backtrace::capture();
    esp_println::println!("\nPANIC: {}\nBacktrace:", info);
    for frame in trace.frames() {
        let pc = frame.program_counter();
        esp_println::println!("0x{:x}", pc);
        let _ = write!(text, " 0x{:x}", pc);
    }
    let mut bytes = [0; 512];
    bytes[..8].copy_from_slice(b"WPANIC1\0");
    bytes[8..10].copy_from_slice(&(text.len() as u16).to_le_bytes());
    bytes[12..12 + text.len()].copy_from_slice(text.as_bytes());
    let crc = crate::gpt::crc32(&bytes[..508]);
    bytes[508..].copy_from_slice(&crc.to_le_bytes());
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(LAST), bytes);
    }
    esp_hal::delay::Delay::new().delay_millis(250);
    esp_hal::system::software_reset()
}
/// Вызывается после настройки RAM-журнала; исходная RTC-запись поглощается один раз.
pub fn recover() {
    let bytes = unsafe {
        let p = core::ptr::addr_of_mut!(LAST);
        let b = core::ptr::read_volatile(p);
        core::ptr::write_volatile(p, [0; 512]);
        b
    };
    let len = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    if &bytes[..8] == b"WPANIC1\0"
        && len <= 480
        && crate::gpt::crc32(&bytes[..508]) == u32::from_le_bytes(bytes[508..].try_into().unwrap())
    {
        if let Ok(text) = core::str::from_utf8(&bytes[12..12 + len]) {
            let mut line = heapless::String::<128>::new();
            for ch in text.chars() {
                if line.len() + ch.len_utf8() > 120 {
                    crate::watch_println!("E (0) previous-panic: {}", line);
                    line.clear();
                }
                let _ = line.push(ch);
            }
            if !line.is_empty() {
                crate::watch_println!("E (0) previous-panic: {}", line);
            }
        }
    }
}
