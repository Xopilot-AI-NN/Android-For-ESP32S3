//! Потоковые файлы с copy-on-write extents в зарезервированном хвосте userdata.
//! Каталог WFSv1 сохраняется; расширение записи использует ранее нулевые байты 89..96.
use super::*;
impl Fs {
    /// Проверяет, что новый пул не пересекается с прежним активным расширением swap.
    pub(super) fn blob_base(&self) -> Result<u64, Error> {
        if let Some(base) = self.pool_base {
            return Ok(base);
        }
        let total = self.part.size_bytes();
        if total < BLOB_RESERVE + 8 * 1024 * 1024 {
            return Err(Error::Full);
        }
        let base = (total - BLOB_RESERVE) / UNIT * UNIT;
        Ok(base)
    }
    /// Выбирает свободный extent под максимальную передачу; чужие блоки не перезаписывает.
    pub(super) fn begin_upload<D: BlockDevice>(
        &mut self,
        dev: &D,
        path: &str,
    ) -> Result<(), Error> {
        self.destination(path)?;
        self.check_virgin(dev)?;
        let index = match self.index(path) {
            Ok(i) if self.entries[i].kind == 1 => i,
            Ok(_) => return Err(Error::Exists),
            Err(_) => self
                .entries
                .iter()
                .position(|e| e.kind == 0)
                .ok_or(Error::Full)?,
        };
        let base = self.blob_base()?;
        let units = MAX_BLOB.div_ceil(UNIT_DATA);
        let count = ((self.part.size_bytes() - base) / UNIT) as usize;
        let start = (0..=count.saturating_sub(units))
            .find(|start| {
                self.entries.iter().filter(|e| e.kind != 0).all(|e| {
                    e.blob.is_none_or(|b| {
                        let a = usize::from(b);
                        let n = (e.size as usize).div_ceil(UNIT_DATA).max(1);
                        start + units <= a || *start >= a + n
                    })
                })
            })
            .ok_or(Error::Full)?;
        self.upload = Some(Upload {
            path: path.into(),
            index,
            start: start as u16,
            size: 0,
            crc: !0,
            generation: self.generation,
        });
        Ok(())
    }
    /// Проверяет или инициализирует один 64-КиБ блок, не присваивая чужие данные.
    fn prepare_unit<D: BlockDevice>(&self, dev: &D, unit: usize) -> Result<(), Error> {
        let base = self.blob_base()? + unit as u64 * UNIT;
        let mut header = [0; 512];
        gpt::read_partition(dev, self.part, base, &mut header).map_err(|_| Error::Io)?;
        if &header[..8] == b"WFBLOB1\0"
            && gpt::crc32(&header[..12]) == u32::from_le_bytes(header[12..16].try_into().unwrap())
        {
            return Ok(());
        }
        // Первый сектор сам по себе не доказывает пустоту остальных 63,5 КиБ.
        let mut chunk = [0; 4096];
        for off in (0..UNIT).step_by(4096) {
            gpt::read_partition(dev, self.part, base + off, &mut chunk).map_err(|_| Error::Io)?;
            if chunk.iter().any(|&x| x != 0) {
                return Err(Error::Foreign);
            }
        }
        header[..8].copy_from_slice(b"WFBLOB1\0");
        header[8..12].copy_from_slice(&(unit as u32).to_le_bytes());
        let crc = gpt::crc32(&header[..12]);
        header[12..16].copy_from_slice(&crc.to_le_bytes());
        gpt::write_partition(dev, self.part, base, &header).map_err(|_| Error::Io)
    }
    /// Записывает DATA порциями до 64 КиБ; RAM не зависит от общего размера файла.
    pub(super) fn upload_chunk<D: BlockDevice>(
        &mut self,
        dev: &D,
        bytes: &[u8],
    ) -> Result<(), Error> {
        let mut upload = self.upload.take().ok_or(Error::Busy)?;
        let result = (|| {
            if upload.size + bytes.len() > MAX_BLOB {
                return Err(Error::TooLarge);
            }
            let mut done = 0;
            while done < bytes.len() {
                let unit = usize::from(upload.start) + upload.size / UNIT_DATA;
                let at = upload.size % UNIT_DATA;
                if at == 0 {
                    self.prepare_unit(dev, unit)?;
                }
                let take = (bytes.len() - done).min(UNIT_DATA - at);
                // GPT helper выполняет read-modify-write краёв, сохраняя соседние байты.
                let offset = self.blob_base()? + unit as u64 * UNIT + 512 + at as u64;
                gpt::write_partition(dev, self.part, offset, &bytes[done..done + take])
                    .map_err(|_| Error::Io)?;
                let mut verify = vec![0; take];
                gpt::read_partition(dev, self.part, offset, &mut verify).map_err(|_| Error::Io)?;
                if verify != bytes[done..done + take] {
                    return Err(Error::Io);
                }
                upload.crc = gpt::crc32_update(upload.crc, &bytes[done..done + take]);
                upload.size += take;
                done += take;
            }
            Ok(())
        })();
        if result.is_ok() {
            self.upload = Some(upload);
        }
        result
    }
    /// Публикует каталог только после полной передачи и проверки данных.
    pub(super) fn finish_upload<D: BlockDevice>(&mut self, dev: &D) -> Result<(), Error> {
        let upload = self.upload.take().ok_or(Error::Busy)?;
        if upload.generation != self.generation {
            return Err(Error::Conflict);
        }
        let mut entries = self.entries.clone();
        entries[upload.index] = Entry {
            kind: 1,
            bank: 0,
            size: upload.size as u32,
            blob: Some(upload.start),
            crc: !upload.crc,
            path: heapless::String::try_from(upload.path.as_str())
                .map_err(|_| Error::InvalidPath)?,
        };
        self.commit(dev, entries)
    }
    /// Читает ограниченную порцию без выделения памяти под многомегабайтное вложение.
    pub fn read_range<D: BlockDevice>(
        &self,
        dev: &D,
        path: &str,
        offset: usize,
        len: usize,
    ) -> Result<Vec<u8>, Error> {
        let entry = &self.entries[self.index(path)?];
        if entry.kind != 1 {
            return Err(Error::InvalidPath);
        }
        if len > 65536 {
            return Err(Error::TooLarge);
        }
        let end = offset.saturating_add(len).min(entry.size as usize);
        if offset >= end {
            return Ok(Vec::new());
        }
        if let Some(start) = entry.blob {
            let mut out = vec![0; end - offset];
            let mut done = 0;
            while done < out.len() {
                let pos = offset + done;
                let at = pos % UNIT_DATA;
                let n = (out.len() - done).min(UNIT_DATA - at);
                let source = self.blob_base()?
                    + (usize::from(start) + pos / UNIT_DATA) as u64 * UNIT
                    + 512
                    + at as u64;
                gpt::read_partition(dev, self.part, source, &mut out[done..done + n])
                    .map_err(|_| Error::Io)?;
                done += n;
            }
            Ok(out)
        } else {
            Ok(self.read(dev, path)?[offset..end].to_vec())
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use embedded_sdmmc::{Block, BlockCount, BlockIdx};
    use std::{cell::RefCell, collections::BTreeMap};
    /// Sparse device models a large card while allocating only sectors that were written.
    pub struct Disk(RefCell<BTreeMap<u32, [u8; 512]>>);
    impl BlockDevice for Disk {
        type Error = std::io::Error;
        fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
            Ok(BlockCount(196608))
        }
        fn read(&self, blocks: &mut [Block], index: BlockIdx) -> Result<(), Self::Error> {
            for (i, b) in blocks.iter_mut().enumerate() {
                b.contents = *self
                    .0
                    .borrow()
                    .get(&(index.0 + i as u32))
                    .unwrap_or(&[0; 512]);
            }
            Ok(())
        }
        fn write(&self, blocks: &[Block], index: BlockIdx) -> Result<(), Self::Error> {
            for (i, b) in blocks.iter().enumerate() {
                self.0.borrow_mut().insert(index.0 + i as u32, b.contents);
            }
            Ok(())
        }
    }
    pub fn fixture() -> Disk {
        let old = super::super::tests::fixture();
        let original = old.data.borrow();
        let mut map = BTreeMap::new();
        let mut entry = original[2];
        entry[40..48].copy_from_slice(&196607u64.to_le_bytes());
        let mut header = original[1];
        header[88..92].copy_from_slice(&gpt::crc32(&entry[..128]).to_le_bytes());
        header[16..20].fill(0);
        let crc = gpt::crc32(&header[..92]);
        header[16..20].copy_from_slice(&crc.to_le_bytes());
        map.insert(1, header);
        map.insert(2, entry);
        Disk(RefCell::new(map))
    }
    #[test]
    fn multi_megabyte_upload_survives_remount_and_aborted_replacement() {
        let disk = fixture();
        let mut fs = Fs::mount(&disk).unwrap();
        fs.write(&disk, "/note.txt", b"keep", None).unwrap();
        fs.begin_upload(&disk, "/photo.jpg").unwrap();
        let chunk = [0x5a; 4096];
        for _ in 0..1280 {
            fs.upload_chunk(&disk, &chunk).unwrap();
        }
        fs.finish_upload(&disk).unwrap();
        drop(fs);
        let mut fs = Fs::mount(&disk).unwrap();
        assert_eq!(
            fs.entries[fs.index("/photo.jpg").unwrap()].size,
            5 * 1024 * 1024
        );
        assert_eq!(
            fs.read_range(&disk, "/photo.jpg", 4 * 1024 * 1024 + 123, 4096)
                .unwrap(),
            chunk
        );
        fs.begin_upload(&disk, "/photo.jpg").unwrap();
        fs.upload_chunk(&disk, b"interrupted new version").unwrap();
        drop(fs);
        let fs = Fs::mount(&disk).unwrap();
        assert_eq!(
            fs.read_range(&disk, "/photo.jpg", 0, 16).unwrap(),
            vec![0x5a; 16]
        );
        assert_eq!(fs.read(&disk, "/note.txt").unwrap(), b"keep");
    }
    #[test]
    fn jpeg_and_phone_sized_png_decode_without_a_full_image_buffer() {
        let disk = fixture();
        let mut fs = Fs::mount(&disk).unwrap();
        for (name, bytes) in [
            (
                "/photo.jpg",
                &include_bytes!("../../../tools/runtime-check/fixtures/red-baseline.jpg")[..],
            ),
            (
                "/screen.png",
                &include_bytes!("../../../tools/runtime-check/fixtures/red-phone.png")[..],
            ),
        ] {
            fs.begin_upload(&disk, name).unwrap();
            for data in bytes.chunks(4096) {
                fs.upload_chunk(&disk, data).unwrap();
            }
            fs.finish_upload(&disk).unwrap();
            let preview = fs
                .thumbnail(&disk, name)
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert_eq!(preview.len(), 240 * 200 * 2);
            let i = (100 * 240 + 120) * 2;
            let pixel = u16::from_le_bytes([preview[i], preview[i + 1]]);
            assert!(pixel & 0xf800 >= 0xf000, "{name}: {pixel:x}");
        }
    }
}
