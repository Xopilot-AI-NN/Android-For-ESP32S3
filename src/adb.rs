//! Small ADB service core shared by the USB development bridge and wireless
//! TCP transport.  The packet header matches Android's 24-byte ADB wire ABI.

use core::fmt::Write;

pub const A_SYNC: u32 = u32::from_le_bytes(*b"SYNC");
pub const A_CNXN: u32 = u32::from_le_bytes(*b"CNXN");
pub const A_OPEN: u32 = u32::from_le_bytes(*b"OPEN");
pub const A_OKAY: u32 = u32::from_le_bytes(*b"OKAY");
pub const A_CLSE: u32 = u32::from_le_bytes(*b"CLSE");
pub const A_WRTE: u32 = u32::from_le_bytes(*b"WRTE");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub command: u32,
    pub arg0: u32,
    pub arg1: u32,
    pub data_length: u32,
    pub data_check: u32,
    pub magic: u32,
}

impl Header {
    pub fn decode(raw: &[u8; 24]) -> Option<Self> {
        let out = Self {
            command: u32::from_le_bytes(raw[0..4].try_into().ok()?),
            arg0: u32::from_le_bytes(raw[4..8].try_into().ok()?),
            arg1: u32::from_le_bytes(raw[8..12].try_into().ok()?),
            data_length: u32::from_le_bytes(raw[12..16].try_into().ok()?),
            data_check: u32::from_le_bytes(raw[16..20].try_into().ok()?),
            magic: u32::from_le_bytes(raw[20..24].try_into().ok()?),
        };
        (out.magic == !out.command).then_some(out)
    }

    pub fn encode(self) -> [u8; 24] {
        let mut out = [0u8; 24];
        for (i, word) in [self.command, self.arg0, self.arg1, self.data_length, self.data_check, self.magic]
            .into_iter()
            .enumerate()
        {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        out
    }
}

pub fn checksum(payload: &[u8]) -> u32 {
    payload.iter().fold(0u32, |sum, &b| sum.wrapping_add(b as u32))
}

pub fn getprop() -> &'static str {
    "ro.product.name=Zephyr Watch; ro.product.device=zero; ro.product.model=Zephyr Watch; ro.build.version.release=0.4; ro.zephyr.runtime=rhai; ro.zephyr.avb=orange"
}

pub fn services() -> &'static str {
    "servicemanager surfaceflinger package activity power input connectivity wifi bluetooth adb zpager"
}

pub fn packages() -> &'static str {
    "com.android.systemui.notifications by.xopilot.settings by.xopilot.clock com.android.settings.connectivity android.system.about"
}

/// Legacy shell service used by the minimal TCP adbd transport.  Keeping the
/// response bounded avoids allocating while Wi-Fi/BLE RTOS tasks are active.
pub fn shell_command<const N: usize>(service: &str, out: &mut heapless::String<N>) {
    let cmd = if let Some(cmd) = service.strip_prefix("shell:") {
        cmd
    } else if service.starts_with("shell,v2") {
        service.rsplit_once(':').map(|(_, cmd)| cmd).unwrap_or("")
    } else {
        service
    }.trim();
    match cmd {
        "getprop" => {
            for item in getprop().split("; ") {
                let _ = writeln!(out, "{}", item);
            }
        }
        "service list" => {
            for (idx, item) in services().split(' ').enumerate() {
                let _ = writeln!(out, "{}\t{}", idx, item);
            }
        }
        "pm list packages" | "cmd package list packages" => {
            for item in packages().split(' ') {
                let _ = writeln!(out, "package:{}", item);
            }
        }
        "dumpsys" => {
            let _ = writeln!(out, "Zephyr Android system_server: running");
            let _ = writeln!(out, "SurfaceFlinger: RGB565/ST7789");
            let _ = writeln!(out, "ConnectivityService: esp-radio/smoltcp");
            let _ = writeln!(out, "ZPager: enabled");
        }
        "id" => { let _ = writeln!(out, "uid=2000(shell) gid=2000(shell) groups=2000(shell)"); }
        "uname -a" => { let _ = writeln!(out, "ZephyrAndroid zero 0.4 ESP32-S3 Xtensa Rhai"); }
        "pwd" => { let _ = writeln!(out, "/"); }
        "whoami" => { let _ = writeln!(out, "shell"); }
        "" => { let _ = writeln!(out, "Zephyr Android shell"); }
        _ if cmd.starts_with("echo ") => { let _ = writeln!(out, "{}", &cmd[5..]); }
        _ => { let _ = writeln!(out, "sh: {}: not found", cmd); }
    }
}
