extern crate alloc;

use alloc::{string::String, string::ToString};
#[cfg(not(feature = "pc-block-boot"))]
use embedded_hal::{delay::DelayNs, spi::SpiDevice};
use embedded_sdmmc::BlockDevice;
#[cfg(not(feature = "pc-block-boot"))]
use embedded_sdmmc::SdCard;

use crate::{
    android_boot,
    avb,
    boot_control::{BootControl, Slot},
    config::MAX_RUNTIME_BUNDLE_BYTES,
    cpio,
    gpt::{self, Partition},
    lp,
};

#[derive(Debug)]
pub enum AndroidBootError {
    Card,
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
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Card => f.write_str("card I/O error"),
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

pub struct SystemImage {
    pub slot: Slot,
    pub version: String,
    pub entry: &'static str,
    pub script: String,
    pub rollback_index: u64,
    pub avb_unsigned: bool,
}

pub struct BootSession<D: BlockDevice> {
    card: D,
    misc: Partition,
    control: BootControl,
    pub image: SystemImage,
}

impl<D: BlockDevice> BootSession<D> {
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
        let metadata_slot = match self.image.slot { Slot::A => 0, Slot::B => 1 };
        let suffix = self.image.slot.suffix();

        let init_script = core::mem::take(&mut self.image.script);
        let mut bundle = String::with_capacity(
            init_script.len().saturating_add(8 * 1024).min(MAX_RUNTIME_BUNDLE_BYTES),
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
    pub fn block_device(&self) -> &D { &self.card }
}

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

#[cfg(not(feature = "pc-block-boot"))]
pub fn load_system<SPI, DELAY, PostInit>(
    spi: SPI,
    delay: DELAY,
    post_init: PostInit,
) -> Result<BootSession<SdCard<SPI, DELAY>>, AndroidBootError>
where
    SPI: SpiDevice<u8>,
    DELAY: DelayNs,
    PostInit: FnOnce(&SdCard<SPI, DELAY>),
{
    let card = SdCard::new(spi, delay);
    card.num_bytes().map_err(|_| AndroidBootError::Card)?;
    post_init(&card);
    load_block_device(card)
}

/// Load the Android-like system from any synchronous 512-byte block device.
pub fn load_block_device<D: BlockDevice>(card: D) -> Result<BootSession<D>, AndroidBootError> {
    card.num_blocks().map_err(|_| AndroidBootError::Card)?;
    gpt::probe(&card).map_err(AndroidBootError::Gpt)?;
    let misc = gpt::find_partition(&card, "misc").map_err(AndroidBootError::Gpt)?;
    let mut control = BootControl::load_or_init(&card, misc).map_err(|_| AndroidBootError::BootControl)?;
    let preferred = control.preferred_slot().ok_or(AndroidBootError::MissingSlot)?;

    let first = try_slot(&card, misc, &mut control, preferred);
    let image = match first {
        Ok(image) => image,
        Err(first_error) => {
            if should_mark_unbootable(&first_error) {
                let _ = control.mark_unbootable(&card, misc, preferred);
            }
            let fallback = preferred.other();
            if !control.is_bootable(fallback) { return Err(first_error); }
            match try_slot(&card, misc, &mut control, fallback) {
                Ok(image) => image,
                Err(_) => return Err(first_error),
            }
        }
    };

    Ok(BootSession { card, misc, control, image })
}

fn try_slot<D: BlockDevice>(
    card: &D,
    misc: Partition,
    control: &mut BootControl,
    slot: Slot,
) -> Result<SystemImage, AndroidBootError> {
    control.begin_attempt(card, misc, slot).map_err(|_| AndroidBootError::BootControl)?;

    let boot_name = partition_name("boot", slot);
    let init_boot_name = partition_name("init_boot", slot);
    let vendor_boot_name = partition_name("vendor_boot", slot);
    let vbmeta_name = partition_name("vbmeta", slot);
    let boot = gpt::find_partition(card, boot_name.as_str()).map_err(AndroidBootError::Gpt)?;
    let init_boot = gpt::find_partition(card, init_boot_name.as_str()).map_err(AndroidBootError::Gpt)?;
    let vendor_boot = gpt::find_partition(card, vendor_boot_name.as_str()).map_err(AndroidBootError::Gpt)?;
    let vbmeta = gpt::find_partition(card, vbmeta_name.as_str()).map_err(AndroidBootError::Gpt)?;

    let boot_avb = avb::verify_partition(card, vbmeta, boot, "boot").map_err(AndroidBootError::Avb)?;
    let init_avb = avb::verify_partition(card, vbmeta, init_boot, "init_boot").map_err(AndroidBootError::Avb)?;
    let vendor_avb = avb::verify_partition(card, vbmeta, vendor_boot, "vendor_boot").map_err(AndroidBootError::Avb)?;
    let rollback_index = boot_avb.rollback_index.max(init_avb.rollback_index).max(vendor_avb.rollback_index);

    android_boot::validate_boot(card, boot).map_err(AndroidBootError::BootImage)?;
    android_boot::validate_vendor_boot(card, vendor_boot).map_err(AndroidBootError::BootImage)?;
    let (version, script) = android_boot::extract_init_script(card, init_boot).map_err(AndroidBootError::BootImage)?;

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

fn partition_name(prefix: &str, slot: Slot) -> String {
    let mut out = String::from(prefix);
    out.push_str(slot.suffix());
    out
}

fn should_mark_unbootable(error: &AndroidBootError) -> bool {
    match error {
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
