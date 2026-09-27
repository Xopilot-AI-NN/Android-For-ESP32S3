# Zephyr Watch Remote Surface v2

Remote Surface v2 mirrors boot UI state from the **real ESP32-S3** over the native USB Serial/JTAG CDC channel.
The PC does not emulate the MCU or execute the bootloader.

## Board -> Host

- `@ZWUI|HELLO|2|240|280|Zephyr Watch|ESP32-S3-Zero-N4R2`
- `@ZWUI|BOOT`
- `@ZWUI|PROGRESS|<label>|<phase 0..7>`
- `@ZWUI|STATUS|<title>|<line1>|<line2>|<line3>`
- `@ZWUI|ERROR|<code>|<detail>`
- `@ZWUI|PONG|2`

## Host -> Board

- `@ZWIN|SYNC` — replay the current logical screen/context
- `@ZWIN|PING` — liveness probe; never changes boot mode
- `@ZWIN|NORMAL`
- `@ZWIN|RECOVERY`
- `@ZWIN|FASTBOOT`
- `@ZWIN|ROTATE|-1`
- `@ZWIN|ROTATE|1`
- `@ZWIN|POWER`

All messages are ASCII lines terminated by `\n` or `\r\n`.

## No-reset host attach

The v2 viewer opens pySerial in two stages: the object remains closed while DTR and RTS are set inactive, and
only then is `/dev/ttyACM*` opened. On Linux it also clears `HUPCL` when the driver supports termios. This avoids
intentional modem-control reset pulses and keeps close/reconnect cycles from acting like a serial hangup.

The viewer has a background reconnect loop. If the board disappears and comes back with another `ttyACM` number,
it scans again, reconnects, sends `SYNC`, and resumes. Firmware-side RX has an 8-event queue so multiple commands in
one USB packet are not silently lost.

## Future v3

For the full watch UI, add dirty-tile packets with RGB565/RLE payloads. The renderer stays on the ESP32-S3; the PC
only displays changed tiles.

## Protocol v3: Material semantic scenes

Firmware 0.2 adds:

`@ZWUI|M3|screen|cursor|brightness|dnd|airplane|theme`

The desktop viewer reconstructs the same Material scene without receiving a
full framebuffer. This keeps USB-PC block traffic small and leaves the ESP32-S3
as the authoritative UI computer.
