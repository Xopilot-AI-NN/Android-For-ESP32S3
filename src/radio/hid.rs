//! HID and raw HCI are exclusive: no second reader may consume host packets.
use super::*;
use crate::{ble_host::BleHost, runtime::BluetoothUiAction};

impl RadioServices {
    pub fn set_bluetooth(&mut self, enabled: bool) {
        self.bt_requested = enabled;
        if !enabled {
            self.hid_error = false;
        }
        if enabled && self.hid.is_none() {
            if internal_free_bytes() < MIN_BLE_INTERNAL_FREE {
                println!("BLE: insufficient internal heap");
                self.hid_error = true;
                return;
            }
            let Some(bt) = self.bt_device.take() else {
                println!("BLE: controller unavailable; reboot required");
                self.hid_error = true;
                return;
            };
            match BleConnector::new(self.controller, bt, Default::default()) {
                Ok(connector) => {
                    let mut address = esp_hal::efuse::Efuse::mac_address();
                    address.reverse(); // HCI uses little-endian Bluetooth addresses.
                    self.hid = Some(BleHost::new(connector, address, HVec::new()));
                    // A newly created host must restore its bonds before its first poll.
                    self.bonds_loaded = false;
                    self.hid_error = false;
                    println!("BLE: HID controller created, restoring bonds before startup");
                }
                Err(e) => {
                    println!("BLE: controller init failed: {:?}", e);
                    self.hid_error = true;
                    return;
                }
            }
        }
        if let Some(host) = self.hid.as_mut() {
            host.set_enabled(enabled);
        }
    }

    pub(super) fn poll_ble(&mut self) {
        if self.bonds_loaded {
            if let Some(host) = self.hid.as_mut() {
                host.poll();
            }
        }
    }

    pub fn take_playback(&mut self) -> Option<crate::media::Playback> {
        self.hid.as_mut().and_then(BleHost::take_playback)
    }
    pub fn bluetooth_snapshot(&self) -> (u8, Option<u32>) {
        if self.hid_error {
            return (5, None);
        }
        self.hid
            .as_ref()
            .map(BleHost::snapshot)
            .unwrap_or((0, None))
    }

    pub fn bluetooth_action(&mut self, action: BluetoothUiAction) {
        if let BluetoothUiAction::Pair = action {
            self.set_bluetooth(true);
        }
        let Some(host) = self.hid.as_mut() else {
            return;
        };
        match action {
            BluetoothUiAction::Pair => host.pair(),
            BluetoothUiAction::Confirm(yes) => host.confirm(yes),
            BluetoothUiAction::Disconnect => host.disconnect(),
            BluetoothUiAction::Forget => host.forget(),
            BluetoothUiAction::Media(usage) => {
                if !host.media(usage) {
                    println!("BLE: media unavailable until encrypted and subscribed");
                }
            }
        }
    }

    pub fn restore_bonds(&mut self, raw: &[u8]) {
        if let Some(host) = self.hid.as_mut() {
            if let Ok(bonds) = crate::ble_host::decode_bonds(raw) {
                host.restore_bonds(bonds);
            }
        }
    }

    pub fn encode_dirty_bonds(&self, out: &mut [u8]) -> Option<usize> {
        let bonds = self.hid.as_ref()?.bonds_to_save()?;
        crate::ble_host::encode_bonds(&bonds, out).ok()
    }

    pub fn bonds_saved(&mut self) {
        if let Some(host) = self.hid.as_mut() {
            host.saved();
        }
    }

    // Keep desktop command compatibility. A HID peripheral pairs from the phone
    // rather than initiating an unauthenticated central HCI connection.
    pub fn start_ble_scan(&mut self) -> bool {
        self.bluetooth_action(BluetoothUiAction::Pair);
        self.hid.is_some()
    }
    pub fn connect_ble(&mut self, _address: [u8; 6]) -> bool {
        false
    }
    pub fn disconnect_ble(&mut self) -> bool {
        let connected = self.bluetooth_snapshot().0 >= 2;
        self.bluetooth_action(BluetoothUiAction::Disconnect);
        connected
    }
    pub fn format_bt_status<const N: usize>(&self, out: &mut HString<N>) {
        let _ = write!(
            out,
            "enabled={} hid_state={} pairing_secs={} internal_free={}",
            self.bt_requested as u8,
            self.bluetooth_snapshot().0,
            self.hid
                .as_ref()
                .map(|host| host.pairing_remaining())
                .unwrap_or(0),
            internal_free_bytes()
        );
    }
    pub fn format_bt_scan<const N: usize>(&self, out: &mut HString<N>) {
        let _ = out.push_str("HID peripheral: pair AOSP Wear in phone Bluetooth settings");
    }
}
