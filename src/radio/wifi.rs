//! Wi-Fi station, profiles, scans and association.
use super::*;

impl RadioServices {
    pub fn set_wifi(&mut self, enabled: bool) {
        if enabled {
            if !self.ensure_wifi_started() {
                return;
            }
            self.wifi_requested = true;
            // Do not perform a synchronous active scan merely because Wi-Fi
            // was toggled on. esp-radio scanning can hold the main loop for a
            // few seconds and, on USB-PC storage builds, starve the shared
            // USB-Serial-JTAG transport. Wear-style network discovery is
            // requested explicitly when the Available networks page opens.
            if let Some(credentials) = self.saved_wifi {
                let _ = self.connect_wifi(credentials);
            }
        } else {
            self.disconnect_wifi();
        }
    }

    pub(super) fn ensure_wifi_started(&mut self) -> bool {
        if matches!(self.wifi.is_started(), Ok(true)) {
            return true;
        }
        let free = internal_free_bytes();
        if free < MIN_WIFI_INTERNAL_FREE {
            println!(
                "radio: wifi start refused: only {} KiB internal heap free (need >= {} KiB)",
                free / 1024,
                MIN_WIFI_INTERNAL_FREE / 1024
            );
            return false;
        }
        if let Err(e) = self
            .wifi
            .set_config(&ModeConfig::Client(ClientConfig::default()))
        {
            println!("radio: wifi config failed: {:?}", e);
            return false;
        }
        if let Err(e) = self.wifi.start() {
            println!("radio: wifi start failed: {:?}", e);
            return false;
        }
        // start() is explicitly non-blocking in esp-radio 0.17.  Scanning
        // immediately after it can race StaStart and yield a misleading empty
        // list, so wait briefly for the driver state machine to settle.
        let delay = Delay::new();
        let mut started = false;
        for _ in 0..100 {
            if matches!(self.wifi.is_started(), Ok(true)) {
                started = true;
                break;
            }
            delay.delay_millis(10);
        }
        if !started {
            println!("radio: wifi start timed out");
            return false;
        }
        self.net.set_link_state(LinkState::LinkUp);
        println!(
            "radio: wifi STA enabled (internal_free={} KiB)",
            internal_free_bytes() / 1024
        );
        true
    }

    pub fn scan_wifi(&mut self) -> bool {
        if !self.ensure_wifi_started() {
            return false;
        }
        self.wifi_requested = true;
        self.wifi_aps.clear();
        println!("radio: scanning Wi-Fi networks...");
        // Keep explicit foreground discovery short enough that the crown UI
        // and shared USB debug/storage transport remain responsive. Nearby
        // watch/phone hotspots are normally discovered within this dwell; an
        // empty first pass is retried below after the PHY settles.
        let scan_config = ScanConfig::default()
            .with_scan_type(ScanTypeConfig::Active {
                min: Duration::from_millis(20),
                max: Duration::from_millis(70),
            })
            .with_max(10);
        let mut result = self.wifi.scan_with_config(scan_config);
        if matches!(&result, Ok(aps) if aps.is_empty()) {
            // A first scan right after radio bring-up can still race the PHY.
            Delay::new().delay_millis(120);
            result = self.wifi.scan_with_config(scan_config);
        }
        match result {
            Ok(aps) => {
                for ap in aps.iter() {
                    let mut ssid = HString::<32>::new();
                    push_safe_text(&mut ssid, ap.ssid.as_str());
                    let _ = self.wifi_aps.push(WifiAp {
                        ssid,
                        rssi: ap.signal_strength,
                        channel: ap.channel,
                    });
                }
                println!("radio: scan found {} AP(s)", self.wifi_aps.len());
                for ap in self.wifi_aps.iter() {
                    println!(
                        "radio: ap ssid={} rssi={} ch={}",
                        ap.ssid, ap.rssi, ap.channel
                    );
                }
                true
            }
            Err(e) => {
                println!("radio: scan failed: {:?}", e);
                false
            }
        }
    }

    pub fn connect_wifi(&mut self, credentials: WifiCredentials) -> bool {
        if !self.ensure_wifi_started() {
            return false;
        }
        let Some(ssid) = credentials.ssid() else {
            println!("radio: invalid SSID encoding");
            return false;
        };
        let Some(password) = credentials.password() else {
            println!("radio: invalid password encoding");
            return false;
        };
        if ssid.is_empty() {
            println!("radio: refusing empty SSID");
            return false;
        }

        if self.saved_wifi == Some(credentials) && self.wifi.is_connected().unwrap_or(false) {
            self.wifi_requested = true;
            println!(
                "radio: already associated with {}; keeping current link",
                ssid
            );
            return true;
        }

        // Proven-good ESP32-S3 path from Zephyr 0.4: let the Espressif
        // station driver perform its own AP selection.  Scans remain available
        // for UI/diagnostics, but we deliberately do not pin BSSID/channel here.
        // Phone hotspots can rotate/reconfigure their BSSID while remaining on
        // the same SSID and pinning the scan result made association fragile.
        let _ = self.wifi.disconnect();
        let config = ModeConfig::Client(
            ClientConfig::default()
                .with_ssid(ssid.into())
                .with_password(password.into()),
        );
        if let Err(e) = self.wifi.set_config(&config) {
            println!("radio: set network failed: {:?}", e);
            return false;
        }
        if !matches!(self.wifi.is_started(), Ok(true)) {
            if let Err(e) = self.wifi.start() {
                println!("radio: wifi start before association failed: {:?}", e);
                return false;
            }
        }

        match self.wifi.connect() {
            Ok(()) => {
                self.connecting_wifi = Some(credentials);
                self.wifi_requested = true;
                self.wifi_connect_started = Some(now_ms());
                self.wifi_reconnect_at = 0;
                self.net.set_link_state(LinkState::Associating);
                println!("radio: associating with {} (driver AP selection)", ssid);
                true
            }
            Err(e) => {
                println!("radio: connect request failed: {:?}", e);
                self.wifi_connect_started = None;
                self.wifi_reconnect_at = now_ms() + WIFI_RECONNECT_BACKOFF_MS;
                false
            }
        }
    }

    pub(super) fn reset_wifi_station(&mut self) {
        let delay = Delay::new();
        let _ = self.wifi.disconnect();
        delay.delay_millis(WIFI_DRIVER_SETTLE_MS);

        if matches!(self.wifi.is_started(), Ok(true)) {
            if let Err(e) = self.wifi.stop() {
                println!("radio: wifi stop during reset failed: {:?}", e);
            }
            for _ in 0..100 {
                if !matches!(self.wifi.is_started(), Ok(true)) {
                    break;
                }
                delay.delay_millis(10);
            }
        }

        let _ = self.wifi.set_config(&ModeConfig::None);
        self.net.reset();
    }

    pub fn disconnect_wifi(&mut self) {
        let _ = self.wifi.disconnect();
        if matches!(self.wifi.is_started(), Ok(true)) {
            if let Err(e) = self.wifi.stop() {
                println!("radio: wifi stop failed: {:?}", e);
            }
        }
        let _ = self.wifi.set_config(&ModeConfig::None);
        self.wifi_requested = false;
        self.connecting_wifi = None;
        self.wifi_connect_started = None;
        self.wifi_reconnect_at = 0;
        self.wifi_failures = 0;
        self.net.reset();
        println!("radio: wifi disabled");
    }

    pub fn format_wifi_status<const N: usize>(&self, out: &mut HString<N>) {
        let _ = write!(
            out,
            "enabled={} state={:?} aps={}",
            self.wifi_requested as u8,
            self.net.link_state(),
            self.wifi_aps.len()
        );
        if let Some(ip) = self.net.ip() {
            let _ = write!(out, " ip={}", ip);
        }
        if matches!(self.net.link_state(), LinkState::Online) {
            if let Ok(rssi) = self.wifi.rssi() {
                let _ = write!(out, " rssi={}", rssi);
            }
        }
        if let Some(c) = self.saved_wifi {
            if let Some(ssid) = c.ssid() {
                let _ = out.push_str(" saved=");
                push_safe_text(out, ssid);
            }
        }
        let _ = write!(
            out,
            " wadb={} listener={} client={}",
            self.net.wadb_active() as u8,
            self.net.wadb_active() as u8,
            self.net.wadb_seen_client() as u8
        );
    }

    #[cfg(feature = "pc-block-boot")]
    pub fn format_wifi_scan<const N: usize>(&self, out: &mut HString<N>) {
        if self.wifi_aps.is_empty() {
            let _ = out.push_str("no access points found");
            return;
        }
        for (i, ap) in self.wifi_aps.iter().enumerate() {
            if i != 0 {
                let _ = out.push_str("; ");
            }
            let _ = write!(out, "{} rssi={} ch={}", ap.ssid, ap.rssi, ap.channel);
        }
    }
}
