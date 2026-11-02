//! Link radio services, framework actions and the active system state.
use crate::persistence::*;
use crate::state_flow::*;
use crate::*;

#[cfg(feature = "radio-services")]
pub(crate) fn service_radio_runtime<D: BlockDevice>(
    radios: &mut radio::RadioServices,
    os: &mut runtime::Runtime,
    pager: Option<&page_store::PageStore>,
    dev: &D,
) -> Option<runtime::RuntimeFrame> {
    #[cfg(feature = "ble-hid")]
    if !radios.bonds_loaded {
        let mut raw = [0u8; crate::ble_bonds::MAX_BOND_BLOB];
        if let Some(pager) = pager {
            if let Ok(n) = pager.read(dev, 68, 0x424e_4431, &mut raw) {
                radios.restore_bonds(&raw[..n]);
            }
        }
        radios.bonds_loaded = true;
    }
    if rtc::take_network_sync_request() {
        radios.request_network_time_sync();
    }
    radios.tick(((os.state() >> 23) & 1) != 0);

    let bluetooth_redraw = {
        #[cfg(feature = "ble-hid")]
        {
            if let Some(action) = os.take_bluetooth_action() {
                radios.bluetooth_action(action);
            }
            let (status, pin) = radios.bluetooth_snapshot();
            let mut raw = [0u8; crate::ble_bonds::MAX_BOND_BLOB];
            if let Some(n) = radios.encode_dirty_bonds(&mut raw) {
                if let Some(pager) = pager {
                    if pager.write(dev, 68, 0x424e_4431, &raw[..n]).is_ok() {
                        radios.bonds_saved();
                    }
                }
            }
            if let Some(playback) = radios.take_playback() {
                os.set_playback(playback);
            }
            os.set_bluetooth_status(status, pin)
        }
        #[cfg(not(feature = "ble-hid"))]
        {
            let _ = os.take_bluetooth_action();
            os.set_bluetooth_status(0, None)
        }
    };

    // Commit Wi-Fi credentials only after real association succeeded. A typo in
    // the crown keyboard must never overwrite the previous known-good profile.
    if let Some(credentials) = radios.take_connected_wifi_for_persist() {
        if let Some(pager) = pager {
            if save_wifi_credentials(pager, dev, credentials) {
                println!("radio: verified Wi-Fi profile persisted");
            }
        }
    }

    let link = match radios.network_state() {
        net::LinkState::Down => 0,
        net::LinkState::Associating => 1,
        net::LinkState::LinkUp => 2,
        net::LinkState::Dhcp => 3,
        net::LinkState::Online => 4,
    };
    let mut redraw = os.set_wifi_link_state(link);
    redraw |= os.set_wifi_ip(radios.ip_address().map(|ip| ip.octets()));
    redraw |= bluetooth_redraw;

    if let Some(unix) = radios.take_network_time() {
        if rtc::set_network_unix_seconds(unix) {
            println!("time: software RTC synchronized from Wi-Fi SNTP");
            redraw = true;
        }
    }

    if let Some(action) = os.take_wifi_ui_action() {
        match action {
            runtime::WifiUiAction::Scan => {
                os.clear_wifi_scan();
                if radios.scan_wifi() {
                    for i in 0..radios.wifi_ap_count() {
                        if let Some((ssid, rssi)) = radios.wifi_ap_info(i) {
                            os.push_wifi_ap(ssid, rssi);
                        }
                    }
                }
                redraw = true;
            }
            runtime::WifiUiAction::Connect { ssid, password } => {
                if let Some(credentials) =
                    WifiCredentials::from_parts(ssid.as_str(), password.as_str())
                {
                    if radios.connect_wifi(credentials) {
                        let _ = set_runtime_flag(os, 21, true);
                    }
                }
                redraw = true;
            }
        }
    }
    if redraw { os.render().ok() } else { None }
}
