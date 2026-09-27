use embedded_sdmmc::BlockDevice;

use crate::gpt::{self, Partition};

const BOOT_CTRL_MAGIC: u32 = 0x4241_4342;
const BOOT_CTRL_VERSION: u8 = 1;
const BOOT_CTRL_OFFSET: u64 = 2048;
const DEFAULT_TRIES: u8 = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Slot { A, B }

impl Slot {
    pub fn suffix(self) -> &'static str { match self { Slot::A => "_a", Slot::B => "_b" } }
    pub fn as_str(self) -> &'static str { match self { Slot::A => "a", Slot::B => "b" } }
    pub fn other(self) -> Self { match self { Slot::A => Slot::B, Slot::B => Slot::A } }
    fn index(self) -> usize { match self { Slot::A => 0, Slot::B => 1 } }
}

#[derive(Clone, Copy, Debug, Default)]
struct SlotMeta {
    priority: u8,
    tries_remaining: u8,
    successful_boot: bool,
    verity_corrupted: bool,
}

impl SlotMeta {
    fn bootable(self) -> bool {
        self.priority != 0 && !self.verity_corrupted && (self.successful_boot || self.tries_remaining != 0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BootControl {
    current: Slot,
    slots: [SlotMeta; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootControlError { Io, Invalid }

impl BootControl {
    pub fn load_or_init<D: BlockDevice>(dev: &D, misc: Partition) -> Result<Self, BootControlError> {
        let mut raw = [0u8; 32];
        gpt::read_partition(dev, misc, BOOT_CTRL_OFFSET, &mut raw).map_err(|_| BootControlError::Io)?;
        match Self::decode(&raw) {
            Ok(ctrl) => Ok(ctrl),
            Err(_) => {
                let ctrl = Self::default_state();
                ctrl.store(dev, misc)?;
                Ok(ctrl)
            }
        }
    }

    pub fn is_bootable(&self, slot: Slot) -> bool {
        self.slots[slot.index()].bootable()
    }

    pub fn preferred_slot(&self) -> Option<Slot> {
        let a = self.slots[0];
        let b = self.slots[1];
        match (a.bootable(), b.bootable()) {
            (false, false) => None,
            (true, false) => Some(Slot::A),
            (false, true) => Some(Slot::B),
            (true, true) => {
                if a.priority >= b.priority { Some(Slot::A) } else { Some(Slot::B) }
            }
        }
    }

    pub fn begin_attempt<D: BlockDevice>(&mut self, dev: &D, misc: Partition, slot: Slot) -> Result<(), BootControlError> {
        let meta = &mut self.slots[slot.index()];
        if !meta.successful_boot && meta.tries_remaining > 0 { meta.tries_remaining -= 1; }
        self.current = slot;
        self.store(dev, misc)
    }

    pub fn mark_unbootable<D: BlockDevice>(&mut self, dev: &D, misc: Partition, slot: Slot) -> Result<(), BootControlError> {
        let meta = &mut self.slots[slot.index()];
        meta.priority = 0;
        meta.tries_remaining = 0;
        meta.successful_boot = false;
        self.store(dev, misc)
    }

    pub fn mark_successful<D: BlockDevice>(&mut self, dev: &D, misc: Partition, slot: Slot) -> Result<(), BootControlError> {
        let meta = &mut self.slots[slot.index()];
        meta.successful_boot = true;
        meta.verity_corrupted = false;
        if meta.tries_remaining == 0 { meta.tries_remaining = 1; }
        self.current = slot;
        self.store(dev, misc)
    }


    fn default_state() -> Self {
        Self {
            current: Slot::A,
            slots: [
                SlotMeta { priority: 15, tries_remaining: DEFAULT_TRIES, successful_boot: false, verity_corrupted: false },
                SlotMeta { priority: 14, tries_remaining: DEFAULT_TRIES, successful_boot: false, verity_corrupted: false },
            ],
        }
    }

    fn decode(raw: &[u8; 32]) -> Result<Self, BootControlError> {
        let expected_crc = u32::from_le_bytes([raw[28], raw[29], raw[30], raw[31]]);
        if gpt::crc32(&raw[..28]) != expected_crc { return Err(BootControlError::Invalid); }
        let magic = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
        if magic != BOOT_CTRL_MAGIC || raw[8] != BOOT_CTRL_VERSION || (raw[9] & 0x07) < 2 {
            return Err(BootControlError::Invalid);
        }
        let current = if raw[0] == b'_' && raw[1] == b'b' { Slot::B } else { Slot::A };
        let mut slots = [SlotMeta::default(); 2];
        for (i, slot) in slots.iter_mut().enumerate() {
            let flags0 = raw[12 + i * 2];
            let flags1 = raw[13 + i * 2];
            *slot = SlotMeta {
                priority: flags0 & 0x0f,
                tries_remaining: (flags0 >> 4) & 0x07,
                successful_boot: flags0 & 0x80 != 0,
                verity_corrupted: flags1 & 0x01 != 0,
            };
        }
        Ok(Self { current, slots })
    }

    fn encode(&self) -> [u8; 32] {
        let mut raw = [0u8; 32];
        let suffix = self.current.suffix().as_bytes();
        raw[..suffix.len()].copy_from_slice(suffix);
        raw[4..8].copy_from_slice(&BOOT_CTRL_MAGIC.to_le_bytes());
        raw[8] = BOOT_CTRL_VERSION;
        raw[9] = 2; // nb_slot=2, recovery_tries_remaining=0
        for (i, slot) in self.slots.iter().enumerate() {
            raw[12 + i * 2] = (slot.priority & 0x0f)
                | ((slot.tries_remaining & 0x07) << 4)
                | if slot.successful_boot { 0x80 } else { 0 };
            raw[13 + i * 2] = if slot.verity_corrupted { 1 } else { 0 };
        }
        let crc = gpt::crc32(&raw[..28]);
        raw[28..32].copy_from_slice(&crc.to_le_bytes());
        raw
    }

    fn store<D: BlockDevice>(&self, dev: &D, misc: Partition) -> Result<(), BootControlError> {
        let raw = self.encode();
        gpt::write_partition(dev, misc, BOOT_CTRL_OFFSET, &raw).map_err(|_| BootControlError::Io)
    }
}
