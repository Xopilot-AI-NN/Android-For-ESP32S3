//! Existing raw-HCI BLE scan/connect compatibility layer.
//! This is mutually exclusive with the optional TrouBLE HID host.
use super::*;

impl RadioServices {
    pub fn set_bluetooth(&mut self, enabled: bool) {
        if enabled {
            if !self.ensure_ble() { return; }
            self.bt_requested = true;
            if !self.ble_advertising && self.ble_scanning_until.is_none() {
                self.enable_advertising();
            }
            println!("radio: bluetooth service enabled");
        } else {
            if let Some(ble) = self.ble.as_mut() {
                if self.ble_scanning_until.is_some() { let _ = hci_command(ble, 0x200c, &[0, 0]); }
                if self.ble_advertising { let _ = hci_command(ble, 0x200a, &[0]); }
                if let Some(connection) = self.ble_connection {
                    let params = [connection.handle as u8, (connection.handle >> 8) as u8, 0x13];
                    let _ = hci_command(ble, 0x0406, &params);
                }
            }
            self.ble_scanning_until = None;
            self.ble_advertising = false;
            self.ble_pending_address = None;
            self.ble_pending_since = None;
            self.ble_connection = None;
            self.bt_requested = false;
            println!("radio: bluetooth service disabled");
        }
    }

    pub(super) fn ensure_ble(&mut self) -> bool {
        if self.ble.is_some() { return true; }
        let free = internal_free_bytes();
        if free < MIN_BLE_INTERNAL_FREE {
            println!(
                "radio: bluetooth start refused: only {} KiB internal heap free (need >= {} KiB)",
                free / 1024,
                MIN_BLE_INTERNAL_FREE / 1024
            );
            return false;
        }
        let Some(bt) = self.bt_device.take() else {
            println!("radio: bluetooth peripheral unavailable until reboot");
            return false;
        };
        match BleConnector::new(self.controller, bt, Default::default()) {
            Ok(connector) => {
                self.ble = Some(connector);
                println!("radio: BLE HCI initialized (internal_free={} KiB)", internal_free_bytes() / 1024);
                true
            }
            Err(e) => {
                println!("radio: BLE HCI init failed: {:?}", e);
                false
            }
        }
    }

    pub(super) fn enable_advertising(&mut self) {
        let Some(ble) = self.ble.as_mut() else { return; };
        // ADV_IND, public address, all channels, 100 ms interval.
        let params = [0xa0,0x00, 0xa0,0x00, 0x00, 0x00, 0x00, 0,0,0,0,0,0, 0x07, 0x00];
        if !hci_command(ble, 0x2006, &params) { return; }

        let mut data = [0u8; 32];
        let name = b"AOSP Wear OS";
        let ad_len = 3 + 2 + name.len();
        data[0] = ad_len as u8;
        data[1..4].copy_from_slice(&[2, 0x01, 0x06]);
        data[4] = (name.len() + 1) as u8;
        data[5] = 0x09;
        data[6..6 + name.len()].copy_from_slice(name);
        if !hci_command(ble, 0x2008, &data) { return; }
        if hci_command(ble, 0x200a, &[1]) {
            self.ble_advertising = true;
            println!("radio: BLE advertising as 'AOSP Wear OS'");
        }
    }

    pub fn start_ble_scan(&mut self) -> bool {
        if !self.ensure_ble() { return false; }
        self.bt_requested = true;
        let Some(ble) = self.ble.as_mut() else { return false; };
        if self.ble_advertising {
            let _ = hci_command(ble, 0x200a, &[0]);
            self.ble_advertising = false;
        }
        // Active scan, 100 ms interval / 50 ms window, public address.
        let params = [0x01, 0xa0,0x00, 0x50,0x00, 0x00, 0x00];
        if !hci_command(ble, 0x200b, &params) { return false; }
        if !hci_command(ble, 0x200c, &[1, 1]) { return false; }
        self.ble_devices.clear();
        self.ble_scanning_until = Some(now_ms() + BLE_SCAN_MS);
        println!("radio: BLE active scan started");
        true
    }

    pub fn connect_ble(&mut self, address: [u8; 6]) -> bool {
        if !self.ensure_ble() { return false; }
        self.bt_requested = true;
        if self.ble_connection.is_some() {
            println!("radio: BLE already connected");
            return false;
        }
        let address_type = self.ble_devices.iter()
            .find(|d| d.address == address)
            .map(|d| d.address_type)
            .unwrap_or(0x00);
        let Some(ble) = self.ble.as_mut() else { return false; };
        if self.ble_scanning_until.is_some() {
            let _ = hci_command(ble, 0x200c, &[0, 0]);
            self.ble_scanning_until = None;
        }
        if self.ble_advertising {
            let _ = hci_command(ble, 0x200a, &[0]);
            self.ble_advertising = false;
        }
        // LE Create Connection.  The address is stored in HCI byte order.
        let mut params = [0u8; 25];
        params[0..2].copy_from_slice(&0x0060u16.to_le_bytes()); // scan interval 60 ms
        params[2..4].copy_from_slice(&0x0030u16.to_le_bytes()); // scan window 30 ms
        params[4] = 0x00; // use peer address below
        params[5] = address_type;
        params[6..12].copy_from_slice(&address);
        params[12] = 0x00; // public own address
        params[13..15].copy_from_slice(&0x0018u16.to_le_bytes()); // 30 ms min
        params[15..17].copy_from_slice(&0x0028u16.to_le_bytes()); // 50 ms max
        params[17..19].copy_from_slice(&0u16.to_le_bytes());
        params[19..21].copy_from_slice(&0x01f4u16.to_le_bytes()); // 5 s supervision
        params[21..23].copy_from_slice(&0u16.to_le_bytes());
        params[23..25].copy_from_slice(&0u16.to_le_bytes());
        if hci_command(ble, 0x200d, &params) {
            self.ble_pending_address = Some(address);
            self.ble_pending_since = Some(now_ms());
            println!("radio: BLE connection requested");
            true
        } else {
            false
        }
    }

    pub fn disconnect_ble(&mut self) -> bool {
        let Some(connection) = self.ble_connection else {
            // Cancel an in-flight LE Create Connection before clearing state.
            let pending = self.ble_pending_address.take().is_some();
            self.ble_pending_since = None;
            if pending {
                if let Some(ble) = self.ble.as_mut() {
                    return hci_command(ble, 0x200e, &[]);
                }
            }
            return false;
        };
        let Some(ble) = self.ble.as_mut() else { return false; };
        let params = [connection.handle as u8, (connection.handle >> 8) as u8, 0x13];
        if hci_command(ble, 0x0406, &params) {
            println!("radio: BLE disconnect requested handle={}", connection.handle);
            true
        } else { false }
    }

    pub(super) fn poll_ble(&mut self) {
        if self.ble.is_none() { return; }
        {
            let ble = self.ble.as_mut().expect("BLE checked above");
            let mut packet = [0u8; 260];
            for _ in 0..8 {
                let Ok(n) = ble.next(&mut packet) else { break; };
                if n == 0 { break; }
                parse_ble_event(
                    &packet[..n],
                    &mut self.ble_devices,
                    &mut self.ble_connection,
                    &mut self.ble_pending_address,
                );
            }
            // A lost LE Connection Complete event must not leave BLE
            // permanently pending (and unable to advertise or reconnect).
            if self.ble_pending_address.is_none() {
                self.ble_pending_since = None;
            } else if self.ble_pending_since
                .map(|since| now_ms().saturating_sub(since) >= 15_000)
                .unwrap_or(false)
            {
                let _ = hci_command(ble, 0x200e, &[]); // LE Create Connection Cancel
                self.ble_pending_address = None;
                self.ble_pending_since = None;
                println!("radio: BLE connection request timed out; canceled");
            }
            if let Some(until) = self.ble_scanning_until {
                if now_ms() >= until {
                    let _ = hci_command(ble, 0x200c, &[0, 0]);
                    self.ble_scanning_until = None;
                    println!("radio: BLE scan complete: {} device(s)", self.ble_devices.len());
                }
            }
        }
        if self.bt_requested
            && self.ble_connection.is_none()
            && self.ble_pending_address.is_none()
            && self.ble_scanning_until.is_none()
            && !self.ble_advertising
        {
            self.enable_advertising();
        }
    }

    pub fn format_bt_status<const N: usize>(&self, out: &mut HString<N>) {
        let _ = write!(
            out,
            "enabled={} hci={} advertising={} scanning={} devices={} connected={} pending={}",
            self.bt_requested as u8,
            self.ble.is_some() as u8,
            self.ble_advertising as u8,
            self.ble_scanning_until.is_some() as u8,
            self.ble_devices.len(),
            self.ble_connection.is_some() as u8,
            self.ble_pending_address.is_some() as u8,
        );
        if let Some(connection) = self.ble_connection {
            let a = connection.address;
            let _ = write!(out, " peer={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", a[5],a[4],a[3],a[2],a[1],a[0]);
        }
    }

    pub fn format_bt_scan<const N: usize>(&self, out: &mut HString<N>) {
        if self.ble_devices.is_empty() {
            let _ = out.push_str(if self.ble_scanning_until.is_some() { "scan in progress" } else { "no BLE devices discovered" });
            return;
        }
        for (i, dev) in self.ble_devices.iter().enumerate() {
            if i != 0 { let _ = out.push_str("; "); }
            if dev.name.is_empty() { let _ = out.push_str("BLE"); } else { let _ = out.push_str(dev.name.as_str()); }
            let a = dev.address;
            let _ = write!(out, " {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} rssi={}", a[5],a[4],a[3],a[2],a[1],a[0], dev.rssi);
        }
    }
}
