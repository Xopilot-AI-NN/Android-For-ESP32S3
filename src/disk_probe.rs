//! Read-only evidence when GPT probing fails; never guesses a replacement layout.
use embedded_sdmmc::{Block, BlockDevice, BlockIdx};

pub fn report<D: BlockDevice>(dev: &D) {
    let mut sector = Block { contents: [0; 512] };
    match dev.read(core::slice::from_mut(&mut sector), BlockIdx(0)) {
        Ok(()) => esp_println::println!(
            "disk: LBA0 prefix={:02x?} signature={:02x?} partition_types={:02x?}",
            &sector.contents[..16],
            &sector.contents[510..],
            [
                sector.contents[450],
                sector.contents[466],
                sector.contents[482],
                sector.contents[498]
            ]
        ),
        Err(e) => esp_println::println!("disk: LBA0 read failed: {e:?}"),
    }
    if let Err(e) = dev.read(core::slice::from_mut(&mut sector), BlockIdx(1)) {
        esp_println::println!("disk: LBA1 read failed: {e:?}");
        return;
    }
    let first = sector.contents;
    esp_println::println!(
        "disk: LBA1 prefix={:02x?}; expected EFI PART (45 46 49 20 50 41 52 54)",
        &first[..16]
    );
    match dev.read(core::slice::from_mut(&mut sector), BlockIdx(1)) {
        Ok(()) => esp_println::println!(
            "disk: repeated LBA1 stable={}; prefix={:02x?}",
            first == sector.contents,
            &sector.contents[..16]
        ),
        Err(e) => esp_println::println!("disk: repeated LBA1 failed: {e:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{cell::RefCell, convert::Infallible};
    struct ReadOnly {
        requests: RefCell<std::vec::Vec<u32>>,
    }
    impl BlockDevice for ReadOnly {
        type Error = Infallible;
        fn read(&self, b: &mut [Block], idx: BlockIdx) -> Result<(), Self::Error> {
            assert_eq!(b.len(), 1);
            self.requests.borrow_mut().push(idx.0);
            b[0].contents.fill(0);
            if idx.0 == 1 {
                b[0].contents[..8].copy_from_slice(b"EFI PART");
            }
            Ok(())
        }
        fn write(&self, _: &[Block], _: BlockIdx) -> Result<(), Self::Error> {
            panic!("disk probe must never write")
        }
        fn num_blocks(&self) -> Result<embedded_sdmmc::BlockCount, Self::Error> {
            Ok(embedded_sdmmc::BlockCount(2048))
        }
    }
    #[test]
    fn diagnosis_reads_absolute_sectors_and_rechecks_without_writing() {
        let d = ReadOnly {
            requests: RefCell::new(std::vec::Vec::new()),
        };
        report(&d);
        assert_eq!(*d.requests.borrow(), [0, 1, 1]);
    }
}
