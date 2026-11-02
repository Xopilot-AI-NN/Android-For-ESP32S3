//! Bounded ADB SYNC v1: wallpaper uploads and read-only built-in PNG files.
use crate::wallpaper;
use alloc::vec::Vec;
const MAX_FILE: usize = 320 * 1024;
pub struct Receiver {
    input: Vec<u8>,
    file: Vec<u8>,
    pub output: Vec<u8>,
    download: Option<(&'static [u8], usize)>,
    sending: bool,
    waiting: bool,
    pub finished: bool,
}
impl Receiver {
    pub fn new() -> Self {
        Self {
            input: Vec::new(),
            file: Vec::new(),
            output: Vec::new(),
            download: None,
            sending: false,
            waiting: false,
            finished: false,
        }
    }
    fn fail(&mut self, reason: &str) {
        self.download = None;
        self.sending = false;
        self.waiting = false;
        self.output.clear();
        self.output.extend_from_slice(b"FAIL");
        self.output
            .extend_from_slice(&(reason.len() as u32).to_le_bytes());
        self.output.extend_from_slice(reason.as_bytes());
        self.finished = true;
    }
    pub fn poll(&mut self) {
        if self.finished {
            return;
        }
        // ACK-paced reads keep at most 520 bytes queued, even for large PNGs.
        if self.output.is_empty() {
            if let Some((bytes, offset)) = self.download {
                if offset == bytes.len() {
                    self.output.extend_from_slice(b"DONE\0\0\0\0");
                    self.download = None;
                } else {
                    let end = (offset + 512).min(bytes.len());
                    self.output.extend_from_slice(b"DATA");
                    self.output
                        .extend_from_slice(&((end - offset) as u32).to_le_bytes());
                    self.output.extend_from_slice(&bytes[offset..end]);
                    self.download = Some((bytes, end));
                }
            }
        }
        if self.waiting {
            if let Some(ok) = wallpaper::upload_result() {
                self.waiting = false;
                if ok {
                    self.output.extend_from_slice(b"OKAY\0\0\0\0");
                } else {
                    self.fail("wallpaper could not be saved to userdata");
                }
            }
        }
    }
    pub fn push(&mut self, bytes: &[u8]) {
        if self.finished || self.waiting {
            return;
        }
        if self.input.len() + bytes.len() > 66 * 1024 {
            self.fail("SYNC packet too large");
            return;
        }
        self.input.extend_from_slice(bytes);
        loop {
            if self.input.len() < 8 {
                return;
            }
            let mut command = [0; 4];
            command.copy_from_slice(&self.input[..4]);
            let size = u32::from_le_bytes(self.input[4..8].try_into().unwrap_or([0; 4])) as usize;
            if &command == b"DONE" {
                self.input.drain(..8);
                if !self.sending {
                    self.fail("SEND required before DONE");
                    return;
                }
                self.sending = false;
                match wallpaper::queue_png(&self.file) {
                    Ok(()) => {
                        self.waiting = true;
                        self.file.clear();
                    }
                    Err(reason) => self.fail(reason),
                }
                return;
            }
            if &command == b"QUIT" {
                self.finished = true;
                self.input.clear();
                return;
            }
            let limit = if &command == b"DATA" { 65536 } else { 1024 };
            if size > limit {
                self.fail("invalid SYNC length");
                return;
            }
            if self.input.len() < size + 8 {
                return;
            }
            let payload = &self.input[8..8 + size];
            match &command {
                b"LIST" => {
                    if self.output.len() + usize::from(wallpaper::builtin_count()) * 32 + 20 > 1024
                    {
                        self.fail("read pending SYNC response first");
                        return;
                    }
                    let path = core::str::from_utf8(payload)
                        .unwrap_or("")
                        .trim_end_matches('/');
                    if path != "/sdcard/Wallpapers" {
                        self.fail("directory not available");
                        return;
                    }
                    for index in 1..=wallpaper::builtin_count() {
                        use core::fmt::Write;
                        let mut name = heapless::String::<12>::new();
                        let _ = write!(name, "{}.png", index);
                        self.output.extend_from_slice(b"DENT");
                        self.output.extend_from_slice(&0o100444_u32.to_le_bytes());
                        self.output.extend_from_slice(
                            &(wallpaper::builtin_png(index).map_or(0, |v| v.len()) as u32)
                                .to_le_bytes(),
                        );
                        self.output.extend_from_slice(&0_u32.to_le_bytes());
                        self.output
                            .extend_from_slice(&(name.len() as u32).to_le_bytes());
                        self.output.extend_from_slice(name.as_bytes());
                    }
                    self.output.extend_from_slice(b"DONE");
                    self.output.extend_from_slice(&[0; 16]);
                }
                b"STAT" => {
                    if self.output.len() > 1008 {
                        self.fail("read pending SYNC response first");
                        return;
                    }
                    self.output.extend_from_slice(b"STAT");
                    let bytes =
                        wallpaper::builtin_file(core::str::from_utf8(payload).unwrap_or(""));
                    let dir = core::str::from_utf8(payload)
                        .unwrap_or("")
                        .trim_end_matches('/')
                        == "/sdcard/Wallpapers";
                    self.output.extend_from_slice(
                        &if dir {
                            0o040755_u32
                        } else if bytes.is_some() {
                            0o100444_u32
                        } else {
                            0
                        }
                        .to_le_bytes(),
                    );
                    self.output
                        .extend_from_slice(&(bytes.map_or(0, |v| v.len()) as u32).to_le_bytes());
                    self.output.extend_from_slice(&0_u32.to_le_bytes());
                }
                b"RECV" => {
                    if self.sending || self.download.is_some() {
                        self.fail("another transfer is active");
                        return;
                    }
                    if let Some(bytes) =
                        wallpaper::builtin_file(core::str::from_utf8(payload).unwrap_or(""))
                    {
                        self.download = Some((bytes, 0));
                    } else {
                        self.fail("read-only files: /sdcard/Wallpapers/1.png through 16.png");
                        return;
                    }
                }
                b"SEND" => {
                    if self.download.is_some() {
                        self.fail("download in progress");
                        return;
                    }
                    let value = core::str::from_utf8(payload).unwrap_or("");
                    let path = value
                        .rsplit_once(',')
                        .map(|(path, _)| path)
                        .unwrap_or(value);
                    if wallpaper::builtin_file(path).is_some() {
                        self.fail("built-in wallpaper is read-only; upload current.png");
                        return;
                    }
                    if !path.starts_with("/sdcard/Wallpapers/")
                        || !path.ends_with(".png")
                        || path.contains("..")
                    {
                        self.fail("use /sdcard/Wallpapers/name.png (240x280 PNG)");
                        return;
                    }
                    self.file.clear();
                    self.sending = true;
                }
                b"DATA" => {
                    if !self.sending || self.file.len() + size > MAX_FILE {
                        self.fail("wallpaper must be at most 320 KiB");
                        return;
                    }
                    self.file.extend_from_slice(payload);
                }
                _ => {
                    self.fail("only wallpaper STAT/SEND/DATA/DONE are supported");
                    return;
                }
            }
            self.input.drain(..size + 8);
        }
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        if self.waiting {
            wallpaper::cancel_upload();
        }
    }
}
