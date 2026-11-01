//! pages bluetooth Material surface implementation.
use super::*;

impl Surface {
    pub(super) fn bluetooth_page(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Bluetooth", t);
        self.round_rect(
            13,
            64,
            214,
            61,
            30,
            if f.cursor == 0 {
                t.primary_container
            } else {
                t.surface_high
            },
        );
        self.icon_bt(
            44,
            94,
            if f.bt {
                t.primary
            } else {
                t.on_surface_variant
            },
        );
        self.text(74, 86, "Bluetooth", 1, t.on_surface);
        self.toggle(169, 81, f.bt, t);
        self.info_card(
            136,
            "PAIR PHONE / PC",
            if f.bt {
                "OPEN PAIRING"
            } else {
                "TURN ON BLUETOOTH"
            },
            f.cursor == 1,
            t,
        );
        // Firmware currently handles cursor 2 as Back, not Media. Never draw a fake action.
        self.info_card(205, "BACK", "CONNECTIVITY", f.cursor == 2, t);
    }

    pub(super) fn display_settings(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Display", t);
        self.round_rect(
            15,
            64,
            210,
            68,
            29,
            if f.cursor == 0 {
                t.primary_container
            } else {
                t.surface_high
            },
        );
        self.icon_sun(44, 96, t.primary);
        self.text(68, 75, "Brightness", 2, t.on_surface);
        self.slider(70, 105, 129, f.brightness, t);
        self.round_rect(
            15,
            144,
            210,
            61,
            29,
            if f.cursor == 1 {
                t.primary_container
            } else {
                t.surface_high
            },
        );
        self.text(33, 154, "COLOR THEME", 1, t.on_surface_variant);
        let name = match f.theme {
            1 => "Blue",
            2 => "Purple",
            3 => "Coral",
            _ => "Teal",
        };
        self.text(33, 176, name, 2, t.on_surface);
        self.circle(192, 173, 15, t.primary);
        self.round_rect(
            33,
            217,
            174,
            42,
            21,
            if f.cursor == 2 {
                t.primary_container
            } else {
                t.surface_high
            },
        );
        self.text_center(231, "Back", 2, t.on_surface);
    }

    pub(super) fn brightness_editor(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Brightness", t);
        self.circle(120, 105, 39, t.primary_container);
        self.icon_sun(120, 105, t.primary);
        self.slider(24, 187, 192, f.brightness, t);
        self.text_center(224, "ROTATE CROWN", 1, t.on_surface);
        self.text_center(245, "PRESS TO CLOSE", 1, t.on_surface_variant);
    }

    pub(super) fn bluetooth_pairing(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Bluetooth", t);
        self.circle(120, 92, 36, t.primary_container);
        self.icon_bt(120, 92, t.primary);
        self.text_center(153, "AOSP Wear", 2, t.on_surface);
        self.text_center(
            181,
            if f.bt {
                "Pairing requires BLE HID"
            } else {
                "Turn on Bluetooth"
            },
            1,
            t.on_surface_variant,
        );
        self.round_rect(25, 217, 190, 40, 20, t.surface_high);
        self.text_center(230, "LONG PRESS TO GO BACK", 1, t.on_surface);
    }
}
