//! Versioned persistent settings and Wi-Fi credentials in reserved ZPager slots.
//! Keep these slots disjoint from Activity slots 0..31.
use crate::*;

// Keep framework Activity pages in slots 0..31.  Older 17.1.1 builds used
// slot 16 for Wi-Fi credentials, which collided with the Date & time Activity
// page and could silently erase the saved network when that screen was left.
const WIFI_CRED_SLOT: u32 = 64;
const WIFI_CRED_LEGACY_SLOT: u32 = 16;
const WIFI_CRED_TAG: u32 = 0x5a57_4946; // "ZWIF"
const TIME_HOLDOVER_SLOT: u32 = 65;
const TIME_HOLDOVER_TAG: u32 = 0x5a54_494d; // "ZTIM"
const SYSTEM_SETTINGS_SLOT: u32 = 66;
const SYSTEM_SETTINGS_TAG: u32 = 0x5a53_4554; // "ZSET"
const RTC_CONFIG_SLOT: u32 = 67;
const RTC_CONFIG_TAG: u32 = 0x5a52_5443; // "ZRTC"
const CLOCK_CONFIG_SLOT: u32 = 69;
const CLOCK_CONFIG_TAG: u32 = 0x5a43_4c4b; // "ZCLK"
// brightness..wireless-debugging, excluding screen/cursor and notifications.
pub(crate) const SYSTEM_SETTINGS_MASK: i32 = 0x00ff_fe00;

pub(crate) fn restore_watch_config<D: BlockDevice>(
    os: &mut runtime::Runtime,
    pager: Option<&page_store::PageStore>,
    dev: &D,
) {
    let mut entropy = [0; 16];
    esp_hal::rng::Rng::new().read(&mut entropy);
    os.set_security_storage(pager.is_some(), entropy);
    if let Some(pager) = pager {
        let mut preferences = [0; 4];
        if let Ok(n) = pager.read(dev, 71, 0x5750_5246, &mut preferences) {
            os.restore_preferences(&preferences[..n]);
        }
        let mut security = [0; 58];
        match pager.read(dev, 70, 0x5753_4543, &mut security) {
            Ok(n) => os.restore_security(&security[..n]),
            Err(page_store::PageStoreError::Empty) => {}
            Err(_) => os.security_storage_error(),
        }
    }
}

pub(crate) fn save_watch_preferences<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
    bytes: &[u8; 4],
) -> bool {
    pager.write(dev, 71, 0x5750_5246, bytes).is_ok()
}
pub(crate) fn save_watch_security<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
    bytes: &[u8; 58],
) -> bool {
    pager.write(dev, 70, 0x5753_4543, bytes).is_ok()
}

pub(crate) fn load_clock_config<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
) -> Option<i32> {
    pager
        .read_i32(dev, CLOCK_CONFIG_SLOT, CLOCK_CONFIG_TAG)
        .ok()
}

pub(crate) fn save_clock_config<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
    config: i32,
) -> bool {
    pager
        .write_i32(dev, CLOCK_CONFIG_SLOT, CLOCK_CONFIG_TAG, config)
        .is_ok()
}

pub(crate) fn load_wifi_credentials<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
) -> Option<WifiCredentials> {
    let mut raw = [0u8; 98];
    let n = match pager.read(dev, WIFI_CRED_SLOT, WIFI_CRED_TAG, &mut raw) {
        Ok(n) => n,
        Err(_) => pager
            .read(dev, WIFI_CRED_LEGACY_SLOT, WIFI_CRED_TAG, &mut raw)
            .ok()?,
    };
    WifiCredentials::decode_page(&raw[..n])
}

pub(crate) fn save_wifi_credentials<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
    credentials: WifiCredentials,
) -> bool {
    let mut raw = [0u8; 98];
    let n = credentials.encode_page(&mut raw);
    pager
        .write(dev, WIFI_CRED_SLOT, WIFI_CRED_TAG, &raw[..n])
        .is_ok()
}

pub(crate) fn load_time_holdover<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
) -> Option<u16> {
    let value = pager
        .read_i32(dev, TIME_HOLDOVER_SLOT, TIME_HOLDOVER_TAG)
        .ok()?;
    if (0..1440).contains(&value) {
        Some(value as u16)
    } else {
        None
    }
}

pub(crate) fn save_time_holdover<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
    minutes: u16,
) -> bool {
    pager
        .write_i32(
            dev,
            TIME_HOLDOVER_SLOT,
            TIME_HOLDOVER_TAG,
            (minutes % 1440) as i32,
        )
        .is_ok()
}

pub(crate) fn load_system_settings<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
) -> Option<i32> {
    pager
        .read_i32(dev, SYSTEM_SETTINGS_SLOT, SYSTEM_SETTINGS_TAG)
        .ok()
}

pub(crate) fn save_system_settings<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
    state: i32,
) -> bool {
    pager
        .write_i32(
            dev,
            SYSTEM_SETTINGS_SLOT,
            SYSTEM_SETTINGS_TAG,
            state & SYSTEM_SETTINGS_MASK,
        )
        .is_ok()
}

pub(crate) fn load_rtc_config<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
) -> Option<i32> {
    pager.read_i32(dev, RTC_CONFIG_SLOT, RTC_CONFIG_TAG).ok()
}

pub(crate) fn save_rtc_config<D: BlockDevice>(
    pager: &page_store::PageStore,
    dev: &D,
    config: i32,
) -> bool {
    pager
        .write_i32(dev, RTC_CONFIG_SLOT, RTC_CONFIG_TAG, config)
        .is_ok()
}
