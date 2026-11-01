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
    pager_used: &mut u32,
) -> Option<runtime::RuntimeFrame> {
    if rtc::take_network_sync_request() {
        radios.request_network_time_sync();
    }
    radios.tick(((os.state() >> 23) & 1) != 0);

    // The browser remote uses the same RuntimeEvent path as the physical crown
    // and desktop viewer, so it cannot bypass normal navigation semantics.
    if let Some(web) = radios.take_web_event() {
        let event = match web {
            net::WebRemoteEvent::Rotate(d) => runtime::RuntimeEvent::Rotate(d as i32),
            net::WebRemoteEvent::Press => runtime::RuntimeEvent::Press,
            net::WebRemoteEvent::Back => runtime::RuntimeEvent::Back,
            net::WebRemoteEvent::Home => runtime::RuntimeEvent::Home,
            net::WebRemoteEvent::Swipe(d) => runtime::RuntimeEvent::Swipe(d as i32),
        };
        if let Ok(frame) = dispatch_with_pager(os, event, pager, dev, pager_used) {
            radios.sync_frame(&frame);
            return Some(frame);
        }
    }

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
