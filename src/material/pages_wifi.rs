//! pages wifi Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn wifi_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Wi-Fi", t);
        let active = f.cursor == 0;
        self.round_rect(
            if active { 10 } else { 17 },
            65,
            if active { 220 } else { 206 },
            61,
            30,
            if active {
                t.primary_container
            } else {
                t.surface_high
            },
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
        self.info_card(207, "BACK", "CONNECTIVITY", f.cursor == 2, t);
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
            self.text_center(140, name, if name.len() > 16 { 1 } else { 2 }, t.on_surface);
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
        const ALPHA: &[u8] = b"qwertyuiopasdfghjklzxcvbnm";
        const SYMBOLS_1: &[u8] = b"1234567890!@#$%^&*()_+-=[]";
        const SYMBOLS_2: &[u8] = b"{}.,:;?'\"/\\|<>`~";

        let page = (f.wifi_editor_char >> 6).min(3);
        let selected = (f.wifi_editor_char & 0x3f) as usize;
        let chars = match page {
            2 => SYMBOLS_1,
            3 => SYMBOLS_2,
            _ => ALPHA,
        };

        // Compact Gboard-like password editor: no suggestion strip, only the
        // field and keyboard.  It is laid out for one-dimensional encoder
        // focus, so the selected key lifts/enlarges instead of relying on touch.
        self.text_center(5, Self::wifi_ssid(&f), 1, t.on_surface_variant);
        self.round_rect(12, 21, 216, 40, 18, t.surface);
        if f.wifi_password_len == 0 {
            self.text(26, 37, "PASSWORD", 1, t.outline);
        } else {
            // Crown-only input is easy to overshoot by one detent. Show the
            // actual tail of the password on the physical watch so a wrong
            // neighbouring key is visible before CONNECT is pressed. The
            // preview is intentionally not serialized to desktop/web remotes.
            let n = f.wifi_password_preview_len.min(16) as usize;
            let visible = core::str::from_utf8(&f.wifi_password_preview[..n]).unwrap_or("?");
            self.text(25, 35, visible, 1, t.on_surface);
            if f.wifi_password_len as usize > n {
                self.text(15, 35, "~", 1, t.outline);
            }
        }
        // Small lock-like dot shows that this is a password field.
        self.circle(211, 41, 5, t.primary_container);
        self.circle(211, 41, 2, t.primary);

        let draw_char_key =
            |this: &mut Surface, x: i32, y: i32, w: i32, h: i32, ch: u8, idx: usize| {
                let is_sel = selected == idx;
                let yy = if is_sel { y - 3 } else { y };
                let hh = if is_sel { h + 6 } else { h };
                let bg = if is_sel { t.primary } else { t.surface_high };
                let fg = if is_sel { t.on_primary } else { t.on_surface };
                this.round_rect(x, yy, w, hh, 8, bg);
                let one = [if page == 1 {
                    ch.to_ascii_uppercase()
                } else {
                    ch
                }];
                let label = core::str::from_utf8(&one).unwrap_or("?");
                let scale = if is_sel { 2 } else { 1 };
                this.text_center_x(x + w / 2, yy + (hh - 7 * scale) / 2, label, scale, fg);
            };

        if page <= 1 {
            // QWERTY rows matching a phone keyboard silhouette.
            let rows = [10usize, 9usize, 7usize];
            let mut base = 0usize;
            for (r, count) in rows.into_iter().enumerate() {
                let y = 76 + r as i32 * 37;
                if r < 2 {
                    let kw = 20;
                    let gap = 2;
                    let row_w = count as i32 * kw + (count as i32 - 1) * gap;
                    let mut x = (WIDTH as i32 - row_w) / 2;
                    for pos in 0..count {
                        let idx = base + pos;
                        let ch = chars[idx];
                        draw_char_key(self, x, y, kw, 29, ch, idx);
                        x += kw + gap;
                    }
                } else {
                    // Third row is inset like Gboard; Shift/Delete live on the
                    // service row below, so all letters remain large enough.
                    let kw = 22;
                    let gap = 3;
                    let row_w = count as i32 * kw + (count as i32 - 1) * gap;
                    let mut x = (WIDTH as i32 - row_w) / 2;
                    for pos in 0..count {
                        let idx = base + pos;
                        let ch = chars[idx];
                        draw_char_key(self, x, y, kw, 29, ch, idx);
                        x += kw + gap;
                    }
                }
                base += count;
            }
        } else {
            let rows: &[usize] = if page == 2 { &[10, 10, 6] } else { &[9, 8] };
            let mut base = 0usize;
            for (r, &count) in rows.iter().enumerate() {
                let y = 76 + r as i32 * 37;
                let kw = if count >= 10 { 20 } else { 22 };
                let gap = 2;
                let row_w = count as i32 * kw + (count as i32 - 1) * gap;
                let mut x = (WIDTH as i32 - row_w) / 2;
                for pos in 0..count {
                    let idx = base + pos;
                    if idx >= chars.len() {
                        break;
                    }
                    draw_char_key(self, x, y, kw, 29, chars[idx], idx);
                    x += kw + gap;
                }
                base += count;
            }
        }

        let n = chars.len();
        let (page_key, alpha_key) = match page {
            2 => ("#+=", "ABC"),
            3 => ("123", "ABC"),
            _ => (if page == 1 { "abc" } else { "SHIFT" }, "123"),
        };
        // Service row mirrors a phone keyboard, but every key remains a simple
        // crown target in the same linear focus order.
        self.keyboard_key(6, 188, 50, 31, page_key, selected == n, t);
        self.keyboard_key(60, 188, 42, 31, alpha_key, selected == n + 1, t);
        self.keyboard_key(106, 188, 74, 31, "SPACE", selected == n + 2, t);
        self.keyboard_key(184, 188, 50, 31, "DEL", selected == n + 3, t);

        self.keyboard_key(6, 226, 64, 34, "BACK", selected == n + 5, t);
        self.keyboard_key(76, 226, 158, 34, "CONNECT", selected == n + 4, t);

        // Make the encoder model discoverable without a permanent instruction
        // banner: selected action is obvious from the raised accent key.
        self.text_center(266, "ROTATE  /  PRESS", 1, t.outline);
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
