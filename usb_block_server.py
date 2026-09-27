#!/usr/bin/env python3
"""Serve a raw Zephyr Watch GPT image over ESP32-S3 USB-Serial-JTAG.

The serial device has exactly one owner: this process.  A desktop viewer attaches
through a local Unix socket, so opening/closing the GUI never opens /dev/ttyACM*
and therefore cannot disturb DTR/RTS, steal bytes from the block protocol, or
reset/re-enumerate the ESP32-S3.
"""
from __future__ import annotations

import argparse
import glob
import os
import queue
import selectors
import socket
import sys
import threading
import zlib
from pathlib import Path

SECTOR = 512
PREFIX = b"@ZBLK|"

try:
    import termios
except ImportError:  # non-POSIX
    termios = None


def default_viewer_socket() -> Path:
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if runtime and os.path.isdir(runtime):
        return Path(runtime) / "zephyr-watch-viewer.sock"
    uid = os.getuid() if hasattr(os, "getuid") else 0
    return Path(f"/tmp/zephyr-watch-viewer-{uid}.sock")


def auto_port() -> str:
    candidates: list[str] = []
    for pattern in ("/dev/ttyACM*", "/dev/ttyUSB*"):
        candidates.extend(sorted(glob.glob(pattern)))
    if not candidates:
        raise SystemExit("No /dev/ttyACM* or /dev/ttyUSB* device found; pass --port explicitly")
    return candidates[0]


def _disable_hupcl(ser) -> None:
    if termios is None or os.name != "posix":
        return
    try:
        attrs = termios.tcgetattr(ser.fileno())
        attrs[2] &= ~termios.HUPCL
        termios.tcsetattr(ser.fileno(), termios.TCSANOW, attrs)
    except (OSError, termios.error):
        pass


def open_serial(port: str, timeout: float):
    try:
        import serial  # type: ignore
    except ImportError as exc:
        raise SystemExit("pyserial is required: python -m pip install pyserial") from exc

    # Configure line state while CLOSED.  More importantly, this is the only
    # process that will own the TTY; desktop_viewer.py talks to our Unix socket.
    ser = serial.Serial()
    ser.port = port
    ser.baudrate = 115200  # ignored by native USB-Serial-JTAG
    ser.timeout = timeout
    ser.write_timeout = 5
    ser.xonxoff = False
    ser.rtscts = False
    ser.dsrdtr = False
    ser.dtr = False
    ser.rts = False
    if os.name == "posix":
        try:
            ser.exclusive = True
        except (AttributeError, ValueError):
            pass
    ser.open()
    _disable_hupcl(ser)
    return ser


class ViewerBridge:
    """Small local line-oriented fanout/broker for desktop_viewer.py."""

    def __init__(self, path: Path):
        self.path = path
        self.commands: queue.Queue[str] = queue.Queue(maxsize=128)
        self.stop_event = threading.Event()
        self.clients: set[socket.socket] = set()
        self.clients_lock = threading.Lock()
        self.thread = threading.Thread(target=self._run, name="zephyr-viewer-bridge", daemon=True)

    def start(self) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        try:
            self.path.unlink()
        except FileNotFoundError:
            pass
        self.thread.start()

    def close(self) -> None:
        self.stop_event.set()
        # Wake selector/accept without touching the serial device.
        try:
            wake = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            wake.settimeout(0.1)
            wake.connect(str(self.path))
            wake.close()
        except OSError:
            pass
        self.thread.join(timeout=1.0)
        with self.clients_lock:
            for client in list(self.clients):
                try:
                    client.close()
                except OSError:
                    pass
            self.clients.clear()
        try:
            self.path.unlink()
        except FileNotFoundError:
            pass

    def pop_command(self) -> str | None:
        try:
            return self.commands.get_nowait()
        except queue.Empty:
            return None

    def broadcast(self, line: str) -> None:
        payload = (line.rstrip("\r\n") + "\n").encode("utf-8", errors="replace")
        dead: list[socket.socket] = []
        with self.clients_lock:
            for client in self.clients:
                try:
                    client.sendall(payload)
                except (BrokenPipeError, ConnectionResetError, OSError):
                    dead.append(client)
            for client in dead:
                self.clients.discard(client)
                try:
                    client.close()
                except OSError:
                    pass

    def _queue_command(self, line: str) -> None:
        line = line.strip("\r\n")
        if not line:
            return
        try:
            self.commands.put_nowait(line)
        except queue.Full:
            # Interactive input prefers newest state, exactly like the viewer.
            try:
                self.commands.get_nowait()
            except queue.Empty:
                pass
            try:
                self.commands.put_nowait(line)
            except queue.Full:
                pass

    def _run(self) -> None:
        selector = selectors.DefaultSelector()
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server.bind(str(self.path))
        os.chmod(self.path, 0o600)
        server.listen(4)
        server.setblocking(False)
        selector.register(server, selectors.EVENT_READ, data=None)
        buffers: dict[socket.socket, bytearray] = {}

        try:
            while not self.stop_event.is_set():
                for key, _ in selector.select(timeout=0.2):
                    if key.data is None:
                        try:
                            client, _ = server.accept()
                        except OSError:
                            continue
                        client.setblocking(False)
                        buffers[client] = bytearray()
                        with self.clients_lock:
                            self.clients.add(client)
                        selector.register(client, selectors.EVENT_READ, data=client)
                        continue

                    client = key.data
                    try:
                        chunk = client.recv(4096)
                    except BlockingIOError:
                        continue
                    except OSError:
                        chunk = b""
                    if not chunk:
                        try:
                            selector.unregister(client)
                        except Exception:
                            pass
                        with self.clients_lock:
                            self.clients.discard(client)
                        buffers.pop(client, None)
                        try:
                            client.close()
                        except OSError:
                            pass
                        continue

                    buf = buffers[client]
                    buf.extend(chunk)
                    while b"\n" in buf:
                        raw, _, rest = buf.partition(b"\n")
                        buf[:] = rest
                        self._queue_command(raw.rstrip(b"\r").decode("utf-8", errors="replace"))
        finally:
            for client in list(buffers):
                try:
                    selector.unregister(client)
                except Exception:
                    pass
                try:
                    client.close()
                except OSError:
                    pass
            try:
                selector.unregister(server)
            except Exception:
                pass
            server.close()
            selector.close()


def read_exact(ser, count: int, timeout_budget: int = 50) -> bytes:
    out = bytearray()
    empty_reads = 0
    while len(out) < count:
        chunk = ser.read(count - len(out))
        if not chunk:
            empty_reads += 1
            if empty_reads >= timeout_budget:
                raise TimeoutError(f"timeout while receiving {count} bytes ({len(out)} received)")
            continue
        empty_reads = 0
        out += chunk
    return bytes(out)


def send_line(ser, text: str) -> None:
    ser.write(text.encode("ascii") + b"\r\n")
    ser.flush()


def fail(ser, message: str) -> None:
    safe = message.replace("|", "/").replace("\r", " ").replace("\n", " ")
    send_line(ser, f"@ZBLK|FAIL|{safe}")


def serve(
    port: str,
    image_path: Path,
    read_only: bool,
    timeout: float,
    viewer_socket: Path,
    viewer_bridge_enabled: bool,
) -> None:
    mode = "rb" if read_only else "r+b"
    bridge = ViewerBridge(viewer_socket) if viewer_bridge_enabled else None
    if bridge is not None:
        bridge.start()

    try:
        with image_path.open(mode, buffering=0) as disk:
            size = os.fstat(disk.fileno()).st_size
            if size == 0 or size % SECTOR:
                raise SystemExit(f"image size must be a non-zero multiple of {SECTOR}: {size}")
            blocks = size // SECTOR
            if blocks > 0xFFFF_FFFF:
                raise SystemExit("image is too large for the current 32-bit LBA transport")

            ser = open_serial(port, timeout)
            interactive = False
            try:
                print("Zephyr USB PC block server")
                print(f"  port : {port} (exclusive single owner, no-reset)")
                print(f"  image: {image_path} ({size // (1024 * 1024)} MiB, {blocks} sectors)")
                print(f"  mode : {'read-only' if read_only else 'read/write'}")
                if bridge is not None:
                    print(f"  viewer bridge: {viewer_socket}")
                    print("  viewer: python desktop_viewer.py   (does NOT open the TTY)")
                print("Waiting for bootloader requests...")

                while True:
                    # Viewer commands are held until the ESP explicitly says it
                    # finished boot-time block traffic. This keeps binary block
                    # replies and interactive UI commands from interleaving.
                    if interactive and bridge is not None:
                        while True:
                            command = bridge.pop_command()
                            if command is None:
                                break
                            try:
                                send_line(ser, command)
                            except Exception as exc:
                                # A board reset/panic can temporarily invalidate
                                # USB-Serial-JTAG. Exit the transport cleanly
                                # instead of dumping a pyserial traceback.
                                print(f"[bridge] serial write failed: {exc}")
                                bridge.broadcast("@HOST|BOARD_DISCONNECTED")
                                return

                    try:
                        raw = ser.readline()
                    except Exception as exc:
                        print(f"[bridge] serial read failed: {exc}")
                        if bridge is not None:
                            bridge.broadcast("@HOST|BOARD_DISCONNECTED")
                        return
                    if not raw:
                        continue

                    line = raw.rstrip(b"\r\n")
                    if not line.startswith(PREFIX):
                        text = line.decode("utf-8", errors="replace")
                        if text:
                            # PONG is a liveness heartbeat, not a user-facing log.
                            # Keep forwarding it to the viewer but avoid flooding
                            # the terminal every few seconds.
                            if bridge is not None:
                                bridge.broadcast(text)
                            if not text.startswith("@ZWUI|PONG|"):
                                print(f"[esp] {text}")
                        continue

                    try:
                        text = line.decode("ascii")
                        parts = text.split("|")
                        if parts[:2] == ["@ZBLK", "HELLO"]:
                            send_line(ser, "@ZBLK|READY|1")
                            continue

                        if parts == ["@ZBLK", "INTERACTIVE"]:
                            interactive = True
                            print("[bridge] ESP entered interactive viewer mode")
                            if bridge is not None:
                                bridge.broadcast("@HOST|BOARD_READY")
                            continue

                        if parts == ["@ZBLK", "SIZE"]:
                            send_line(ser, f"@ZBLK|SIZE|{blocks}")
                            continue

                        if len(parts) == 4 and parts[:2] == ["@ZBLK", "READ"]:
                            lba = int(parts[2], 10)
                            count = int(parts[3], 10)
                            if count <= 0 or lba < 0 or lba + count > blocks:
                                fail(ser, "read out of range")
                                continue
                            byte_count = count * SECTOR
                            disk.seek(lba * SECTOR)
                            payload = disk.read(byte_count)
                            if len(payload) != byte_count:
                                fail(ser, "short image read")
                                continue
                            crc = zlib.crc32(payload) & 0xFFFF_FFFF
                            send_line(ser, f"@ZBLK|DATA|{byte_count}|{crc:08x}")
                            ser.write(payload)
                            ser.flush()
                            continue

                        if len(parts) == 6 and parts[:2] == ["@ZBLK", "WRITE"]:
                            lba = int(parts[2], 10)
                            count = int(parts[3], 10)
                            byte_count = int(parts[4], 10)
                            expected_crc = int(parts[5], 16)
                            if count <= 0 or byte_count != count * SECTOR:
                                fail(ser, "invalid write length")
                                continue

                            payload = read_exact(ser, byte_count)
                            if lba < 0 or lba + count > blocks:
                                fail(ser, "write out of range")
                                continue
                            if (zlib.crc32(payload) & 0xFFFF_FFFF) != expected_crc:
                                fail(ser, "write crc mismatch")
                                continue
                            if read_only:
                                fail(ser, "server is read-only")
                                continue

                            disk.seek(lba * SECTOR)
                            written = disk.write(payload)
                            if written != byte_count:
                                fail(ser, "short image write")
                                continue
                            disk.flush()
                            os.fsync(disk.fileno())
                            send_line(ser, "@ZBLK|OK")
                            continue

                        fail(ser, "unknown command")
                    except (ValueError, UnicodeError) as exc:
                        fail(ser, f"bad command: {exc}")
                    except TimeoutError as exc:
                        fail(ser, str(exc))
            finally:
                try:
                    ser.dtr = False
                    ser.rts = False
                except Exception:
                    pass
                ser.close()
    finally:
        if bridge is not None:
            bridge.close()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", help="serial device; default: first /dev/ttyACM* or /dev/ttyUSB*")
    parser.add_argument(
        "--image",
        type=Path,
        default=Path("out/zephyr-watch-sd.img"),
        help="raw GPT image (default: out/zephyr-watch-sd.img)",
    )
    parser.add_argument("--read-only", action="store_true", help="reject writes (A/B boot metadata then cannot update)")
    parser.add_argument("--timeout", type=float, default=0.1, help="serial polling timeout in seconds")
    parser.add_argument("--viewer-socket", type=Path, default=default_viewer_socket())
    parser.add_argument("--no-viewer-bridge", action="store_true")
    args = parser.parse_args()

    if not args.image.is_file():
        raise SystemExit(
            f"image not found: {args.image}\n"
            "Create one with: python tools/mk_android_sd.py --output out/zephyr-watch-sd.img --size-mib 256 --version dev"
        )
    serve(
        args.port or auto_port(),
        args.image,
        args.read_only,
        args.timeout,
        args.viewer_socket,
        not args.no_viewer_bridge,
    )


if __name__ == "__main__":
    main()
