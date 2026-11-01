//! HCI protocol helpers. Only radio.rs owns the controller and packet reads.
use super::*;

pub(super) fn push_safe_text<const N: usize>(out: &mut HString<N>, text: &str) {
    for ch in text.chars() {
        let safe = if ch == '\r' || ch == '\n' || ch == '|' {
            '?'
        } else {
            ch
        };
        if out.push(safe).is_err() {
            break;
        }
    }
}

pub(super) fn hci_command(ble: &mut BleConnector<'static>, opcode: u16, params: &[u8]) -> bool {
    if params.len() > 255 {
        return false;
    }
    let mut packet = [0u8; 260];
    packet[0] = 0x01;
    packet[1..3].copy_from_slice(&opcode.to_le_bytes());
    packet[3] = params.len() as u8;
    packet[4..4 + params.len()].copy_from_slice(params);
    if let Err(e) = ble.write(&packet[..4 + params.len()]) {
        println!("radio: HCI command 0x{:04x} failed: {:?}", opcode, e);
        return false;
    }
    // Give the controller task a short opportunity to consume the command. A
    // command-complete event is intentionally left for poll_ble to drain.
    Delay::new().delay_millis(8);
    true
}

pub(super) fn parse_ble_event(
    packet: &[u8],
    out: &mut HVec<BleDevice, 10>,
    connection: &mut Option<BleConnection>,
    pending: &mut Option<[u8; 6]>,
) {
    // VHCI uses H4 framing. Be tolerant of a backend without the H4 type byte.
    let (event_code, params) = if packet.len() >= 3 && packet[0] == 0x04 {
        let plen = packet[2] as usize;
        if packet.len() < 3 + plen {
            return;
        }
        (packet[1], &packet[3..3 + plen])
    } else if packet.len() >= 2 {
        let plen = packet[1] as usize;
        if packet.len() < 2 + plen {
            return;
        }
        (packet[0], &packet[2..2 + plen])
    } else {
        return;
    };

    // LE Meta Event.
    if event_code == 0x3e && !params.is_empty() {
        match params[0] {
            0x01 => {
                // LE Connection Complete
                if params.len() < 12 {
                    return;
                }
                let status = params[1];
                let handle = u16::from_le_bytes([params[2], params[3]]) & 0x0fff;
                let mut address = [0u8; 6];
                address.copy_from_slice(&params[6..12]);
                if status == 0 {
                    *connection = Some(BleConnection { address, handle });
                    *pending = None;
                    println!("radio: BLE connected handle={}", handle);
                } else {
                    *pending = None;
                    println!("radio: BLE connection failed status=0x{:02x}", status);
                }
            }
            0x02 => parse_advertising_params(params, out),
            _ => {}
        }
        return;
    }

    // Disconnection Complete.
    if event_code == 0x05 && params.len() >= 4 {
        let handle = u16::from_le_bytes([params[1], params[2]]) & 0x0fff;
        if connection.as_ref().map(|c| c.handle) == Some(handle) {
            *connection = None;
            println!(
                "radio: BLE disconnected handle={} reason=0x{:02x}",
                handle, params[3]
            );
        }
    }
}

fn parse_advertising_params(params: &[u8], out: &mut HVec<BleDevice, 10>) {
    if params.len() < 2 || params[0] != 0x02 {
        return;
    }
    let reports = params[1] as usize;
    let mut pos = 2usize;
    for _ in 0..reports {
        if pos + 9 > params.len() {
            break;
        }
        let _event_type = params[pos];
        let address_type = params[pos + 1];
        let mut address = [0u8; 6];
        address.copy_from_slice(&params[pos + 2..pos + 8]);
        let data_len = params[pos + 8] as usize;
        pos += 9;
        if pos + data_len + 1 > params.len() {
            break;
        }
        let data = &params[pos..pos + data_len];
        pos += data_len;
        let rssi = params[pos] as i8;
        pos += 1;

        let mut name = HString::<24>::new();
        parse_local_name(data, &mut name);
        if let Some(existing) = out.iter_mut().find(|d| d.address == address) {
            existing.rssi = rssi;
            existing.address_type = address_type;
            if !name.is_empty() {
                existing.name = name;
            }
        } else {
            let _ = out.push(BleDevice {
                address,
                address_type,
                rssi,
                name,
            });
        }
    }
}

fn parse_local_name(data: &[u8], out: &mut HString<24>) {
    let mut pos = 0usize;
    while pos < data.len() {
        let len = data[pos] as usize;
        if len == 0 || pos + 1 + len > data.len() {
            break;
        }
        let kind = data[pos + 1];
        if kind == 0x09 || kind == 0x08 {
            for &b in &data[pos + 2..pos + 1 + len] {
                let ch = if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '?'
                };
                if out.push(ch).is_err() {
                    break;
                }
            }
            return;
        }
        pos += 1 + len;
    }
}
