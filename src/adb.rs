//! Small ADB service core shared by the USB development bridge and wireless
//! TCP transport. The packet header matches Android's 24-byte ADB wire ABI.

use core::fmt::Write;
pub mod logcat;
pub mod sync;

pub const A_CNXN: u32 = u32::from_le_bytes(*b"CNXN");
pub const A_OPEN: u32 = u32::from_le_bytes(*b"OPEN");
pub const A_OKAY: u32 = u32::from_le_bytes(*b"OKAY");
pub const A_CLSE: u32 = u32::from_le_bytes(*b"CLSE");
pub const A_WRTE: u32 = u32::from_le_bytes(*b"WRTE");

pub use crate::device_info::{
    HOSTNAME, PRODUCT_DEVICE, PRODUCT_MANUFACTURER, PRODUCT_MODEL, PRODUCT_NAME,
};

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
        for (i, word) in [
            self.command,
            self.arg0,
            self.arg1,
            self.data_length,
            self.data_check,
            self.magic,
        ]
        .into_iter()
        .enumerate()
        {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        out
    }
}

pub fn checksum(payload: &[u8]) -> u32 {
    payload
        .iter()
        .fold(0u32, |sum, &b| sum.wrapping_add(b as u32))
}

fn prop(key: &str) -> Option<&'static str> {
    match key {
        "ro.product.locale" | "persist.sys.locale" => Some(crate::material::DEFAULT_LOCALE),
        "ro.product.name" => Some(PRODUCT_NAME),
        "ro.product.model" => Some(PRODUCT_MODEL),
        "ro.product.device" => Some(PRODUCT_DEVICE),
        "ro.product.manufacturer" => Some(PRODUCT_MANUFACTURER),
        "ro.product.brand" => Some(crate::device_info::PRODUCT_BRAND),
        "ro.product.marketname" => Some(PRODUCT_MODEL),
        "ro.build.version.release" => Some("17"),
        "ro.build.version.sdk" => Some("37"),
        "ro.build.version.codename" => Some("REL"),
        "ro.build.version.incremental" => Some("AOSP17QPR1.17.2.1"),
        "ro.build.type" => Some("userdebug"),
        "ro.build.tags" => Some("test-keys"),
        "ro.build.flavor" => Some("aosp-userdebug"),
        "ro.build.display.id" => Some("AOSP Wear OS Android 17 QPR1"),
        "ro.build.fingerprint" => Some("aosp/aosp/aosp:17/QPR1/17.2.1:userdebug/test-keys"),
        "ro.boot.dynamic_partitions" => Some("true"),
        "ro.treble.enabled" => Some("true"),
        "ro.aosp_esp32.runtime" => Some("rhai"),
        "ro.aosp_esp32.arch" => Some("xtensa-lx7"),
        "ro.aosp_esp32.avb" => Some("orange"),
        "ro.aosp_esp32.ui" => Some("wearos-material3-expressive"),
        "ro.aosp_esp32.shell" => Some("wear-runtime-v8"),
        "ro.build.characteristics" => Some("watch"),
        "ro.product.first_api_level" => Some("37"),
        "ro.build.version.security_patch" => Some("2026-09-05"),
        "ro.system.build.version.release" => Some("17"),
        "ro.system.build.version.sdk" => Some("37"),
        "ro.wear.platform" => Some("aosp"),
        "ro.aosp_esp32.version" => Some("17.2.1"),
        "net.hostname" => Some(HOSTNAME),
        _ => None,
    }
}

const PROPS: &[&str] = &[
    "ro.product.locale",
    "persist.sys.locale",
    "ro.product.name",
    "ro.product.model",
    "ro.product.device",
    "ro.product.manufacturer",
    "ro.product.brand",
    "ro.product.marketname",
    "ro.build.version.release",
    "ro.build.version.sdk",
    "ro.build.version.codename",
    "ro.build.version.incremental",
    "ro.build.type",
    "ro.build.tags",
    "ro.build.flavor",
    "ro.build.display.id",
    "ro.build.fingerprint",
    "ro.boot.dynamic_partitions",
    "ro.treble.enabled",
    "ro.aosp_esp32.runtime",
    "ro.aosp_esp32.arch",
    "ro.aosp_esp32.avb",
    "ro.aosp_esp32.ui",
    "ro.aosp_esp32.shell",
    "ro.build.characteristics",
    "ro.product.first_api_level",
    "ro.build.version.security_patch",
    "ro.system.build.version.release",
    "ro.system.build.version.sdk",
    "ro.wear.platform",
    "ro.aosp_esp32.version",
    "net.hostname",
    "persist.sys.device_name",
];

/// Compact property summary used by the local ZADB bridge.
#[cfg(feature = "pc-block-boot")]
pub fn getprop() -> heapless::String<512> {
    let mut out = heapless::String::new();
    let _ = write!(
        out,
        "ro.product.name={}; ro.product.model={}; ro.product.device={}; ro.product.manufacturer={}; persist.sys.device_name={}",
        PRODUCT_NAME,
        PRODUCT_MODEL,
        PRODUCT_DEVICE,
        PRODUCT_MANUFACTURER,
        crate::device_name::snapshot()
    );
    out
}

pub fn services() -> &'static str {
    "servicemanager surfaceflinger package activity power input connectivity wifi bluetooth adb notification media_session alarm display battery launcher wearable zpager"
}

pub fn packages() -> &'static str {
    "com.android.systemui com.android.wear.launcher com.android.wear.tiles com.android.wear.media com.android.wear.assistant com.android.settings com.android.deskclock com.android.settings.connectivity android.system"
}

/// Execute the tiny userdebug shell used by wireless ADB. This is deliberately
/// bounded and deterministic; the transport layer supplies interactive line
/// editing/prompt handling and this function only executes one command.
pub fn shell_command<const N: usize>(service: &str, out: &mut heapless::String<N>) {
    let cmd = if let Some(cmd) = service.strip_prefix("shell:") {
        cmd
    } else if service.starts_with("shell,v2") {
        service.rsplit_once(':').map(|(_, cmd)| cmd).unwrap_or("")
    } else {
        service
    }
    .trim();

    if cmd == "getprop" {
        for key in PROPS {
            if *key == "persist.sys.device_name" {
                let _ = writeln!(out, "[{}]: [{}]", key, crate::device_name::snapshot());
                continue;
            }
            if let Some(value) = prop(key) {
                let _ = writeln!(out, "[{}]: [{}]", key, value);
            }
        }
        return;
    }
    if let Some(key) = cmd.strip_prefix("getprop ") {
        if key.trim() == "persist.sys.device_name" {
            let _ = writeln!(out, "{}", crate::device_name::snapshot());
            return;
        }
        if let Some(value) = prop(key.trim()) {
            let _ = writeln!(out, "{}", value);
        }
        return;
    }

    if let Some(value) = cmd.strip_prefix("settings put global device_name ") {
        let value = value.trim();
        let value = value
            .strip_prefix('\'')
            .and_then(|v| v.strip_suffix('\''))
            .or_else(|| value.strip_prefix('"').and_then(|v| v.strip_suffix('"')))
            .unwrap_or(value);
        if !crate::device_name::set(value) {
            let _ = writeln!(
                out,
                "invalid device name: use 1..32 UTF-8 bytes without control characters"
            );
        }
        return;
    }
    match cmd {
        "service list" => {
            for (idx, item) in services().split(' ').enumerate() {
                let _ = writeln!(out, "{}\t{}: [android.os.IService]", idx, item);
            }
        }
        "pm list packages" | "cmd package list packages" => {
            for item in packages().split(' ') {
                let _ = writeln!(out, "package:{}", item);
            }
        }
        "dumpsys" => {
            let _ = writeln!(out, "Android 17 QPR1 system_server: running");
            let _ = writeln!(
                out,
                "SurfaceFlinger: RGB565/ST7789 Wear Material 3 Expressive"
            );
            let _ = writeln!(
                out,
                "WearLauncher: expressive curved list / crown navigation"
            );
            let _ = writeln!(out, "NotificationManager: wearable notification stream");
            let _ = writeln!(out, "MediaSessionService: companion transport ready");
            let _ = writeln!(out, "ConnectivityService: esp-radio/smoltcp");
            let _ = writeln!(out, "BluetoothService: ESP32-S3 BLE HCI");
            let _ = writeln!(out, "ZPager: enabled (32 MiB backing store)");
            let _ = writeln!(out, "adbd: tcp:5555 userdebug");
        }
        "pm list features" | "cmd package list features" => {
            let _ = writeln!(out, "feature:android.hardware.type.watch");
            let _ = writeln!(out, "feature:android.hardware.wifi");
            let _ = writeln!(out, "feature:android.hardware.bluetooth_le");
        }
        "dumpsys display" => {
            let _ = writeln!(out, "DISPLAY MANAGER (dumpsys display)");
            let _ = writeln!(
                out,
                "mDefaultDisplay=DisplayDeviceInfo{{AOSP Wear OS, 240 x 280, density 280}}"
            );
            let _ = writeln!(out, "state=ON, colorMode=RGB565, panel=ST7789V3");
        }
        "dumpsys notification" => {
            let _ = writeln!(out, "NotificationManagerService: running");
            let _ = writeln!(out, "wearNotificationStream=true");
            let _ = writeln!(out, "channels: android.system, wireless_debug");
        }
        "dumpsys media_session" => {
            let _ = writeln!(out, "MEDIA SESSION SERVICE");
            let _ = writeln!(out, "activeSessions=0 companionTransport=ready");
        }
        "dumpsys alarm" => {
            let _ = writeln!(
                out,
                "AlarmManagerService: clock tools available; wall clock source=companion/manual"
            );
        }
        "settings get global wear_launcher_ui_mode" => {
            let _ = writeln!(out, "1");
        }
        "settings get global device_provisioned" => {
            let _ = writeln!(out, "1");
        }
        "settings get secure user_setup_complete" => {
            let _ = writeln!(out, "1");
        }
        "dumpsys wifi" => {
            let _ = writeln!(out, "Wi-Fi service: native esp-radio 0.17");
            let _ = writeln!(out, "Mode: STA / DHCP / reconnect enabled");
        }
        "dumpsys bluetooth" | "dumpsys bluetooth_manager" => {
            let _ = writeln!(out, "Bluetooth service: ESP32-S3 BLE HCI");
            let _ = writeln!(out, "Coexistence: Wi-Fi + BLE");
        }
        "dumpsys activity" | "dumpsys activity activities" => {
            let _ = writeln!(out, "ACTIVITY MANAGER ACTIVITIES (Android 17 QPR1)");
            let _ = writeln!(
                out,
                "mResumedActivity: com.android.systemui/.watch.WatchFaceActivity"
            );
        }
        "id" => {
            let _ = writeln!(out, "uid=2000(shell) gid=2000(shell) groups=2000(shell)");
        }
        "uname -a" => {
            let _ = writeln!(out, "Android aosp-wear 17-QPR1 ESP32-S3 Xtensa Rhai");
        }
        "hostname" => {
            let _ = writeln!(out, "{}", HOSTNAME);
        }
        "pwd" => {
            let _ = writeln!(out, "/");
        }
        "whoami" => {
            let _ = writeln!(out, "shell");
        }
        "wm size" => {
            let _ = writeln!(out, "Physical size: 240x280");
        }
        "wm density" => {
            let _ = writeln!(out, "Physical density: 280");
        }
        "cat /proc/version" => {
            let _ = writeln!(
                out,
                "Android 17 QPR1 AOSP Wear OS (ESP32-S3 Xtensa Rust + Rhai runtime)"
            );
        }
        "cat /proc/meminfo" => {
            let _ = writeln!(out, "MemTotal:        2144 kB");
            let _ = writeln!(out, "SwapTotal:      32636 kB");
            let _ = writeln!(out, "SwapBackend:    ZPager");
        }
        "settings get global device_name" | "settings get secure bluetooth_name" => {
            let _ = writeln!(out, "{}", crate::device_name::snapshot());
        }
        "help" => {
            let _ = writeln!(out, "getprop, id, uname -a, hostname, pwd, whoami");
            let _ = writeln!(out, "service list, pm list packages, pm list features");
            let _ = writeln!(
                out,
                "dumpsys [wifi|bluetooth|display|notification|media_session|alarm], wm size, wm density"
            );
        }
        "" => {}
        _ if cmd.starts_with("echo ") => {
            let _ = writeln!(out, "{}", &cmd[5..]);
        }
        _ => {
            let _ = writeln!(out, "sh: {}: not found", cmd);
        }
    }
}
