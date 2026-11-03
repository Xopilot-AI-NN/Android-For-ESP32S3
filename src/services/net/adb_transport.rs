// Модуль потоков беспроводного ADB.
//! ACK-paced ADB streams. Large command replies live in the stream, not stack.
use super::*;

/// Продолжает ADB-поток с учётом подтверждений и очереди вывода.
pub(super) fn service_stream(socket: &mut tcp::Socket<'_>, stream: &mut AdbStream) {
    if stream.mode == AdbStreamMode::Closed || stream.waiting_for_okay {
        return;
    }
    if let Some(sync) = stream.sync.as_mut() {
        sync.poll();
        if !sync.output.is_empty() {
            if send_packet(
                socket,
                A_WRTE,
                stream.device_id,
                stream.host_id,
                &sync.output,
            ) {
                sync.output.clear();
                stream.waiting_for_okay = true;
            }
        } else if sync.finished {
            let _ = send_packet(socket, A_CLSE, stream.device_id, stream.host_id, &[]);
            stream.clear();
        }
        return;
    }
    if stream.output_pos < stream.output.len() {
        let end = (stream.output_pos + 512).min(stream.output.len());
        if send_packet(
            socket,
            A_WRTE,
            stream.device_id,
            stream.host_id,
            &stream.output.as_bytes()[stream.output_pos..end],
        ) {
            stream.output_pos = end;
            stream.waiting_for_okay = true;
            if stream.close_after_okay {
                stream.close_deadline_ms = now_ms() + ADB_CLOSE_GRACE_MS;
            }
        }
        return;
    }
    stream.output.clear();
    stream.output_pos = 0;
    if let Some(logcat) = stream.logcat.as_mut() {
        if let Some(line) = logcat.next_line() {
            let _ = stream.output.push_str(&line);
        } else if logcat.finished() {
            stream.close_after_okay = true;
        }
        if !stream.output.is_empty() {
            return;
        }
    }
    if stream.close_after_okay {
        if send_packet(socket, A_CLSE, stream.device_id, stream.host_id, &[]) {
            stream.clear();
        }
    }
}

/// Выполняет разрешённую команду и формирует ограниченный ответ.
fn execute(socket: &mut tcp::Socket<'_>, stream: &mut AdbStream, command: &str) {
    stream.output.clear();
    stream.output_pos = 0;
    if adb::logcat::command(command).is_some() {
        match adb::logcat::Session::parse(command) {
            Ok(session) => {
                stream.mode = AdbStreamMode::Logcat;
                stream.logcat = Some(session);
                stream.close_after_okay = false;
            }
            Err(error) => {
                let _ = writeln!(stream.output, "logcat: {}", error);
                stream.close_after_okay = true;
            }
        }
    } else {
        adb::shell_command(command, stream.output.as_mut());
        if stream.mode == AdbStreamMode::Interactive {
            let _ = stream.output.push_str("aosp:/ $ ");
        } else {
            stream.close_after_okay = true;
        }
    }
    service_stream(socket, stream);
}

/// Разбирает входящий ADB-пакет и изменяет состояние потока.
pub(super) fn handle_adb(
    socket: &mut tcp::Socket<'_>,
    message: AdbMessage<'_>,
    stream: &mut AdbStream,
    next_id: &mut u32,
) {
    match message.header.command {
        A_CNXN => {
            let mut banner = HString::<256>::new();
            let _ = write!(
                banner,
                "device::ro.product.name={};ro.product.model={};ro.product.device={};ro.product.manufacturer={};features=;\0",
                adb::PRODUCT_NAME,
                adb::PRODUCT_MODEL,
                adb::PRODUCT_DEVICE,
                adb::PRODUCT_MANUFACTURER
            );
            let _ = send_packet(socket, A_CNXN, ADB_VERSION, ADB_MAX_DATA, banner.as_bytes());
        }
        A_OPEN => {
            let host = message.header.arg0;
            let end = message
                .payload
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(message.payload.len());
            let service = core::str::from_utf8(&message.payload[..end]).unwrap_or("");
            if !(service.starts_with("shell:") || service == "shell" || service == "sync:") {
                let _ = send_packet(socket, A_CLSE, 0, host, &[]);
                return;
            }
            if stream.mode != AdbStreamMode::Closed {
                let _ = send_packet(socket, A_CLSE, stream.device_id, stream.host_id, &[]);
            }
            let command = service.strip_prefix("shell:").unwrap_or("").trim();
            stream.open(
                host,
                next_local_id(next_id),
                if service == "sync:" {
                    AdbStreamMode::Sync
                } else if command.is_empty() {
                    AdbStreamMode::Interactive
                } else {
                    AdbStreamMode::OneShot
                },
            );
            if !send_packet(socket, A_OKAY, stream.device_id, host, &[]) {
                stream.clear();
                return;
            }
            if service == "sync:" {
                stream.sync = Some(adb::sync::Receiver::new());
                return;
            }
            if command.is_empty() {
                let _ = stream.output.push_str("AOSP wearable shell\r\naosp:/ $ ");
                service_stream(socket, stream);
            } else {
                println!("adbd: shell {}", command);
                execute(socket, stream, command);
            }
        }
        A_WRTE => {
            if !stream.matches(message.header.arg0, message.header.arg1) {
                let _ = send_packet(socket, A_CLSE, 0, message.header.arg0, &[]);
                return;
            }
            let _ = send_packet(socket, A_OKAY, stream.device_id, stream.host_id, &[]);
            if let Some(sync) = stream.sync.as_mut() {
                sync.push(message.payload);
                service_stream(socket, stream);
                return;
            }
            if stream.mode == AdbStreamMode::Logcat {
                if message.payload.contains(&3) || message.payload.contains(&4) {
                    stream.logcat = None;
                    stream.close_after_okay = true;
                }
                return;
            }
            if stream.mode != AdbStreamMode::Interactive || stream.waiting_for_okay {
                return;
            }
            let mut execute_line = false;
            for &b in message.payload {
                match b {
                    b'\r' | b'\n' => execute_line = true,
                    4 => {
                        let _ = stream.input.push_str("exit");
                        execute_line = true;
                    }
                    8 | 127 => {
                        stream.input.pop();
                    }
                    32..=126 => {
                        let _ = stream.input.push(b as char);
                    }
                    _ => {}
                }
            }
            if execute_line {
                let command = stream.input.clone();
                stream.input.clear();
                if command.trim() == "exit" {
                    stream.close_after_okay = true;
                    service_stream(socket, stream);
                } else {
                    execute(socket, stream, command.trim());
                }
            }
        }
        A_OKAY => {
            if stream.matches(message.header.arg0, message.header.arg1) {
                stream.waiting_for_okay = false;
                stream.close_deadline_ms = 0;
                service_stream(socket, stream);
            }
        }
        A_CLSE => {
            if stream.matches(message.header.arg0, message.header.arg1) || message.header.arg1 == 0
            {
                stream.clear();
            }
        }
        _ => {}
    }
}
