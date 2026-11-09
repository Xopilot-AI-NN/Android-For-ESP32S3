// Модуль загрузки Android-подобного образа.
extern crate alloc;

use alloc::{string::String, string::ToString};
#[cfg(not(feature = "pc-block-boot"))]
use embedded_hal::delay::DelayNs;
#[cfg(all(not(feature = "pc-block-boot"), not(feature = "sd-checked-startup")))]
use embedded_hal::spi::SpiDevice;
#[cfg(all(not(feature = "pc-block-boot"), feature = "sd-checked-startup"))]
use embedded_hal::{digital::OutputPin, spi::SpiBus};
use embedded_sdmmc::BlockDevice;
#[cfg(not(feature = "pc-block-boot"))]
use embedded_sdmmc::SdCard;

use crate::{
    android_boot, avb,
    boot_control::{BootControl, Slot},
    config::MAX_RUNTIME_BUNDLE_BYTES,
    cpio,
    gpt::{self, Partition},
    lp,
};

#[derive(Debug)]
/// Ошибки операций компонента загрузки Android-подобного образа.
pub enum AndroidBootError {
    Card,
    #[cfg(all(not(feature = "pc-block-boot"), feature = "sd-checked-startup"))]
    SdInit(crate::sd_init::InitError),
    #[cfg(all(not(feature = "pc-block-boot"), not(feature = "sd-checked-startup")))]
    SdAcquire(embedded_sdmmc::SdCardError),
    Gpt(gpt::GptError),
    BootControl,
    MissingSlot,
    Avb(avb::AvbError),
    BootImage(android_boot::BootImageError),
    Lp(lp::LpError),
    Cpio(cpio::CpioError),
    RuntimeBundleTooLarge,
}

impl core::fmt::Display for AndroidBootError {
    /// Форматирует значение для диагностики, не изменяя его.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Card => f.write_str("card I/O error"),
            #[cfg(all(not(feature = "pc-block-boot"), not(feature = "sd-checked-startup")))]
            Self::SdAcquire(error) => write!(f, "SD acquisition: {error:?}"),
            #[cfg(all(not(feature = "pc-block-boot"), feature = "sd-checked-startup"))]
            Self::SdInit(error) => write!(f, "SD initialization: {error}"),
            Self::Gpt(e) => write!(f, "GPT error: {:?}", e),
            Self::BootControl => f.write_str("boot-control error"),
            Self::MissingSlot => f.write_str("no bootable slot"),
            Self::Avb(e) => write!(f, "AVB error: {:?}", e),
            Self::BootImage(e) => write!(f, "boot-image error: {:?}", e),
            Self::Lp(e) => write!(f, "liblp error: {:?}", e),
            Self::Cpio(e) => write!(f, "CPIO error: {:?}", e),
            Self::RuntimeBundleTooLarge => f.write_str("runtime bundle too large"),
        }
    }
}

/// Загруженный слот, версия и входной скрипт компонента загрузки Android-подобного образа.
pub struct SystemImage {
    /// Выбранный загрузочный слот.
    pub slot: Slot,
    /// Прочитанная либо ожидаемая версия формата/системы.
    pub version: String,
    /// Входной скрипт или метаданные текущего элемента.
    pub entry: &'static str,
    /// Проверенный текст скрипта для запуска.
    pub script: String,
    /// Индекс anti-rollback из заголовка AVB.
    pub rollback_index: u64,
    /// Образ использует неподписанный режим AVB.
    pub avb_unsigned: bool,
}

/// Структура данных компонента загрузки Android-подобного образа.
pub struct BootSession<D: BlockDevice> {
    /// Владелец блочного интерфейса карты.
    card: D,
    /// Служебный раздел с метаданными A/B.
    misc: Partition,
    /// Управляющее состояние компонента/протокола.
    control: BootControl,
    /// Дескриптор загружаемого/тестируемого системного образа.
    pub image: SystemImage,
}

impl<D: BlockDevice> BootSession<D> {
    /// Возвращает диск при отказе запуска runtime, не оставляя USB внутри потерянной сессии.
    pub fn into_block_device(self) -> D { self.card }

    /// Отмечает выбранный слот как успешно загрузившийся.
    pub fn mark_successful(&mut self) -> Result<(), AndroidBootError> {
        self.control
            .mark_successful(&self.card, self.misc, self.image.slot)
            .map_err(|_| AndroidBootError::BootControl)
    }

    /// Load the AOSP-like userspace from real Android liblp metadata inside
    /// `super`. The early `/init.rhai` still originates in init_boot, while
    /// framework/vendor/product code lives in slot-suffixed logical partitions.
    pub fn load_userspace_bundle(&mut self) -> Result<String, AndroidBootError> {
        let super_part = gpt::find_partition(&self.card, "super").map_err(AndroidBootError::Gpt)?;
        let metadata_slot = match self.image.slot {
            Slot::A => 0,
            Slot::B => 1,
        };
        let suffix = self.image.slot.suffix();

        let init_script = core::mem::take(&mut self.image.script);
        let mut bundle = String::with_capacity(
            init_script
                .len()
                .saturating_add(8 * 1024)
                .min(MAX_RUNTIME_BUNDLE_BYTES),
        );
        append_component(&mut bundle, "/* init_boot:/init.rhai */\n", &init_script)?;
        drop(init_script);

        let components = [
            ("vendor", "etc/aosp_wear/vendor_runtime.rhai"),
            ("odm", "etc/aosp_wear/odm_runtime.rhai"),
            ("system_ext", "etc/aosp_wear/system_ext_runtime.rhai"),
            ("system", "framework/aosp-wear-framework.rhai"),
            ("product", "etc/aosp_wear/product_runtime.rhai"),
        ];

        for (logical_base, path) in components {
            let mut logical_name = logical_base.to_string();
            logical_name.push_str(suffix);
            let logical = lp::find_logical(&self.card, super_part, metadata_slot, &logical_name)
                .map_err(AndroidBootError::Lp)?;
            let text = cpio::extract_text(
                &self.card,
                super_part,
                logical,
                path,
                MAX_RUNTIME_BUNDLE_BYTES,
            )
            .map_err(AndroidBootError::Cpio)?;
            append_component(&mut bundle, "\n/* logical-partition component */\n", &text)?;
        }

        if bundle.len() > MAX_RUNTIME_BUNDLE_BYTES {
            return Err(AndroidBootError::RuntimeBundleTooLarge);
        }
        Ok(bundle)
    }

    /// Access the underlying block device without taking ownership.
    pub fn block_device(&self) -> &D {
        &self.card
    }
}

/// Добавляет проверенный компонент к загружаемому системному пакету.
fn append_component(bundle: &mut String, marker: &str, text: &str) -> Result<(), AndroidBootError> {
    let new_len = bundle
        .len()
        .checked_add(marker.len())
        .and_then(|v| v.checked_add(text.len()))
        .ok_or(AndroidBootError::RuntimeBundleTooLarge)?;
    if new_len > MAX_RUNTIME_BUNDLE_BYTES {
        return Err(AndroidBootError::RuntimeBundleTooLarge);
    }
    bundle.push_str(marker);
    bundle.push_str(text);
    Ok(())
}

#[cfg(all(not(feature = "pc-block-boot"), feature = "sd-checked-startup"))]
/// Загружает систему через соответствующий транспорт и проверяет её компоненты.
pub fn load_system<BUS, CS, DELAY, PostInit>(
    spi: crate::sd_spi::SdSpiSession<BUS, CS, DELAY>,
    delay: DELAY,
    post_init: PostInit,
) -> Result<BootSession<SdCard<crate::sd_spi::SdSpiSession<BUS, CS, DELAY>, DELAY>>, AndroidBootError>
where
    BUS: SpiBus<u8>,
    CS: OutputPin,
    DELAY: DelayNs,
    PostInit: FnOnce(&SdCard<crate::sd_spi::SdSpiSession<BUS, CS, DELAY>, DELAY>),
{
    let card = crate::sd_init::open(spi, delay).map_err(AndroidBootError::SdInit)?;
    post_init(&card);
    load_block_device(card)
}

/// Original SD acquisition path from the pre-restoration firmware. Error detail
/// is retained, but card protocol and CS behavior come from the original crates.
#[cfg(all(not(feature = "pc-block-boot"), not(feature = "sd-checked-startup")))]
pub fn load_system<SPI: SpiDevice<u8>, DELAY: DelayNs, PostInit: FnOnce(&SdCard<SPI, DELAY>)>(
    spi: SPI,
    delay: DELAY,
    post_init: PostInit,
) -> Result<BootSession<SdCard<SPI, DELAY>>, AndroidBootError> {
    crate::watch_println!("microSD: startup=original embedded-sdmmc 0.10 / CRC on");
    let card = SdCard::new(spi, delay);
    let bytes = card.num_bytes().map_err(AndroidBootError::SdAcquire)?;
    crate::watch_println!(
        "microSD: initialized, {} MiB, {:?}",
        bytes / (1024 * 1024),
        card.get_card_type()
    );
    post_init(&card);
    load_block_device(card)
}

/// Load the Android-like system from any synchronous 512-byte block device.
pub fn load_block_device<D: BlockDevice>(card: D) -> Result<BootSession<D>, AndroidBootError> {
    load_block_device_recoverable(card).map_err(|(_, error)| error)
}

/// Сохраняет транспорт при ошибке, чтобы сервисный режим смог повторно использовать USB.
/// Ошибка GPT/AVB не должна уничтожать единственного владельца периферии.
pub fn load_block_device_recoverable<D: BlockDevice>(card: D) -> Result<BootSession<D>, (D, AndroidBootError)> {
    match inspect_device(&card) {
        Ok((misc, control, image)) => Ok(BootSession { card, misc, control, image }),
        Err(error) => Err((card, error)),
    }
}

/// Читает и проверяет метаданные, не забирая владение диском у вызывающего кода.
fn inspect_device<D: BlockDevice>(card: &D) -> Result<(Partition, BootControl, SystemImage), AndroidBootError> {
    card.num_blocks().map_err(|_| AndroidBootError::Card)?;
    if let Err(error) = gpt::probe(card) {
        crate::disk_probe::report(card);
        return Err(AndroidBootError::Gpt(error));
    }
    let misc = gpt::find_partition(card, "misc").map_err(AndroidBootError::Gpt)?;
    let mut control =
        BootControl::load_or_init(card, misc).map_err(|_| AndroidBootError::BootControl)?;
    let preferred = control
        .preferred_slot()
        .ok_or(AndroidBootError::MissingSlot)?;

    let first = try_slot(card, misc, &mut control, preferred);
    let image = match first {
        Ok(image) => image,
        Err(first_error) => {
            if should_mark_unbootable(&first_error) {
                let _ = control.mark_unbootable(card, misc, preferred);
            }
            let fallback = preferred.other();
            if !control.is_bootable(fallback) {
                return Err(first_error);
            }
            match try_slot(card, misc, &mut control, fallback) {
                Ok(image) => image,
                Err(_) => return Err(first_error),
            }
        }
    };

    Ok((misc, control, image))
}

/// Пытается загрузить один слот, возвращая точную причину отказа.
fn try_slot<D: BlockDevice>(
    card: &D,
    misc: Partition,
    control: &mut BootControl,
    slot: Slot,
) -> Result<SystemImage, AndroidBootError> {
    control
        .begin_attempt(card, misc, slot)
        .map_err(|_| AndroidBootError::BootControl)?;

    let boot_name = partition_name("boot", slot);
    let init_boot_name = partition_name("init_boot", slot);
    let vendor_boot_name = partition_name("vendor_boot", slot);
    let vbmeta_name = partition_name("vbmeta", slot);
    let boot = gpt::find_partition(card, boot_name.as_str()).map_err(AndroidBootError::Gpt)?;
    let init_boot =
        gpt::find_partition(card, init_boot_name.as_str()).map_err(AndroidBootError::Gpt)?;
    let vendor_boot =
        gpt::find_partition(card, vendor_boot_name.as_str()).map_err(AndroidBootError::Gpt)?;
    let vbmeta = gpt::find_partition(card, vbmeta_name.as_str()).map_err(AndroidBootError::Gpt)?;

    let boot_avb =
        avb::verify_partition(card, vbmeta, boot, "boot").map_err(AndroidBootError::Avb)?;
    let init_avb = avb::verify_partition(card, vbmeta, init_boot, "init_boot")
        .map_err(AndroidBootError::Avb)?;
    let vendor_avb = avb::verify_partition(card, vbmeta, vendor_boot, "vendor_boot")
        .map_err(AndroidBootError::Avb)?;
    let rollback_index = boot_avb
        .rollback_index
        .max(init_avb.rollback_index)
        .max(vendor_avb.rollback_index);

    android_boot::validate_boot(card, boot).map_err(AndroidBootError::BootImage)?;
    android_boot::validate_vendor_boot(card, vendor_boot).map_err(AndroidBootError::BootImage)?;
    let (version, script) =
        android_boot::extract_init_script(card, init_boot).map_err(AndroidBootError::BootImage)?;

    Ok(SystemImage {
        slot,
        version,
        entry: "/init.rhai",
        script,
        rollback_index,
        avb_unsigned: boot_avb.unsigned_development_vbmeta
            || init_avb.unsigned_development_vbmeta
            || vendor_avb.unsigned_development_vbmeta,
    })
}

/// Формирует имя раздела с суффиксом слота.
fn partition_name(prefix: &str, slot: Slot) -> String {
    let mut out = String::from(prefix);
    out.push_str(slot.suffix());
    out
}

/// Определяет, следует ли исключить слот после данной ошибки.
fn should_mark_unbootable(error: &AndroidBootError) -> bool {
    match error {
        #[cfg(all(not(feature = "pc-block-boot"), feature = "sd-checked-startup"))]
        AndroidBootError::SdInit(_) => false,
        #[cfg(all(not(feature = "pc-block-boot"), not(feature = "sd-checked-startup")))]
        AndroidBootError::SdAcquire(_) => false,
        AndroidBootError::Avb(avb::AvbError::Io) => false,
        AndroidBootError::Avb(_) => true,
        AndroidBootError::BootImage(android_boot::BootImageError::Io) => false,
        AndroidBootError::BootImage(_) => true,
        AndroidBootError::Card
        | AndroidBootError::Gpt(_)
        | AndroidBootError::BootControl
        | AndroidBootError::MissingSlot
        | AndroidBootError::Lp(_)
        | AndroidBootError::Cpio(_)
        | AndroidBootError::RuntimeBundleTooLarge => false,
    }
}
