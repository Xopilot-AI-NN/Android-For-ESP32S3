//! Run the production GPT/AVB/liblp/CPIO loader against the new system image.
//! A sparse 512 MiB image sits on an emulated 8 GB card. Metadata writes stay
//! in RAM; the source image is opened read-only and never modified by tests.
use super::*;
use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
};

struct Card8Gb {
    image: RefCell<File>,
    overlay: RefCell<BTreeMap<u32, [u8; 512]>>,
}
impl BlockDevice for Card8Gb {
    type Error = std::io::Error;
    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        Ok(BlockCount(15_661_056))
    }
    fn read(&self, blocks: &mut [Block], index: BlockIdx) -> Result<(), Self::Error> {
        for (offset, block) in blocks.iter_mut().enumerate() {
            let sector = index.0 + offset as u32;
            if let Some(data) = self.overlay.borrow().get(&sector) {
                block.contents = *data;
                continue;
            }
            let mut file = self.image.borrow_mut();
            file.seek(SeekFrom::Start(u64::from(sector) * 512))?;
            file.read_exact(&mut block.contents)?;
        }
        Ok(())
    }
    fn write(&self, blocks: &[Block], index: BlockIdx) -> Result<(), Self::Error> {
        for (offset, block) in blocks.iter().enumerate() {
            self.overlay
                .borrow_mut()
                .insert(index.0 + offset as u32, block.contents);
        }
        Ok(())
    }
}

#[test]
#[ignore = "requires a built firmware image; run explicitly with --ignored"]
fn new_image_boots_from_an_eight_gb_device_through_the_real_loader() {
    time::set(0);
    let path = std::env::var_os("WATCH_TEST_IMAGE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../Firmware/out/target/product/zero/aosp-wear-sd.img")
        });
    let image =
        File::open(&path).expect("build the new firmware image first, or set WATCH_TEST_IMAGE");
    let bytes = image.metadata().unwrap().len();
    assert_eq!(bytes, 512 * 1024 * 1024);
    let card = Card8Gb {
        image: RefCell::new(image),
        overlay: RefCell::new(BTreeMap::new()),
    };
    let mut boot = android_storage::load_block_device(card)
        .expect("GPT, slot, boot images and AVB must load on an 8 GB device");
    let bundle = boot
        .load_userspace_bundle()
        .expect("framework must load from super/liblp/CPIO");
    let (mut os, frame) =
        runtime::Runtime::start(bundle).expect("the built image must start the actual runtime");
    assert!(matches!(frame, runtime::RuntimeFrame::Material(_)));
    os.dispatch(runtime::RuntimeEvent::Home).unwrap();
    assert_eq!(os.state() & 31, 1);
    boot.mark_successful()
        .expect("A/B handoff metadata must be writable");
    assert!(!boot.block_device().overlay.borrow().is_empty());
    assert_eq!(
        boot.block_device().image.borrow().metadata().unwrap().len(),
        bytes
    );
}
