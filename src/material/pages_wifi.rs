//! pages wifi Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn wifi_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Wi-Fi", t);
        let active = f.cursor == 0;
        self.round_rect(
            17,
            65,
            206,
            61,
            30,
            Self::focus_fill(t.surface_high, active),
        );
        self.icon_wifi(
            42,
            95,
            if f.wifi {
                t.primary
            } else {
                t.on_surface_variant
            },
        );
        self.text(72, 75, "USE WI-FI", 1, t.on_surface);
        self.text(
            72,
            96,
            if f.wifi { "ON" } else { "OFF" },
            1,
            t.on_surface_variant,
        );
        self.toggle(168, 83, f.wifi, t);
        self.info_card(
            137,
            "NETWORKS",
            if f.wifi {
                "SCAN / SAVED"
            } else {
                "WI-FI IS OFF"
            },
            f.cursor == 1,
            t,
        );
        self.info_card(200, "BACK", "CONNECTIVITY", f.cursor == 2, t);
        if let Some(ip) = f.wifi_ip {
            let label = alloc::format!("IP {}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
            self.text_center(267, label.as_str(), 1, t.primary);
        }
    }

    pub(super) fn wifi_ssid<'a>(f: &'a MaterialFrame) -> &'a str {
        let n = (f.wifi_ap_ssid_len as usize).min(32);
        core::str::from_utf8(&f.wifi_ap_ssid[..n]).unwrap_or("UNKNOWN")
    }

    pub(super) fn wifi_networks(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Available networks", t);
        if f.wifi_ap_count == 0 {
            self.circle(120, 115, 33, t.surface_high);
            self.icon_wifi(120, 115, t.primary);
            self.text_center(175, "SCANNING...", 2, t.on_surface);
            self.text_center(203, "ROTATE CROWN", 1, t.on_surface_variant);
            self.round_rect(47, 237, 146, 33, 16, t.surface_high);
            self.text_center(249, "BACK", 1, t.on_surface);
            return;
        }
        if f.cursor >= f.wifi_ap_count {
            self.round_rect(19, 82, 202, 132, 36, t.primary_container);
            self.text_center(120, "Back", 2, t.on_surface);
            self.text_center(155, "Return to Wi-Fi", 1, t.on_surface_variant);
        } else {
            self.round_rect(15, 78, 210, 141, 36, t.surface_high);
            self.icon_wifi(120, 108, t.on_surface);
            let name = Self::wifi_ssid(&f);
            self.text_center_raw(140, name, if name.len() > 16 { 1 } else { 2 }, t.on_surface);
            let signal = if f.wifi_ap_rssi > -55 {
                "EXCELLENT"
            } else if f.wifi_ap_rssi > -70 {
                "GOOD"
            } else {
                "WEAK"
            };
            self.text_center(165, signal, 1, t.primary);
            self.text_center(190, "PRESS TO CONNECT", 1, t.on_surface);
        }
        for i in 0..f.wifi_ap_count.min(7) {
            let x = 120 - (f.wifi_ap_count.min(7) as i32 - 1) * 6 + i as i32 * 12;
            self.circle(
                x,
                243,
                if i == f.wifi_ap_index { 4 } else { 2 },
                if i == f.wifi_ap_index {
                    t.primary
                } else {
                    t.outline
                },
            );
        }
    }

    pub(super) fn wifi_password(&mut self, f: MaterialFrame, t: Theme) {
        let n = usize::from(f.wifi_password_preview_len).min(16);
        let visible = core::str::from_utf8(&f.wifi_password_preview[..n]).unwrap_or("");
        self.text_keyboard(f, t, "Пароль Wi-Fi", visible, "Ввод");
    }

    pub(super) fn wifi_connecting(&mut self, f: MaterialFrame, t: Theme) {
        self.title("WI-FI", t);
        self.circle(120, 102, 38, t.surface);
        self.icon_wifi(
            120,
            102,
            if f.wifi_link_state == 4 {
                t.primary
            } else {
                t.outline
            },
        );
        let (title, sub) = match f.wifi_link_state {
            4 => ("CONNECTED", "TIME SYNC STARTED"),
            3 => ("GETTING ADDRESS", "DHCP"),
            2 => ("LINK READY", "STARTING NETWORK"),
            1 => ("CONNECTING", "ASSOCIATING"),
            _ => ("NOT CONNECTED", "PRESS TO RETURN"),
        };
        self.text_center(
            159,
            title,
            2,
            if f.wifi_link_state == 4 {
                t.primary
            } else {
                t.on_surface
            },
        );
        self.text_center(187, sub, 1, t.on_surface_variant);
        self.round_rect(48, 222, 144, 38, 19, t.surface_high);
        self.text_center(234, "DONE", 1, t.on_surface);
    }
}
