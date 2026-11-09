//! pages bluetooth Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn bluetooth_page(&mut self, f: MaterialFrame, t: Theme) {
        self.title("BLUETOOTH", t);
        self.round_rect(
            18,
            52,
            204,
            58,
            29,
            if f.cursor == 0 {
                t.primary_container
            } else {
                t.surface
            },
        );
        self.icon_bt(47, 81, if f.bt { t.primary } else { t.outline });
        self.text(72, 66, "BLUETOOTH", 1, t.on_surface);
        self.text(
            72,
            84,
            if f.bt { "ON" } else { "OFF" },
            1,
            t.on_surface_variant,
        );
        self.toggle(171, 69, f.bt, t);
        self.info_card(
            124,
            "PAIR NEW DEVICE",
            if f.bt {
                "READY TO SCAN"
            } else {
                "BLUETOOTH OFF"
            },
            f.cursor == 1,
            t,
        );
        self.info_card(188, "WATCH NAME", "AOSP WEAR", f.cursor == 2, t);
    }

    pub(super) fn display_settings(&mut self, f: MaterialFrame, t: Theme) {
        self.title("DISPLAY", t);
        self.round_rect(
            16,
            55,
            208,
            66,
            33,
            if f.cursor == 0 {
                t.primary_container
            } else {
                t.surface
            },
        );
        self.icon_sun(42, 86, t.primary);
        self.text(66, 68, "BRIGHTNESS", 1, t.on_surface_variant);
        self.slider(66, 94, 132, f.brightness, t);
        self.round_rect(
            16,
            135,
            208,
            58,
            29,
            if f.cursor == 1 {
                t.primary_container
            } else {
                t.surface
            },
        );
        self.text(38, 148, "DYNAMIC COLOR", 1, t.on_surface_variant);
        let color = match f.theme {
            1 => "BLUE",
            2 => "PURPLE",
            3 => "CORAL",
            _ => "GREEN",
        };
        self.text(38, 168, color, 2, t.primary);
        self.round_rect(
            16,
            207,
            208,
            42,
            21,
            if f.cursor == 2 {
                t.primary_container
            } else {
                t.surface
            },
        );
        self.text_center(219, "BACK", 1, t.on_surface);
    }

    pub(super) fn brightness_editor(&mut self, f: MaterialFrame, t: Theme) {
        self.title("BRIGHTNESS", t);
        self.icon_sun(120, 100, t.primary);
        self.text_center(139, "ROTATE CROWN", 1, t.on_surface_variant);
        self.slider(28, 179, 184, f.brightness, t);
        self.text_center(224, "PRESS OR BACK TO EXIT", 1, t.on_surface);
    }

    pub(super) fn bluetooth_pairing(&mut self, f: MaterialFrame, t: Theme) {
        self.title("BLUETOOTH", t);
        self.icon_bt(120, 96, t.primary);
        self.text_center(
            143,
            if f.bt { "PAIRING" } else { "BLUETOOTH OFF" },
            2,
            t.on_surface,
        );
        self.text_center(186, "PAIRING NOT YET WIRED", 1, t.on_surface_variant);
        self.text_center(229, "HOME TO EXIT", 1, t.on_surface_variant);
    }
}
