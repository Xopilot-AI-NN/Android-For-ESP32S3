#!/usr/bin/env python3
"""AOSP Wear OS USB Desktop Display v3.1.

When usb_block_server.py is active, this viewer NEVER opens /dev/ttyACM*.
It attaches to the block server through a local Unix socket, so opening or
closing the GUI cannot toggle DTR/RTS, steal protocol bytes, or reset the board.
Direct serial mode remains available for normal SD-card boot.
"""

from __future__ import annotations

import argparse
import os
import queue
import socket
import threading
import time
import tkinter as tk
from pathlib import Path
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



def default_viewer_socket() -> Path:
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if runtime and os.path.isdir(runtime):
        return Path(runtime) / "aosp-wear-viewer.sock"
    uid = os.getuid() if hasattr(os, "getuid") else 0
    return Path(f"/tmp/aosp-wear-viewer-{uid}.sock")


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
    if termios is None or os.name != "posix":
        return
    try:
        attrs = termios.tcgetattr(ser.fileno())
        attrs[2] &= ~termios.HUPCL
        termios.tcsetattr(ser.fileno(), termios.TCSANOW, attrs)
    except (OSError, termios.error):
        pass


def open_no_reset(port: str) -> serial.Serial:
    ser = serial.Serial()
    ser.baudrate = 115200
    ser.timeout = 0.05
    ser.write_timeout = None
    ser.xonxoff = False
    ser.rtscts = False
    ser.dsrdtr = False
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


class BridgeLink:
    """Viewer link that never touches the ESP serial device."""

    def __init__(self, path: Path, messages: queue.Queue[str]):
        self.path = path
        self.messages = messages
        self.port: str | None = f"bridge:{path}"
        self.stop_event = threading.Event()
        self.outbox: queue.Queue[str] = queue.Queue(maxsize=64)
        self.thread = threading.Thread(target=self._worker, name="zw-viewer-bridge", daemon=True)
        self.thread.start()

    def send(self, line: str) -> None:
        try:
            self.outbox.put_nowait(line)
        except queue.Full:
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

    def _worker(self) -> None:
        sock: socket.socket | None = None
        rx = bytearray()
        last_attempt = 0.0
        handshake_pending = False
        handshake_at = 0.0
        last_ping = 0.0

        while not self.stop_event.is_set():
            if sock is None:
                now = time.monotonic()
                if now - last_attempt < 0.25:
                    time.sleep(0.03)
                    continue
                last_attempt = now
                if not self.path.exists():
                    self.messages.put("@HOST|WAITING")
                    time.sleep(0.2)
                    continue
                self.messages.put(f"@HOST|CONNECTING|bridge:{self.path}")
                candidate = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                candidate.settimeout(0.3)
                try:
                    candidate.connect(str(self.path))
                except OSError as exc:
                    candidate.close()
                    self.messages.put(f"@HOST|DISCONNECTED|bridge:{self.path}|{exc}")
                    time.sleep(0.2)
                    continue
                candidate.settimeout(0.05)
                sock = candidate
                rx.clear()
                self.messages.put(f"@HOST|CONNECTED|bridge:{self.path}")
                handshake_pending = True
                handshake_at = time.monotonic() + 0.05
                last_ping = time.monotonic()

            try:
                now = time.monotonic()
                if handshake_pending and now >= handshake_at:
                    sock.sendall(b"@ZWIN|SYNC\n@ZWIN|PING\n")
                    handshake_pending = False
                    last_ping = now

                while True:
                    try:
                        line = self.outbox.get_nowait()
                    except queue.Empty:
                        break
                    sock.sendall((line + "\n").encode("utf-8"))

                now = time.monotonic()
                if not handshake_pending and now - last_ping >= 5.0:
                    sock.sendall(b"@ZWIN|PING\n")
                    last_ping = now

                try:
                    chunk = sock.recv(4096)
                except socket.timeout:
                    chunk = None
                if chunk == b"":
                    raise ConnectionResetError("viewer bridge closed")
                if chunk:
                    rx.extend(chunk)
                    while b"\n" in rx:
                        raw, _, rest = rx.partition(b"\n")
                        rx[:] = rest
                        line = raw.rstrip(b"\r").decode("utf-8", errors="replace")
                        if line:
                            self.messages.put(line)
            except (OSError, ConnectionError) as exc:
                self.messages.put(f"@HOST|DISCONNECTED|bridge:{self.path}|{exc}")
                try:
                    sock.close()
                except Exception:
                    pass
                sock = None
                handshake_pending = False
                rx.clear()
                time.sleep(0.15)

        if sock is not None:
            try:
                sock.close()
            except OSError:
                pass


class SerialLink:
    """Direct no-reset serial link used when no PC-block bridge is active."""

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
                if not handshake_pending and now - last_ping >= 5.0:
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
    def __init__(self, root: tk.Tk, link, scale: float):
        self.root = root
        self.link = link
        self.scale = scale
        self.messages = link.messages
        self.last_state: tuple[str, ...] = ("BOOT",)
        self.protocol_version = "?"
        self.last_pong = 0.0
        self.connected = False

        root.title("AOSP Wear OS 17.2.1 — Android 17 QPR1 — Live ESP32-S3")
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

        ttk.Button(controls, text="Home", command=lambda: link.send("@ZWIN|NORMAL")).grid(row=0, column=0, sticky="ew", padx=3)
        ttk.Button(controls, text="Recovery", command=lambda: link.send("@ZWIN|RECOVERY")).grid(row=0, column=1, sticky="ew", padx=3)
        ttk.Button(controls, text="Fastboot", command=lambda: link.send("@ZWIN|FASTBOOT")).grid(row=0, column=2, sticky="ew", padx=3)
        ttk.Button(controls, text="◀ Crown", command=lambda: link.send("@ZWIN|ROTATE|-1")).grid(row=0, column=3, sticky="ew", padx=3)
        ttk.Button(controls, text="Crown ●", command=lambda: link.send("@ZWIN|POWER")).grid(row=0, column=4, sticky="ew", padx=3)
        ttk.Button(controls, text="Crown ▶", command=lambda: link.send("@ZWIN|ROTATE|1")).grid(row=0, column=5, sticky="ew", padx=3)

        ttk.Button(controls, text="Swipe ↑", command=lambda: link.send("@ZWIN|SWIPE|UP")).grid(row=1, column=1, sticky="ew", padx=3, pady=(6, 0))
        ttk.Button(controls, text="Swipe ↓", command=lambda: link.send("@ZWIN|SWIPE|DOWN")).grid(row=1, column=2, sticky="ew", padx=3, pady=(6, 0))
        ttk.Button(controls, text="Swipe ←", command=lambda: link.send("@ZWIN|SWIPE|LEFT")).grid(row=1, column=3, sticky="ew", padx=3, pady=(6, 0))
        ttk.Button(controls, text="Swipe →", command=lambda: link.send("@ZWIN|SWIPE|RIGHT")).grid(row=1, column=4, sticky="ew", padx=3, pady=(6, 0))

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
        if kind == "M3":
            self.render_m3(state)
            return
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


    def m3_palette(self, idx: int):
        palettes = [
            dict(bg="#0a120d", surface="#141e18", high="#1e2b23", primary="#7edfa4", on_primary="#00391d", pc="#145130", on_pc="#9efbc1", secondary="#34463b", on="#e0e9e1", muted="#becac1", outline="#89958c", error="#ffb4ab"),
            dict(bg="#0c1018", surface="#161b26", high="#202735", primary="#a7c7ff", on_primary="#002e5c", pc="#124474", on_pc="#d5e3ff", secondary="#374254", on="#e4e7ee", muted="#c2c7d1", outline="#8b919c", error="#ffb4ab"),
            dict(bg="#140e18", surface="#1f1823", high="#2c2231", primary="#e0b8ff", on_primary="#461865", pc="#60307f", on_pc="#f3daff", secondary="#4d3952", on="#ede3ee", muted="#d1c2d3", outline="#998b9b", error="#ffb4ab"),
            dict(bg="#180f0d", surface="#251916", high="#33231f", primary="#ffb59d", on_primary="#5c1d0b", pc="#7c341f", on_pc="#ffdbd0", secondary="#523a32", on="#f5e2dc", muted="#d8c2bb", outline="#a08c86", error="#ffb4ab"),
        ]
        return palettes[idx % len(palettes)]

    def m3_rr(self, x, y, w, h, r, fill, outline=""):
        a = self.p(x, y); b = self.p(x + w, y + h)
        return self.round_rect(a[0], a[1], b[0], b[1], r * self.scale, fill=fill, outline=outline)

    def m3_circle(self, cx, cy, r, fill):
        x, y = self.p(cx, cy); rr = r * self.scale
        self.canvas.create_oval(x-rr, y-rr, x+rr, y+rr, fill=fill, outline="")

    def render_m3(self, state: tuple[str, ...]):
        vals = list(state[1:]) + ["0"] * 30
        try:
            (screen, cursor, brightness, dnd, airplane, theme, locked, wifi, bt,
             adb, notes, mins, swap_pages, timer_secs, stopwatch_secs, alarm_enabled,
             auto_time, time_valid, timezone_hours, wifi_ap_count, wifi_ap_index,
             wifi_ap_rssi, wifi_password_len, wifi_editor_char, wifi_link_state) = [int(v) for v in vals[:25]]
            try:
                wifi_ap_ssid = bytes.fromhex(vals[25]).decode("utf-8", errors="replace") if vals[25] else ""
            except ValueError:
                wifi_ap_ssid = ""
        except ValueError:
            return
        t = self.m3_palette(theme)
        # Wear surfaces build from true black; color appears in containers.
        t["bg"] = "#000000"
        self.canvas.delete("all")
        self.m3_rr(0, 0, 240, 280, 34, t["bg"], outline="#35363a")

        def text(x, y, value, size=12, color=None, bold=False, center=False):
            px, py = self.p(x, y)
            self.canvas.create_text(
                px, py, text=value, fill=color or t["on"],
                font=("Inter Display", max(8, int(size*self.scale)), "bold" if bold else "normal"),
                anchor="n" if center else "nw"
            )

        def title(value):
            text(120, 12, value, 11, t["on"], True, True)

        def line(x0,y0,x1,y1,color,width=2):
            a=self.p(x0,y0); b=self.p(x1,y1)
            self.canvas.create_line(a[0],a[1],b[0],b[1],fill=color,width=max(1,int(width*self.scale)),capstyle="round")

        def icon_wifi(cx,cy,color):
            line(cx-12,cy-6,cx,cy-12,color); line(cx,cy-12,cx+12,cy-6,color)
            line(cx-8,cy,cx,cy-4,color); line(cx,cy-4,cx+8,cy,color)
            line(cx-4,cy+6,cx,cy+4,color); line(cx,cy+4,cx+4,cy+6,color)
            self.m3_circle(cx,cy+10,2,color)

        def icon_bt(cx,cy,color):
            line(cx,cy-13,cx,cy+13,color)
            line(cx,cy-13,cx+8,cy-5,color); line(cx+8,cy-5,cx-7,cy+7,color)
            line(cx-7,cy-7,cx+8,cy+5,color); line(cx+8,cy+5,cx,cy+13,color)

        def icon_dnd(cx,cy,color):
            x,y=self.p(cx,cy); r=12*self.scale
            self.canvas.create_oval(x-r,y-r,x+r,y+r,outline=color,width=max(1,int(2*self.scale)))
            line(cx-7,cy,cx+7,cy,color,3)

        def icon_air(cx,cy,color):
            line(cx-13,cy,cx+13,cy,color,3); line(cx+3,cy,cx-3,cy-9,color,3); line(cx+3,cy,cx-3,cy+9,color,3)

        def icon_sun(cx,cy,color):
            x,y=self.p(cx,cy); r=7*self.scale
            self.canvas.create_oval(x-r,y-r,x+r,y+r,outline=color,width=max(1,int(2*self.scale)))
            for a,b,c,d in ((0,-14,0,-10),(0,10,0,14),(-14,0,-10,0),(10,0,14,0)):
                line(cx+a,cy+b,cx+c,cy+d,color,2)

        def icon_clock(cx,cy,color):
            x,y=self.p(cx,cy); r=12*self.scale
            self.canvas.create_oval(x-r,y-r,x+r,y+r,outline=color,width=max(1,int(2*self.scale)))
            line(cx,cy,cx,cy-7,color); line(cx,cy,cx+6,cy+3,color)

        def icon_bell(cx,cy,color):
            self.m3_circle(cx,cy,10,color); self.m3_circle(cx,cy,7,t["surface"])
            self.m3_rr(cx-10,cy,20,9,4,color); self.m3_circle(cx,cy+12,2,color)

        def icon_settings(cx,cy,color):
            x,y=self.p(cx,cy); r=9*self.scale
            self.canvas.create_oval(x-r,y-r,x+r,y+r,outline=color,width=max(1,int(3*self.scale)))
            self.m3_circle(cx,cy,3,color)
            for a,b,c,d in ((0,-15,0,-10),(0,10,0,15),(-15,0,-10,0),(10,0,15,0)):
                line(cx+a,cy+b,cx+c,cy+d,color,3)

        def icon_media(cx,cy,color):
            # Compact Material play/media glyph.
            for i in range(13):
                half=i if i<7 else 12-i
                line(cx-6+i,cy-half,cx-6+i,cy+half,color,2)
            line(cx-11,cy+13,cx+11,cy+13,color,2)

        def icon_info(cx,cy,color):
            x,y=self.p(cx,cy); r=12*self.scale
            self.canvas.create_oval(x-r,y-r,x+r,y+r,outline=color,width=max(1,int(2*self.scale)))
            self.m3_circle(cx,cy-6,2,color); self.m3_rr(cx-2,cy-1,4,10,2,color)

        def icon_grid(cx,cy,color):
            for yy in (-7,0,7):
                for xx in (-7,0,7): self.m3_circle(cx+xx,cy+yy,2,color)

        def icon_assistant(cx,cy,color):
            line(cx,cy-13,cx,cy+13,color,2); line(cx-13,cy,cx+13,cy,color,2)
            line(cx-8,cy-8,cx+8,cy+8,color,2); line(cx-8,cy+8,cx+8,cy-8,color,2)
            self.m3_circle(cx,cy,3,color)

        def app_icon(index,cx,cy,color):
            if index==0: icon_clock(cx,cy,color)
            elif index==1: icon_media(cx,cy,color)
            elif index==2: icon_settings(cx,cy,color)
            elif index==3: icon_wifi(cx,cy,color)
            elif index==4: icon_info(cx,cy,color)
            elif index==5: icon_assistant(cx,cy,color)
            else: icon_grid(cx,cy,color)

        app_accents=("#b5e8d3","#ffbfab","#aecbff","#66ddcd","#dabeff","#7ee1a5","#323b36")

        def icon_back(cx,cy,color):
            line(cx+8,cy,cx-8,cy,color,3); line(cx-8,cy,cx-1,cy-7,color,3); line(cx-8,cy,cx-1,cy+7,color,3)

        def icon_lock(cx,cy,color):
            self.m3_rr(cx-9,cy-1,18,15,4,color)
            x,y=self.p(cx,cy-5); r=7*self.scale
            self.canvas.create_oval(x-r,y-r,x+r,y+r,outline=color,width=max(1,int(2*self.scale)))

        def icon_sound(cx,cy,color):
            self.m3_rr(cx-11,cy-4,6,8,1,color)
            line(cx-5,cy-4,cx+2,cy-10,color,3); line(cx-5,cy+4,cx+2,cy+10,color,3)
            line(cx+6,cy-7,cx+10,cy,color,2); line(cx+10,cy,cx+6,cy+7,color,2)

        def icon_person(cx,cy,color):
            self.m3_circle(cx,cy-8,4,color); line(cx,cy-2,cx,cy+10,color,3)
            line(cx-9,cy+2,cx+9,cy+2,color,3); line(cx,cy+10,cx-7,cy+15,color,3); line(cx,cy+10,cx+7,cy+15,color,3)

        def row_icon(label,cx,cy,color):
            if label in ("CONNECTIVITY","WI-FI"): icon_wifi(cx,cy,color)
            elif label=="BLUETOOTH": icon_bt(cx,cy,color)
            elif label in ("APPS & NOTIFS","NOTIFICATIONS"): icon_bell(cx,cy,color)
            elif label in ("DISPLAY","BRIGHTNESS"): icon_sun(cx,cy,color)
            elif label=="SOUND & VIBRATION": icon_sound(cx,cy,color)
            elif label=="ACCESSIBILITY": icon_person(cx,cy,color)
            elif label in ("SECURITY","SCREEN LOCK"): icon_lock(cx,cy,color)
            elif label in ("DATE & TIME","AUTOMATIC TIME","SET HOUR","SET MINUTE","TIME ZONE","CLOCK"): icon_clock(cx,cy,color)
            elif label in ("ABOUT","SYSTEM INFO","BUILD NUMBER"): icon_info(cx,cy,color)
            elif label in ("DEVELOPER OPTIONS","SYSTEM","SETTINGS"): icon_settings(cx,cy,color)
            elif label=="MEDIA CONTROLS": icon_media(cx,cy,color)
            elif label=="AIRPLANE MODE": icon_air(cx,cy,color)
            elif label=="BACK": icon_back(cx,cy,color)
            else: self.m3_circle(cx,cy,3,color)

        def row(y, label, selected):
            x,w,h,r = (5,230,50,25) if selected else (17,206,42,21)
            bg=t["pc"] if selected else t["surface"]
            fg=t["on_pc"] if selected else t["on"]
            self.m3_rr(x,y,w,h,r,bg)
            self.m3_circle(x+25,y+h/2,12 if selected else 9,t["primary"] if selected else t["high"])
            row_icon(label,x+25,y+h/2,t["on_primary"] if selected else t["muted"])
            size = 7 if len(label) > 14 else (8 if len(label)>10 else 10)
            text(x+46,y+(15 if size<=8 else 11),label,size,fg,selected)

        def toggle(x,y,on):
            bg=t["primary"] if on else t["high"]
            self.m3_rr(x,y,42,24,12,bg)
            self.m3_circle(x+(30 if on else 12),y+12,8,t["on_primary"] if on else t["muted"])

        def qbutton(cx,cy,label,on,selected):
            w,h=(92,54) if selected else (82,48)
            bg=t["primary"] if on else (t["pc"] if selected else t["high"])
            fg=t["on_primary"] if on else (t["on_pc"] if selected else t["on"])
            self.m3_rr(cx-w/2,cy-h/2,w,h,h/2,bg)
            ix=cx-22
            {"WIFI":icon_wifi,"BT":icon_bt,"DND":icon_dnd,"AIR":icon_air,"LIGHT":icon_sun,"SET":icon_settings}.get(label,lambda x,y,c:self.m3_circle(x,y,7,c))(ix,cy,fg)
            text(cx-2,cy-5,label,7,fg,True)

        def info(y,a,b,active=False):
            self.m3_rr(16,y,208,52,26,t["pc"] if active else t["surface"])
            self.m3_circle(42,y+26,10,t["primary"] if active else t["high"])
            text(64,y+8,a,7,t["muted"]); text(64,y+27,b,8,t["primary"] if active else t["on"],True)

        hour=(mins//60)%24; minute=mins%60
        clock=f"{hour:02d}:{minute:02d}"

        if screen == 0:
            text(120,66,clock,37,t["on"],True,True)
            self.m3_circle(120,145,17,t["high"]); self.m3_rr(114,142,12,12,3,t["muted"])
            if notes: self.m3_circle(120,196,5,t["primary"])
            text(120,226,"UNLOCK",7,t["muted"],False,True)

        elif screen == 1:
            sx=18
            if airplane: icon_air(sx,15,t["muted"]); sx+=29
            if wifi: icon_wifi(sx,15,t["primary"]); sx+=31
            if bt: icon_bt(sx,15,t["primary"]); sx+=25
            if dnd: icon_dnd(sx,15,t["muted"])
            if notes: self.m3_circle(218,15,4,t["primary"])
            text(120,55,clock,38,t["on"],True,True)
            if not time_valid:
                text(120,112,"SYNCING TIME" if auto_time else "SET TIME IN SETTINGS",7,t["muted"],False,True)
            for cx in (54,120,186): self.m3_circle(cx,177,24,t["surface"])
            icon_wifi(54,177,t["primary"] if wifi else t["outline"])
            icon_clock(120,177,t["primary"] if alarm_enabled else t["outline"])
            icon_dnd(186,177,t["primary"] if dnd else t["outline"])
            if notes:
                self.m3_circle(120,231,6,t["primary"]); self.m3_circle(120,231,2,t["on_primary"])

        elif screen == 2:
            labels=("CLOCK","MEDIA","SETTINGS","CONNECTIVITY","SYSTEM INFO","ASSISTANT","LIST VIEW")
            cur=min(cursor,6)
            pos=((52,70),(120,58),(188,70),(52,140),(120,128),(188,140),(120,208))
            for i,(cx,cy) in enumerate(pos):
                sel=(i==cur); rr=33 if sel else (26 if i==6 else 28)
                bg=t["high"] if i==6 else app_accents[i]
                fg=t["muted"] if i==6 else "#0a2218"
                if sel:
                    x,y=self.p(cx,cy); r=(rr+3)*self.scale
                    self.canvas.create_oval(x-r,y-r,x+r,y+r,outline=t["on"],width=max(1,int(2*self.scale)))
                self.m3_circle(cx,cy,rr,bg); app_icon(i,cx,cy,fg)
            self.m3_rr(31,244,178,27,13,t["surface"]); text(120,252,labels[cur],8,t["on"],True,True)

        elif screen == 3:
            text(120,10,clock,8,t["muted"],False,True)
            if notes:
                self.m3_rr(12,50,216,112,35,t["pc"] if cursor==0 else t["surface"])
                self.m3_circle(44,82,14,t["primary"]); icon_bell(44,82,t["on_primary"])
                text(67,67,"SYSTEM",7,t["muted"]); text(67,88,"NOTIFICATION",10,t["on"],True)
                text(67,119,"PRESS TO DISMISS",7,t["muted"])
            else:
                self.m3_circle(120,113,32,t["surface"]); icon_bell(120,113,t["muted"])
                text(120,161,"NO NOTIFICATIONS",8,t["muted"],False,True)

        elif screen == 4:
            text(120,8,clock,12,t["on"],True,True)
            qbutton(66,68,"WIFI",wifi,cursor==0); qbutton(174,68,"BT",bt,cursor==1)
            qbutton(66,132,"DND",dnd,cursor==2); qbutton(174,132,"AIR",airplane,cursor==3)
            qbutton(66,196,"LIGHT",True,cursor==4); qbutton(174,196,"SET",False,cursor==5)
            icon_sun(25,250,t["muted"]); self.m3_rr(43,246,165,10,5,t["high"]); self.m3_rr(43,246,max(10,int(165*brightness/100)),10,5,t["primary"])

        elif screen == 5:
            title("SETTINGS")
            labels=("CONNECTIVITY","APPS & NOTIFS","DISPLAY","SOUND & VIBRATION","GESTURES","ACCESSIBILITY","SECURITY","SYSTEM","BACK")
            cur=min(cursor,8); start=min(max(cur-1,0),6)
            for r in range(3):
                i=min(8,start+r); row(55+r*64,labels[i],i==cur)

        elif screen == 6:
            text(120,42,clock,37,t["on"],True,True); text(120,98,"CLOCK",8,t["muted"],False,True)
            for i,name in enumerate(("TIMER","STOPWATCH","ALARM")):
                cx=48+i*72; sel=cursor==i; self.m3_circle(cx,165,28 if sel else 23,t["primary"] if sel else t["high"]); icon_clock(cx,165,t["on_primary"] if sel else t["on"]); text(cx,202,name,7,t["muted"],False,True)

        elif screen == 7:
            title("CONNECTIVITY")
            labels=("WI-FI","BLUETOOTH","AIRPLANE MODE","BACK"); vals=(wifi,bt,airplane,False)
            cur=min(cursor,3); start=min(max(cur-1,0),1)
            for r in range(3):
                i=min(3,start+r); y=57+r*64; row(y,labels[i],i==cur)
                if i<3: toggle(174,y+13,vals[i])

        elif screen == 8:
            title("ABOUT"); self.m3_circle(120,72,30,t["pc"]); text(120,54,"A",27,t["primary"],True,True)
            text(120,116,"AOSP WEAR",12,t["on"],True,True); text(120,144,"ANDROID 17",8,t["primary"],True,True)
            self.m3_rr(22,168,196,36,18,t["surface"]); text(120,179,"BUILD 17.2.1",8,t["on"],False,True)
            text(120,216,"ESP32-S3-ZERO-N4R2",7,t["muted"],False,True)
            text(120,238,"SYSTEM STORAGE ACTIVE" if swap_pages else "SYSTEM STORAGE READY",7,t["outline"],False,True)

        elif screen == 9:
            cur=cursor%3
            if cur==0:
                bg,fg,sub="#3a715e","#e4fff1",("WI-FI CONNECTED" if wifi else "WI-FI OFF")
            elif cur==1:
                bg,fg,sub="#2f4e7e","#e2ecff",("TIMER RUNNING" if timer_secs else "5 MINUTE TIMER")
            else:
                bg,fg,sub="#5c4377","#f7e7ff",("ALARM ON" if alarm_enabled else "ALARM OFF")
            self.m3_rr(10,24,220,208,42,bg)
            if cur==0: icon_wifi(52,66,fg); text(82,55,"CONNECTIVITY",8,fg,True); text(27,112,sub,11,fg,True); text(27,153,"PRESS TO OPEN",7,fg)
            elif cur==1:
                icon_clock(52,66,fg); text(82,55,"CLOCK",8,fg,True); text(27,112,sub,11,fg,True); tt=timer_secs if timer_secs else 300; text(27,153,f"{tt//60:02d}:{tt%60:02d}",11,fg,True)
            else: icon_clock(52,66,fg); text(82,55,"ALARM",8,fg,True); text(27,112,sub,11,fg,True); text(27,153,"PRESS TO OPEN",7,fg)
            for i in range(3): self.m3_circle(108+i*12,254,4 if i==cur else 2,t["primary"] if i==cur else t["outline"])

        elif screen == 10:
            text(120,9,clock,8,t["muted"],False,True); self.m3_circle(120,78,38,t["high"]); text(120,58,"M",27,t["primary"],True,True)
            text(120,128,"NO MEDIA PLAYING",9,t["on"],True,True); text(120,149,"START MEDIA ON PHONE",7,t["muted"],False,True)
            for i,label in enumerate(("<","||",">")):
                cx=55+i*65; sel=cursor==i; self.m3_circle(cx,204,26 if sel else 21,t["primary"] if sel else t["high"]); text(cx,194,label,12,t["on_primary"] if sel else t["on"],True,True)

        elif screen == 11:
            title("CLOCK")
            timer_text=f"{(timer_secs if timer_secs else 300)//60:02d}:{(timer_secs if timer_secs else 300)%60:02d}"; sw_text=f"{stopwatch_secs//60:02d}:{stopwatch_secs%60:02d}"
            labels=("TIMER","STOPWATCH","ALARM","BACK"); cur=min(cursor,3); start=min(max(cur-1,0),1)
            for r in range(3):
                i=min(3,start+r); y=57+r*64; row(y,labels[i],i==cur)
                if i==0: text(154,y+18,timer_text,7,t["primary"] if timer_secs else t["muted"])
                elif i==1: text(154,y+18,sw_text,7,t["muted"])
                elif i==2: text(174,y+18,"ON" if alarm_enabled else "OFF",7,t["primary"] if alarm_enabled else t["muted"])

        elif screen == 12:
            title("WI-FI"); self.m3_rr(18,52,204,58,29,t["pc"] if cursor==0 else t["surface"]); icon_wifi(47,81,t["primary"] if wifi else t["outline"])
            text(72,66,"USE WI-FI",8,t["on"]); text(72,84,"ON" if wifi else "OFF",7,t["muted"]); toggle(171,69,wifi)
            info(124,"NETWORKS","SCAN / SAVED" if wifi else "WI-FI IS OFF",cursor==1); info(188,"BACK","CONNECTIVITY",cursor==2)

        elif screen == 13:
            title("BLUETOOTH"); self.m3_rr(18,52,204,58,29,t["pc"] if cursor==0 else t["surface"]); icon_bt(47,81,t["primary"] if bt else t["outline"])
            text(72,66,"BLUETOOTH",8,t["on"]); text(72,84,"ON" if bt else "OFF",7,t["muted"]); toggle(171,69,bt)
            info(124,"PAIR NEW DEVICE","READY TO SCAN" if bt else "BLUETOOTH OFF",cursor==1); info(188,"WATCH NAME","AOSP WEAR",cursor==2)

        elif screen == 14:
            title("DISPLAY"); self.m3_rr(16,55,208,66,33,t["pc"] if cursor==0 else t["surface"]); icon_sun(42,86,t["primary"]); text(66,68,"BRIGHTNESS",7,t["muted"]); self.m3_rr(66,94,132,10,5,t["high"]); self.m3_rr(66,94,max(10,int(132*brightness/100)),10,5,t["primary"])
            self.m3_rr(16,135,208,58,29,t["pc"] if cursor==1 else t["surface"]); text(38,148,"DYNAMIC COLOR",7,t["muted"]); text(38,168,("GREEN","BLUE","PURPLE","CORAL")[theme%4],10,t["primary"],True)
            self.m3_rr(16,207,208,42,21,t["pc"] if cursor==2 else t["surface"]); text(120,219,"BACK",8,t["on"],True,True)

        elif screen == 15:
            title("SYSTEM"); labels=("DATE & TIME","ABOUT","DEVELOPER OPTIONS","BACK"); cur=min(cursor,3); start=min(max(cur-1,0),1)
            for r in range(3): i=min(3,start+r); row(55+r*64,labels[i],i==cur)

        elif screen == 16:
            title("DATE & TIME"); labels=("AUTOMATIC TIME","SET HOUR","SET MINUTE","TIME ZONE","BACK"); cur=min(cursor,4); start=min(max(cur-1,0),2)
            for r in range(3):
                i=min(4,start+r); y=56+r*64; row(y,labels[i],i==cur)
                if i==0: toggle(174,y+13,bool(auto_time))
                elif i in (1,2): text(167,y+18,clock,7,t["outline"] if auto_time else t["primary"])
                elif i==3: text(166,y+18,f"UTC{timezone_hours:+d}" if timezone_hours else "UTC",7,t["muted"])
            if auto_time and not time_valid: text(120,251,"SYNCING OVER WI-FI / PHONE",7,t["primary"],False,True)

        elif screen == 17:
            title("SOUND & VIBRATION"); self.m3_circle(120,103,37,t["surface"]); text(120,87,"S",20,t["primary"],True,True); text(120,157,"AUDIO HARDWARE",8,t["on"],True,True); text(120,180,"NOT PRESENT ON THIS BOARD",7,t["muted"],False,True); self.m3_rr(45,217,150,38,19,t["high"]); text(120,229,"BACK",8,t["on"],True,True)

        elif screen == 18:
            title("GESTURES"); info(65,"CROWN","ROTATE TO SCROLL",True); info(132,"CROWN PRESS","SELECT / APPS"); info(199,"HOME","RETURN TO WATCH FACE")

        elif screen == 19:
            title("ACCESSIBILITY"); info(68,"INPUT","CROWN-ONLY MODE",True); info(135,"CONTRAST","DARK WATCH UI"); self.m3_rr(45,213,150,40,20,t["high"]); text(120,226,"BACK",8,t["on"],True,True)

        elif screen == 20:
            title("SECURITY"); self.m3_rr(18,65,204,64,32,t["pc"] if cursor==0 else t["surface"]); text(42,78,"SCREEN LOCK",8,t["on"]); text(42,99,"ENABLED" if locked else "NONE",7,t["muted"]); toggle(169,85,bool(locked)); self.m3_rr(45,194,150,42,21,t["pc"] if cursor==1 else t["surface"]); text(120,207,"BACK",8,t["on"],True,True)

        elif screen == 21:
            title("APPS & NOTIFS"); labels=("NOTIFICATIONS","APP INFO","BACK"); cur=min(cursor,2)
            for i in range(3):
                y=57+i*64; row(y,labels[i],i==cur)
                if i==0: text(176,y+18,"NEW" if notes else "0",7,t["muted"])
                elif i==1: text(176,y+18,"6",7,t["muted"])

        elif screen == 22:
            title("DEVELOPER OPTIONS"); labels=("WIRELESS DEBUG","BUILD NUMBER","SYSTEM","BACK"); cur=min(cursor,3); start=min(max(cur-1,0),1)
            for r in range(3):
                i=min(3,start+r); y=57+r*64; row(y,labels[i],i==cur)
                if i==0: toggle(174,y+13,bool(adb))
                elif i==1: text(157,y+18,"17.2.1",7,t["primary"])
            if adb: text(120,253,"WIRELESS ADB IS A DEVELOPER FEATURE",6,t["error"],False,True)

        elif screen == 23:
            title("AVAILABLE NETWORKS")
            if wifi_ap_count <= 0:
                self.m3_circle(120,104,34,t["surface"]); icon_wifi(120,104,t["outline"])
                text(120,158,"SCANNING...",10,t["on"],True,True); text(120,184,"ROTATE CROWN AFTER SCAN",7,t["muted"],False,True)
            elif cursor >= wifi_ap_count:
                self.m3_rr(30,87,180,94,36,t["pc"]); text(120,113,"BACK",12,t["on_pc"],True,True); text(120,146,"RETURN TO WI-FI",7,t["muted"],False,True)
            else:
                self.m3_rr(16,55,208,145,38,t["surface"]); icon_wifi(120,91,t["primary"] if wifi_ap_rssi>-70 else t["outline"])
                text(120,124,wifi_ap_ssid or "UNKNOWN",10,t["on"],True,True)
                sig="EXCELLENT" if wifi_ap_rssi>-55 else ("GOOD" if wifi_ap_rssi>-70 else "WEAK")
                text(120,151,sig,7,t["muted"],False,True); text(120,176,"PRESS TO CONNECT",7,t["primary"],False,True)

        elif screen == 24:
            page=(wifi_editor_char >> 6) & 0x03; selected=wifi_editor_char & 0x3f
            alpha="qwertyuiopasdfghjklzxcvbnm"
            symbols1="1234567890!@#$%^&*()_+-=[]"
            symbols2="{}.,:;?'\"/\\|<>`~"
            chars=symbols1 if page==2 else symbols2 if page==3 else alpha
            text(120,5,wifi_ap_ssid or "NETWORK",7,t["muted"],False,True)
            self.m3_rr(12,21,216,40,18,t["surface"])
            preview="PASSWORD" if wifi_password_len==0 else "*"*min(16,wifi_password_len)
            text(25,34,preview,11 if wifi_password_len else 7,t["on"] if wifi_password_len else t["outline"],False,False)
            self.m3_circle(211,41,5,t["pc"]); self.m3_circle(211,41,2,t["primary"])

            def kbchar(x,y,w,h,ch,index):
                sel=(selected==index); yy=y-3 if sel else y; hh=h+6 if sel else h
                self.m3_rr(x,yy,w,hh,8,t["primary"] if sel else t["high"])
                text(x+w/2,yy+(5 if sel else 8),ch.upper() if page==1 else ch,10 if sel else 7,t["on_primary"] if sel else t["on"],True,True)

            if page in (0,1):
                rows=(10,9,7); base=0
                for r,count in enumerate(rows):
                    y=76+r*37; kw=20 if r<2 else 22; gap=2 if r<2 else 3
                    rw=count*kw+(count-1)*gap; x=(240-rw)//2
                    for pos in range(count):
                        idx=base+pos; kbchar(x,y,kw,29,chars[idx],idx); x += kw+gap
                    base += count
            else:
                rows=(10,10,6) if page==2 else (9,8); base=0
                for r,count in enumerate(rows):
                    y=76+r*37; kw=20 if count>=10 else 22; gap=2; rw=count*kw+(count-1)*gap; x=(240-rw)//2
                    for pos in range(count):
                        idx=base+pos
                        if idx>=len(chars): break
                        kbchar(x,y,kw,29,chars[idx],idx); x += kw+gap
                    base += count
            n=len(chars)
            page_key="#+=" if page==2 else "123" if page==3 else ("abc" if page==1 else "SHIFT")
            alpha_key="ABC" if page in (2,3) else "123"
            def kbaction(x,y,w,h,label,index):
                sel=(selected==index); self.m3_rr(x,y,w,h,8,t["primary"] if sel else t["high"]); text(x+w/2,y+9,label,7,t["on_primary"] if sel else t["on"],True,True)
            kbaction(6,188,50,31,page_key,n); kbaction(60,188,42,31,alpha_key,n+1); kbaction(106,188,74,31,"SPACE",n+2); kbaction(184,188,50,31,"DEL",n+3)
            kbaction(6,226,64,34,"BACK",n+5); kbaction(76,226,158,34,"CONNECT",n+4)
            text(120,266,"ROTATE  /  PRESS",7,t["outline"],False,True)

        elif screen == 25:
            title("WI-FI"); self.m3_circle(120,102,38,t["surface"]); icon_wifi(120,102,t["primary"] if wifi_link_state==4 else t["outline"])
            states={4:("CONNECTED","TIME SYNC STARTED"),3:("GETTING ADDRESS","DHCP"),2:("LINK READY","STARTING NETWORK"),1:("CONNECTING","ASSOCIATING")}
            a,b=states.get(wifi_link_state,("NOT CONNECTED","PRESS TO RETURN")); text(120,159,a,11,t["primary"] if wifi_link_state==4 else t["on"],True,True); text(120,187,b,7,t["muted"],False,True)
            self.m3_rr(48,222,144,38,19,t["high"]); text(120,234,"DONE",8,t["on"],True,True)

        elif screen == 26:
            labels=("CLOCK","MEDIA","SETTINGS","CONNECTIVITY","SYSTEM INFO","ASSISTANT","GRID VIEW")
            cur=min(cursor,6); start=max(0,min(cur-1,4))
            for r in range(3):
                i=min(start+r,6); y=44+r*66; sel=(i==cur)
                if i==6:
                    x,w,h=(22,196,54) if sel else (34,172,46)
                    self.m3_rr(x,y,w,h,h/2,t["primary"] if sel else t["high"]); icon_grid(x+31,y+h/2,t["on_primary"] if sel else t["muted"]); text(x+58,y+17,"GRID VIEW",9,t["on_primary"] if sel else t["on"],True)
                else:
                    x,w,h,radius=(5,230,50,25) if sel else (17,206,42,21)
                    self.m3_rr(x,y,w,h,radius,t["pc"] if sel else t["surface"]); self.m3_circle(x+25,y+h/2,12 if sel else 9,app_accents[i]); app_icon(i,x+25,y+h/2,"#0a2218"); text(x+48,y+14,labels[i],9 if len(labels[i])<12 else 7,t["on_pc"] if sel else t["on"],True)
            self.m3_circle(228,72+(cur-start)*66,3,t["primary"])

        elif screen == 27:
            x,y=self.p(120,116); rr=70*self.scale
            self.canvas.create_oval(x-rr,y-rr,x+rr,y+rr,outline=t["high"],width=max(1,int(2*self.scale)))
            icon_assistant(120,91,t["primary"]); text(120,126,"ASK ASSISTANT",12,t["on"],True,True); text(120,158,"COMPANION APP REQUIRED",7,t["muted"],False,True)
            for x0,x1,c in ((79,101,"#50beff"),(101,123,"#7ee1a5"),(123,145,"#e0bbff"),(145,167,"#ffb8a1")): line(x0,183,x1,183,c,3)
            self.m3_rr(51,224,138,38,19,t["high"]); text(120,236,"PRESS TO RETURN",7,t["muted"],False,True)

        else:
            title("WEAR OS")
            text(120,130,"ANDROID 17",12,t["on"],True,True)

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
        now = time.monotonic()
        if self.connected and self.last_pong and now - self.last_pong > 3.0:
            port = self.link.port or "?"
            self.status.configure(text=f"ESP32-S3 • {port} • USB open, waiting for board")
        self.root.after(500, self.health_tick)

    def close(self):
        self.link.close()
        self.root.destroy()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", help="preferred CDC port for direct-serial mode, e.g. /dev/ttyACM0")
    ap.add_argument("--bridge", type=Path, default=default_viewer_socket(), help="Unix socket exposed by usb_block_server.py")
    ap.add_argument("--direct-serial", action="store_true", help="bypass the local bridge and open the TTY directly")
    ap.add_argument("--scale", type=float, default=1.6)
    args = ap.parse_args()

    messages: queue.Queue[str] = queue.Queue()
    # Critical no-reset rule: if the PC block server owns the TTY, the GUI
    # must attach to its local socket instead of opening /dev/ttyACM* again.
    if not args.direct_serial and args.bridge.exists():
        link = BridgeLink(args.bridge, messages)
    else:
        link = SerialLink(args.port, messages)

    root = tk.Tk()
    WatchViewer(root, link, max(1.0, args.scale))
    try:
        root.mainloop()
    except KeyboardInterrupt:
        # Ctrl+C is a normal development exit, not an error.
        try:
            link.close()
        finally:
            try:
                root.destroy()
            except tk.TclError:
                pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
