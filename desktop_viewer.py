#!/usr/bin/env python3
"""Zephyr Watch USB Desktop Display v3.0.

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
        return Path(runtime) / "zephyr-watch-viewer.sock"
    uid = os.getuid() if hasattr(os, "getuid") else 0
    return Path(f"/tmp/zephyr-watch-viewer-{uid}.sock")


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
        vals = list(state[1:14]) + ["0"] * 13
        try:
            screen, cursor, brightness, dnd, airplane, theme, locked, wifi, bt, adb, notes, mins, swap_pages = [int(v) for v in vals[:13]]
        except ValueError:
            return
        t = self.m3_palette(theme)
        self.canvas.delete("all")
        self.m3_rr(0, 0, 240, 280, 34, t["bg"], outline="#4a4d52")

        def text(x, y, value, size=12, color=None, bold=False, center=False):
            px, py = self.p(x, y)
            self.canvas.create_text(px, py, text=value, fill=color or t["on"],
                font=("Sans", max(8, int(size*self.scale)), "bold" if bold else "normal"),
                anchor="n" if center else "nw")

        def appbar(title):
            self.m3_rr(10, 8, 220, 34, 17, t["surface"])
            self.m3_circle(28, 25, 7, t["primary"])
            text(43, 16, title, 12, bold=True)

        def status_icons():
            self.m3_circle(18,18,5,t["primary"] if wifi else t["high"]); text(29,12,"W",8,t["primary"] if wifi else t["outline"],bold=True)
            self.m3_circle(59,18,5,t["primary"] if bt else t["high"]); text(70,12,"B",8,t["primary"] if bt else t["outline"],bold=True)
            if adb:
                self.m3_rr(91,10,31,16,8,t["pc"]); text(97,12,"ADB",7,t["on_pc"],bold=True)
            text(181,12,"USB",8,t["muted"])

        def card(y, label, selected):
            x,w,h,r = (8,224,46,22) if selected else (13,214,40,17)
            bg=t["pc"] if selected else t["surface"]; fg=t["on_pc"] if selected else t["on"]
            self.m3_rr(x,y,w,h,r,bg); self.m3_circle(x+21,y+h/2,9 if selected else 7,t["primary"] if selected else t["high"])
            text(x+39,y+10,label,11,fg,bold=selected); text(x+w-20,y+8,"›",15,fg)

        def toggle(x,y,on):
            self.m3_rr(x,y,42,24,12,t["primary"] if on else t["high"])
            self.m3_circle(x+(30 if on else 12),y+12,8,t["on_primary"] if on else t["muted"])

        hour=(mins//60)%24; minute=mins%60; clock=f"{hour:02d}:{minute:02d}"

        if screen == 0:
            status_icons(); text(120,69,clock,34,t["on"],bold=True,center=True); text(120,120,"ZEPHYR WATCH",9,t["muted"],center=True)
            self.m3_circle(120,158,22,t["pc"]); self.m3_rr(112,146,16,22,7,t["primary"]); self.m3_rr(115,141,10,15,5,t["primary"])
            if notes:
                self.m3_rr(34,194,172,42,21,t["surface"]); text(52,204,"1 NOTIFICATION",9,t["on"],bold=True); self.m3_circle(188,215,7,t["primary"])
            text(120,253,"PRESS CROWN TO UNLOCK",8,t["outline"],center=True)
        elif screen == 1:
            status_icons(); text(120,59,clock,34,t["primary"],bold=True,center=True); text(120,111,"ZEPHYR",8,t["muted"],center=True)
            self.m3_rr(18,143,96,58,25,t["pc"] if notes else t["surface"]); text(33,154,"MESSAGES",8,t["on_pc"] if notes else t["on"],bold=True); text(33,175,"1 NEW" if notes else "CLEAR",12,t["primary"] if notes else t["muted"],bold=True)
            self.m3_rr(126,143,96,58,25,t["surface"]); text(143,154,"QUICK",8,t["on"],bold=True); text(143,175,"DND" if dnd else "READY",12,t["primary"] if dnd else t["muted"],bold=True)
            self.m3_rr(46,215,148,30,15,t["secondary"]); text(120,222,"CROWN: APPS",8,t["on"],bold=True,center=True); text(120,256,"LEFT MESSAGES  RIGHT QUICK",7,t["outline"],center=True)
        elif screen == 2:
            appbar("APPS"); labels=("MESSAGES","SETTINGS","CLOCK","CONNECT","ABOUT"); cur=min(cursor,4); start=1 if cur>=4 else 0
            for row in range(4): i=start+row; card(52+row*52,labels[i],cur==i)
        elif screen == 3:
            appbar("MESSAGES"); self.m3_rr(12,56,216,88,28,t["pc"]); self.m3_circle(38,83,12,t["primary"]); text(58,67,"SYSTEM",12,t["on_pc"],bold=True); text(58,94,"ZEPHYR IS READY",8,t["on_pc"]); text(58,112,"PHONE LINK WAITING",7,t["muted"])
            self.m3_rr(12,156,216,66,25,t["surface"]); text(31,169,"CHATS",12,t["on"],bold=True); text(31,196,"NO PHONE SYNC YET",8,t["muted"]); text(120,251,"CROWN: BACK",8,t["outline"],center=True)
        elif screen == 4:
            appbar("QUICK"); tiles=((12,52,"WIFI",wifi),(124,52,"BT",bt),(12,112,"ADB",adb),(124,112,"DND",dnd))
            for i,(x,y,label,on) in enumerate(tiles):
                sel=cursor==i; self.m3_rr(x,y,104,52,25 if sel else 20,t["pc"] if sel or on else t["surface"]); self.m3_circle(x+22,y+20,9,t["primary"] if on else t["high"]); text(x+39,y+10,label,9,t["on_pc"] if sel or on else t["on"],bold=True); text(x+39,y+28,"ON" if on else "OFF",7,t["muted"])
            sel=cursor==4; self.m3_rr(8 if sel else 12,174,224 if sel else 216,34,17,t["pc"] if sel or airplane else t["surface"]); text(29,182,"AIRPLANE",9,t["on_pc"] if sel or airplane else t["on"],bold=True); text(181,182,"ON" if airplane else "OFF",8,t["muted"])
            sel=cursor==5; self.m3_rr(8 if sel else 12,218,224 if sel else 216,44,20,t["pc"] if sel else t["surface"]); text(26,226,"BRIGHT",8,t["on_pc"] if sel else t["on"]); self.m3_rr(92,235,116,8,4,t["high"]); active=int(108*brightness/100); self.m3_rr(92,235,active+8,8,4,t["primary"]); self.m3_circle(96+active,239,7,t["primary"])
        elif screen == 5:
            appbar("SETTINGS"); labels=("BRIGHTNESS","THEME","CONNECT","ABOUT","BACK"); cur=min(cursor,4); start=1 if cur>=4 else 0
            for row in range(4):
                i=start+row; y=52+row*52; card(y,labels[i],cur==i)
                if i==0: text(170,y+14,f"{brightness}%",8,t["muted"])
                if i==1: text(158,y+14,("GREEN","BLUE","PURPLE","CORAL")[theme%4],7,t["muted"])
        elif screen == 6:
            appbar("CLOCK"); self.m3_rr(12,58,216,158,34,t["surface"]); text(120,86,clock,34,t["primary"],bold=True,center=True); text(120,145,"UPTIME CLOCK",8,t["muted"],center=True); self.m3_rr(48,178,144,26,13,t["secondary"]); text(120,184,"RTC / NTP READY",7,t["on"],center=True); text(120,248,"CROWN: BACK",8,t["outline"],center=True)
        elif screen == 7:
            appbar("CONNECT"); labels=("WIFI","BLUETOOTH","WADB","AIRPLANE","BACK"); vals=(wifi,bt,adb,airplane,False); cur=min(cursor,4); start=1 if cur>=4 else 0
            for row in range(4):
                i=start+row; y=52+row*52; card(y,labels[i],cur==i)
                if i<4: toggle(171,y+9,vals[i])
        else:
            appbar("ABOUT"); self.m3_rr(12,56,216,194,32,t["surface"]); self.m3_circle(120,91,25,t["pc"]); text(120,75,"Z",28,t["primary"],bold=True,center=True); text(120,126,"ZEPHYR ANDROID",12,t["on"],bold=True,center=True); text(120,154,"VERSION 0.4.0",8,t["muted"],center=True); self.m3_rr(31,178,178,27,13,t["secondary"]); text(120,185,"ESP32-S3 / RHAI",7,t["on"],center=True); text(120,212,"PSRAM 2M / SWAP 32M",7,t["muted"],center=True); text(120,231,"PAGER ACTIVE" if swap_pages else "PAGER READY",7,t["primary"] if swap_pages else t["outline"],center=True); text(120,257,"AVB ORANGE",7,t["error"],center=True)

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
    root.mainloop()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
