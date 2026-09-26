#!/usr/bin/env python3
"""Zephyr Watch USB Desktop Display v2.1.

The ESP32-S3 remains the real computer. This viewer only mirrors UI state and
sends input. The serial link is opened without intentionally toggling DTR/RTS,
uses Linux no-HUPCL when possible, and reconnects in a background thread.

Requirements:
    python -m pip install pyserial

Examples:
    python desktop_viewer.py
    python desktop_viewer.py --port /dev/ttyACM0
"""

from __future__ import annotations

import argparse
import os
import queue
import threading
import time
import tkinter as tk
from tkinter import ttk

try:
    import serial
    import serial.tools.list_ports
except ImportError as exc:
    raise SystemExit("pyserial is required: python -m pip install pyserial") from exc

try:
    import termios
except ImportError:  # Windows
    termios = None

LOGICAL_W = 240
LOGICAL_H = 280
ESPRESSIF_VID = 0x303A


def list_candidate_ports() -> list[str]:
    ports = list(serial.tools.list_ports.comports())
    esp = []
    preferred = []
    fallback = []
    for p in ports:
        device = p.device or ""
        text = " ".join(filter(None, [device, p.description, p.manufacturer, p.product])).lower()
        if p.vid == ESPRESSIF_VID:
            esp.append(device)
        elif "acm" in device.lower() or "esp" in text or "usb jtag" in text or "cdc" in text:
            preferred.append(device)
        elif "usb" in text or "serial" in text:
            fallback.append(device)
    return esp + preferred + fallback


def find_port() -> str | None:
    ports = list_candidate_ports()
    return ports[0] if ports else None


def _disable_hupcl(ser: serial.Serial) -> None:
    """Keep Linux from applying modem hangup semantics on close.

    DTR/RTS are already set inactive before open. HUPCL is an additional guard
    for disconnect/reconnect cycles. Failure is harmless on drivers that do not
    expose normal termios modem controls.
    """
    if termios is None or os.name != "posix":
        return
    try:
        attrs = termios.tcgetattr(ser.fileno())
        attrs[2] &= ~termios.HUPCL
        termios.tcsetattr(ser.fileno(), termios.TCSANOW, attrs)
    except (OSError, termios.error):
        pass


def open_no_reset(port: str) -> serial.Serial:
    # Important: construct CLOSED first. pySerial defaults DTR/RTS to active;
    # passing the port to Serial(...) opens it before we can change those
    # states and can create a short control-line pulse on ESP USB serial links.
    ser = serial.Serial()
    ser.baudrate = 115200
    ser.timeout = 0.05
    # Writes happen only on the background worker.  Keep them blocking so a
    # freshly-enumerated USB Serial/JTAG endpoint can finish becoming ready
    # instead of killing the worker with SerialTimeoutException.
    ser.write_timeout = None
    ser.xonxoff = False
    ser.rtscts = False
    ser.dsrdtr = False

    # Store inactive line states while the file descriptor is still closed.
    ser.dtr = False
    ser.rts = False
    ser.port = port
    if os.name == "posix":
        try:
            ser.exclusive = True
        except (AttributeError, ValueError):
            pass
    ser.open()
    _disable_hupcl(ser)
    return ser


class Link:
    """Asynchronous, auto-reconnecting board link.

    Tk never owns the serial file descriptor. UI actions are queued to the
    worker, so a slow/disconnected USB endpoint cannot freeze the viewer.
    """

    def __init__(self, requested_port: str | None, messages: queue.Queue[str]):
        self.requested_port = requested_port
        self.messages = messages
        self.port: str | None = None
        self.stop_event = threading.Event()
        self.outbox: queue.Queue[str] = queue.Queue(maxsize=64)
        self.thread = threading.Thread(target=self._worker, name="zw-usb-link", daemon=True)
        self.thread.start()

    def send(self, line: str) -> None:
        try:
            self.outbox.put_nowait(line)
        except queue.Full:
            # Interactive controls should prefer the latest command.
            try:
                self.outbox.get_nowait()
            except queue.Empty:
                pass
            try:
                self.outbox.put_nowait(line)
            except queue.Full:
                pass

    def close(self) -> None:
        self.stop_event.set()
        self.thread.join(timeout=0.8)

    def _pick_port(self) -> str | None:
        if self.requested_port and os.path.exists(self.requested_port):
            return self.requested_port
        # If the board re-enumerated with another ttyACM number, recover it.
        auto = find_port()
        return auto or self.requested_port

    def _worker(self) -> None:
        ser: serial.Serial | None = None
        rx = bytearray()
        last_ping = 0.0
        last_attempt = 0.0
        handshake_at = 0.0
        handshake_pending = False

        while not self.stop_event.is_set():
            if ser is None:
                now = time.monotonic()
                if now - last_attempt < 0.35:
                    time.sleep(0.03)
                    continue
                last_attempt = now
                port = self._pick_port()
                if not port or not os.path.exists(port):
                    self.messages.put("@HOST|WAITING")
                    time.sleep(0.25)
                    continue
                self.messages.put(f"@HOST|CONNECTING|{port}")
                try:
                    ser = open_no_reset(port)
                except (OSError, serial.SerialException) as exc:
                    self.messages.put(f"@HOST|DISCONNECTED|{port}|{exc}")
                    ser = None
                    time.sleep(0.25)
                    continue

                self.port = port
                rx.clear()
                self.messages.put(f"@HOST|CONNECTED|{port}")
                # Do not write immediately after open.  Native ESP32-S3 USB can
                # need a short moment after enumeration before the TX endpoint
                # accepts host data.  No DTR/RTS reset sequence is used.
                handshake_at = time.monotonic() + 0.35
                handshake_pending = True
                last_ping = time.monotonic()

            try:
                now = time.monotonic()
                if handshake_pending and now >= handshake_at:
                    self._write_line(ser, "@ZWIN|SYNC")
                    self._write_line(ser, "@ZWIN|PING")
                    handshake_pending = False
                    last_ping = time.monotonic()

                while True:
                    try:
                        line = self.outbox.get_nowait()
                    except queue.Empty:
                        break
                    self._write_line(ser, line)

                now = time.monotonic()
                if not handshake_pending and now - last_ping >= 1.0:
                    self._write_line(ser, "@ZWIN|PING")
                    last_ping = now

                chunk = ser.read(256)
                if chunk:
                    rx.extend(chunk)
                    while b"\n" in rx:
                        raw, _, rest = rx.partition(b"\n")
                        rx[:] = rest
                        line = raw.rstrip(b"\r").decode("utf-8", errors="replace")
                        if line:
                            self.messages.put(line)
                else:
                    time.sleep(0.005)
            except (OSError, serial.SerialException) as exc:
                # SerialTimeoutException is a SerialException too.  Any real
                # transport failure tears down only this connection; the worker
                # remains alive and auto-reconnects.
                port = self.port or "?"
                self.messages.put(f"@HOST|DISCONNECTED|{port}|{exc}")
                try:
                    ser.dtr = False
                    ser.rts = False
                except Exception:
                    pass
                try:
                    ser.close()
                except Exception:
                    pass
                ser = None
                self.port = None
                rx.clear()
                handshake_pending = False
                time.sleep(0.15)

        if ser is not None:
            try:
                ser.dtr = False
                ser.rts = False
            except Exception:
                pass
            try:
                ser.close()
            except Exception:
                pass

    @staticmethod
    def _write_line(ser: serial.Serial, line: str) -> None:
        ser.write((line + "\n").encode("utf-8"))


class WatchViewer:
    def __init__(self, root: tk.Tk, link: Link, scale: float):
        self.root = root
        self.link = link
        self.scale = scale
        self.messages = link.messages
        self.last_state: tuple[str, ...] = ("BOOT",)
        self.protocol_version = "?"
        self.last_pong = 0.0
        self.connected = False

        root.title("Zephyr Watch — Live ESP32-S3")
        root.minsize(620, 650)

        outer = ttk.Frame(root, padding=14)
        outer.pack(fill="both", expand=True)

        top = ttk.Frame(outer)
        top.pack(fill="x", pady=(0, 8))
        self.status = ttk.Label(top, text="ESP32-S3 • waiting for USB • no-reset async link")
        self.status.pack(side="left")
        ttk.Button(top, text="Sync", command=lambda: link.send("@ZWIN|SYNC")).pack(side="right")

        canvas_w = int(LOGICAL_W * scale)
        canvas_h = int(LOGICAL_H * scale)
        bezel = 26
        self.canvas = tk.Canvas(
            outer,
            width=canvas_w + bezel * 2,
            height=canvas_h + bezel * 2,
            bg="#202124",
            highlightthickness=0,
        )
        self.canvas.pack(pady=4)
        self.ox = bezel
        self.oy = bezel
        self.sw = canvas_w
        self.sh = canvas_h
        self.draw_screen_bg()

        controls = ttk.LabelFrame(outer, text="Real-board controls", padding=10)
        controls.pack(fill="x", pady=(12, 6))
        for col in range(6):
            controls.columnconfigure(col, weight=1)

        ttk.Button(controls, text="Normal", command=lambda: link.send("@ZWIN|NORMAL")).grid(row=0, column=0, sticky="ew", padx=3)
        ttk.Button(controls, text="Recovery", command=lambda: link.send("@ZWIN|RECOVERY")).grid(row=0, column=1, sticky="ew", padx=3)
        ttk.Button(controls, text="Fastboot", command=lambda: link.send("@ZWIN|FASTBOOT")).grid(row=0, column=2, sticky="ew", padx=3)
        ttk.Button(controls, text="◀ Crown", command=lambda: link.send("@ZWIN|ROTATE|-1")).grid(row=0, column=3, sticky="ew", padx=3)
        ttk.Button(controls, text="Crown ●", command=lambda: link.send("@ZWIN|POWER")).grid(row=0, column=4, sticky="ew", padx=3)
        ttk.Button(controls, text="Crown ▶", command=lambda: link.send("@ZWIN|ROTATE|1")).grid(row=0, column=5, sticky="ew", padx=3)

        fb = ttk.LabelFrame(outer, text="Fastboot+", padding=10)
        fb.pack(fill="x", pady=6)
        ttk.Button(fb, text="help", command=lambda: link.send("help")).pack(side="left", padx=3)
        ttk.Button(fb, text="getvar:all", command=lambda: link.send("getvar:all")).pack(side="left", padx=3)
        ttk.Button(fb, text="continue", command=lambda: link.send("continue")).pack(side="left", padx=3)
        ttk.Button(fb, text="reboot", command=lambda: link.send("reboot")).pack(side="left", padx=3)

        self.log = tk.Text(outer, height=8, wrap="word", state="disabled")
        self.log.pack(fill="both", expand=True, pady=(6, 0))

        self.root.protocol("WM_DELETE_WINDOW", self.close)
        self.root.after(20, self.pump)
        self.root.after(500, self.health_tick)

    def p(self, x: float, y: float) -> tuple[float, float]:
        return self.ox + x * self.scale, self.oy + y * self.scale

    def draw_screen_bg(self):
        self.canvas.delete("all")
        x0, y0 = self.p(0, 0)
        x1, y1 = self.p(LOGICAL_W, LOGICAL_H)
        r = 34 * self.scale
        self.round_rect(x0, y0, x1, y1, r, fill="#000000", outline="#4a4d52", width=2)

    def round_rect(self, x1, y1, x2, y2, r, **kwargs):
        points = [
            x1+r,y1, x2-r,y1, x2,y1, x2,y1+r,
            x2,y2-r, x2,y2, x2-r,y2, x1+r,y2,
            x1,y2, x1,y2-r, x1,y1+r, x1,y1,
        ]
        return self.canvas.create_polygon(points, smooth=True, **kwargs)

    def text_center(self, y: int, text: str, size: int, color="#f1f3f4", weight="normal"):
        x, py = self.p(LOGICAL_W / 2, y)
        self.canvas.create_text(x, py, text=text, fill=color, font=("Sans", max(8, int(size * self.scale)), weight), anchor="n")

    def text_left(self, x: int, y: int, text: str, size: int, color="#f1f3f4"):
        px, py = self.p(x, y)
        self.canvas.create_text(px, py, text=text, fill=color, font=("Sans", max(8, int(size * self.scale))), anchor="nw")

    def mark(self):
        colors = ("#4285f4", "#ea4335", "#34a853", "#fbbc05")
        rects = ((91,62,120,72),(120,62,149,72),(91,72,120,82),(120,72,149,82))
        for c, (x0,y0,x1,y1) in zip(colors, rects):
            a = self.p(x0,y0); b = self.p(x1,y1)
            self.canvas.create_rectangle(*a, *b, fill=c, outline=c)
        self.text_center(88, "Z", 38, "white", "bold")

    def spinner(self, phase: int):
        pts = [(120,214),(136,208),(142,192),(136,176),(120,170),(104,176),(98,192),(104,208)]
        for i, (x,y) in enumerate(pts):
            px, py = self.p(x,y)
            rr = 4 * self.scale
            color = "#f1f3f4" if i == phase % 8 else "#5f6368"
            self.canvas.create_oval(px-rr, py-rr, px+rr, py+rr, fill=color, outline="")

    def render(self, state: tuple[str, ...]):
        self.last_state = state
        self.draw_screen_bg()
        kind = state[0]
        if kind == "BOOT":
            self.mark()
            self.text_center(155, "ANDROID", 18, "#f1f3f4", "bold")
        elif kind == "PROGRESS":
            label = state[1] if len(state) > 1 else "BOOTING"
            phase = int(state[2]) if len(state) > 2 and state[2].isdigit() else 0
            self.mark()
            self.text_center(145, label, 14, "#f1f3f4", "bold")
            self.spinner(phase)
        elif kind == "STATUS":
            title, l1, l2, l3 = (list(state[1:5]) + ["", "", "", ""])[:4]
            self.text_center(35, title, 18, "#f1f3f4", "bold")
            a = self.p(25,72); b = self.p(215,74)
            self.canvas.create_rectangle(*a,*b,fill="#5f6368",outline="")
            self.text_left(24,104,l1,13)
            self.text_left(24,142,l2,13)
            self.text_left(24,180,l3,13)
        elif kind == "ERROR":
            code = state[1] if len(state) > 1 else "ERROR"
            detail = state[2] if len(state) > 2 else ""
            self.text_center(55, "BOOT FAILED", 18, "#ea4335", "bold")
            self.text_center(125, code, 14, "#f1f3f4", "bold")
            self.text_center(165, detail, 12, "#9aa0a6")

    def append_log(self, text: str):
        self.log.configure(state="normal")
        self.log.insert("end", text + "\n")
        self.log.see("end")
        self.log.configure(state="disabled")

    def handle_line(self, line: str):
        if line.startswith("@HOST|"):
            parts = line.split("|", 3)
            event = parts[1] if len(parts) > 1 else ""
            if event == "CONNECTED":
                self.connected = True
                port = parts[2] if len(parts) > 2 else "?"
                self.status.configure(text=f"ESP32-S3 • {port} • connected • no-reset async link")
            elif event == "CONNECTING":
                port = parts[2] if len(parts) > 2 else "?"
                self.status.configure(text=f"ESP32-S3 • connecting {port}…")
            elif event == "DISCONNECTED":
                self.connected = False
                self.status.configure(text="ESP32-S3 • disconnected • auto-reconnect enabled")
                if len(parts) > 3:
                    self.append_log(f"USB: {parts[3]}")
            elif event == "WAITING":
                self.connected = False
                self.status.configure(text="ESP32-S3 • waiting for USB • auto-reconnect enabled")
            return

        if line.startswith("@ZWUI|"):
            parts = tuple(line.split("|"))
            if len(parts) >= 2 and parts[1] == "HELLO":
                self.protocol_version = parts[2] if len(parts) > 2 else "?"
                port = self.link.port or "?"
                self.status.configure(text=f"LIVE ESP32-S3 • {port} • protocol v{self.protocol_version} • no-reset")
            elif len(parts) >= 2 and parts[1] == "PONG":
                self.last_pong = time.monotonic()
            elif len(parts) >= 2:
                self.render(parts[1:])
            return

        if line:
            self.append_log(line)

    def pump(self):
        try:
            while True:
                self.handle_line(self.messages.get_nowait())
        except queue.Empty:
            pass
        self.root.after(20, self.pump)

    def health_tick(self):
        if self.connected and self.last_pong and time.monotonic() - self.last_pong > 3.0:
            port = self.link.port or "?"
            self.status.configure(text=f"ESP32-S3 • {port} • USB open, waiting for board")
        self.root.after(500, self.health_tick)

    def close(self):
        self.link.close()
        self.root.destroy()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", help="preferred CDC port, e.g. /dev/ttyACM0; auto-reconnect follows re-enumeration")
    ap.add_argument("--scale", type=float, default=1.6)
    args = ap.parse_args()

    messages: queue.Queue[str] = queue.Queue()
    link = Link(args.port, messages)
    root = tk.Tk()
    WatchViewer(root, link, max(1.0, args.scale))
    root.mainloop()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
