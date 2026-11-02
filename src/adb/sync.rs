//! Restricted ADB SYNC v1 receiver for PNG wallpaper uploads, not a fake filesystem.
use crate::wallpaper;
use alloc::vec::Vec;
const MAX_FILE: usize = 320 * 1024;
pub struct Receiver {
    input: Vec<u8>,
    file: Vec<u8>,
    pub output: Vec<u8>,
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
            sending: false,
            waiting: false,
            finished: false,
        }
    }
    fn fail(&mut self, reason: &str) {
        self.output.clear();
        self.output.extend_from_slice(b"FAIL");
        self.output
            .extend_from_slice(&(reason.len() as u32).to_le_bytes());
        self.output.extend_from_slice(reason.as_bytes());
        self.finished = true;
    }
    pub fn poll(&mut self) {
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
                b"STAT" => {
                    self.output.extend_from_slice(b"STAT");
                    self.output.extend_from_slice(&[0; 12]);
                }
                b"SEND" => {
                    let value = core::str::from_utf8(payload).unwrap_or("");
                    let path = value
                        .rsplit_once(',')
                        .map(|(path, _)| path)
                        .unwrap_or(value);
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
