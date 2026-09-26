extern crate alloc;

use alloc::string::String;
use embedded_hal::{delay::DelayNs, spi::SpiDevice};
use embedded_sdmmc::{BlockDevice, SdCard};

use crate::{
    android_boot,
    avb,
    boot_control::{BootControl, Slot},
    gpt::{self, Partition},
};

#[derive(Debug)]
pub enum AndroidBootError {
    Card,
    Gpt(gpt::GptError),
    BootControl,
    MissingSlot,
    Avb(avb::AvbError),
    BootImage(android_boot::BootImageError),
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
}

pub fn load_system<SPI, DELAY, POST_INIT>(
    spi: SPI,
    delay: DELAY,
    post_init: POST_INIT,
) -> Result<BootSession<SdCard<SPI, DELAY>>, AndroidBootError>
where
    SPI: SpiDevice<u8>,
    DELAY: DelayNs,
    POST_INIT: FnOnce(&SdCard<SPI, DELAY>),
{
    let card = SdCard::new(spi, delay);
    card.num_bytes().map_err(|_| AndroidBootError::Card)?;
    post_init(&card);
    gpt::probe(&card).map_err(AndroidBootError::Gpt)?;
    let misc = gpt::find_partition(&card, "misc").map_err(AndroidBootError::Gpt)?;
    let mut control = BootControl::load_or_init(&card, misc).map_err(|_| AndroidBootError::BootControl)?;
    let preferred = control.preferred_slot().ok_or(AndroidBootError::MissingSlot)?;

    let first = try_slot(&card, misc, &mut control, preferred);
    let image = match first {
        Ok(image) => image,
        Err(first_error) => {
            // A transient SD I/O failure must not permanently kill a slot. The
            // AOSP-compatible tries_remaining counter was already consumed by
            // begin_attempt(). Only verified metadata/image corruption is
            // promoted to an unbootable slot immediately.
            if should_mark_unbootable(&first_error) {
                let _ = control.mark_unbootable(&card, misc, preferred);
            }
            let fallback = preferred.other();
            if !control.is_bootable(fallback) {
                return Err(first_error);
            }
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
        avb_unsigned: boot_avb.unsigned_development_vbmeta || init_avb.unsigned_development_vbmeta || vendor_avb.unsigned_development_vbmeta,
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
        // GPT/card/control failures are media/global-state failures rather
        // than proof that one A/B slot is corrupt. Let retries/fallback deal
        // with them without rewriting slot priority.
        AndroidBootError::Card
        | AndroidBootError::Gpt(_)
        | AndroidBootError::BootControl
        | AndroidBootError::MissingSlot => false,
    }
}
