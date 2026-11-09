# Модуль проверок logcat.
"""Exercise real bridge fanout and logcat parsing without a physical TTY."""
import socket
import tempfile
import unittest
import subprocess
import sys
import os
from pathlib import Path

from logcat import LogBuffer, Record, accepted, filters, format_record, parse_record
from usb_block_server import ViewerBridge


# Структура данных компонента проверок logcat.
class LogcatTests(unittest.TestCase):
    @unittest.skipUnless(os.name == "posix", "requires a POSIX pseudo-terminal")
    # Проверяет чтение реальных строк serial-logcat и корректное завершение клиента.
    def test_direct_serial_logcat_reads_real_lines_and_exits_cleanly(self):
        import pty
        import select
        import signal
        master, slave = pty.openpty()
        proc = subprocess.Popen(
            [sys.executable, str(Path(__file__).resolve().parents[1] / "logcat.py"),
             "--serial", os.ttyname(slave), "-v", "brief"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        try:
            self.assertTrue(select.select([proc.stdout], [], [], 3)[0], "serial frontend did not start")
            self.assertIn("Live USB logs", proc.stdout.readline())
            os.write(master, b"@ZWUI|M3|1|0\nE (123) sd: read failed\n")
            self.assertTrue(select.select([proc.stdout], [], [], 3)[0], "serial record did not arrive")
            self.assertEqual(proc.stdout.readline().strip(), "E/sd(0): read failed")
            proc.send_signal(signal.SIGINT)
            proc.communicate(timeout=3)
            self.assertEqual(proc.returncode, 0)
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.communicate()
            os.close(master)
            os.close(slave)

    # Проверяет уровни ESP-IDF, uptime, Unicode и исключение управляющих пакетов из лога.
    def test_idf_levels_uptime_unicode_and_protocol_exclusion(self):
        r = parse_record("\x1b[31mE (145) sd: CRC failed\x1b[0m", 1.25)
        self.assertEqual((r.level, r.tag, r.uptime_ms), ("E", "sd", 145))
        self.assertEqual(Record.from_wire(r.wire()), r)
        self.assertIn("E sd", format_record(r, "threadtime"))
        for line in ("@ZWUI|M3|1", "ZWUI|M3|1", "@ZBLK|READ|1"):
            self.assertIsNone(parse_record(line))
        russian = parse_record("settings: настройки сохранены", 1)
        self.assertEqual(Record.from_wire(russian.wire()), russian)
        self.assertTrue(accepted(r, filters(["*:W"], False)))
        self.assertFalse(accepted(russian, filters(["*:W"], False)))
        self.assertTrue(accepted(russian, filters(["settings:I"], True)))

    # Проверяет ограничение длины истории журнала.
    def test_history_is_bounded(self):
        logs = LogBuffer()
        for n in range(300):
            logs.add(f"radio: event {n}")
        self.assertEqual(len(logs.records), 128)
        self.assertEqual(logs.records[0].message, "event 172")

    # Проверяет dump/clear/live-логи и отсутствие их отправки как команд плате.
    def test_bridge_dump_clear_live_logs_never_become_board_commands(self):
        with tempfile.TemporaryDirectory() as tmp:
            bridge = ViewerBridge(Path(tmp) / "bridge.sock")
            bridge.start()
            try:
                # start() launches a thread; wait for its socket using connect,
                # not a fixed sleep that can race on a busy test machine.
                import time
                deadline = time.monotonic() + 2
                client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                while True:
                    try:
                        client.connect(str(bridge.path))
                        break
                    except (FileNotFoundError, ConnectionRefusedError):
                        if time.monotonic() > deadline:
                            raise
                        time.sleep(0.005)
                with client:
                    client.settimeout(2)
                    bridge.publish_log("radio: online")
                    client.sendall(b"@ZLOG|DUMP|1\n")
                    with client.makefile("rb") as stream:
                        lines = []
                        while (line := stream.readline().decode().strip()) != "@ZLOG|END":
                            lines.append(line)
                        self.assertTrue(any(Record.from_wire(x).message == "online" for x in lines))
                        self.assertIsNone(bridge.pop_command())
                        bridge.publish_log("E (42) test: live failure")
                        self.assertEqual(Record.from_wire(stream.readline().decode()).level, "E")
                        client.sendall(b"@ZLOG|CLEAR|128\n")
                        self.assertEqual(stream.readline().strip(), b"@ZLOG|END")
                        self.assertEqual(len(bridge.logs.records), 0)
            finally:
                bridge.close()

    # Проверяет настоящую CLI-команду logcat на отфильтрованной истории моста.
    def test_real_logcat_command_dumps_filtered_bridge_history(self):
        with tempfile.TemporaryDirectory() as tmp:
            bridge = ViewerBridge(Path(tmp) / "bridge.sock")
            bridge.start()
            try:
                import time
                deadline = time.monotonic() + 2
                while not bridge.path.exists():
                    if time.monotonic() >= deadline:
                        self.fail("bridge socket was not created")
                    time.sleep(0.005)
                bridge.publish_log("radio: connected")
                bridge.publish_log("E (42) sd: read failed")
                bridge.publish_log("@ZWUI|M3|1|0")
                result = subprocess.run(
                    [sys.executable, str(Path(__file__).resolve().parents[1] / "logcat.py"),
                     "--socket", str(bridge.path), "-d", "-v", "brief", "*:W"],
                    text=True, capture_output=True, timeout=3, check=True,
                )
                self.assertEqual(result.stdout.strip(), "E/sd(0): read failed")
                self.assertIsNone(bridge.pop_command())
            finally:
                bridge.close()


if __name__ == "__main__":
    unittest.main()
