#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import re
from pathlib import Path


def update_slot(slot_dir: Path) -> None:
    manifest = slot_dir / "MANIFEST.TXT"
    text = manifest.read_text(encoding="utf-8")
    m = re.search(r"^entry=(.+)$", text, re.MULTILINE | re.IGNORECASE)
    if not m:
        raise SystemExit(f"{manifest}: missing entry=")
    entry = m.group(1).strip()
    script = slot_dir / entry
    digest = hashlib.sha256(script.read_bytes()).hexdigest()
    if re.search(r"^sha256=", text, re.MULTILINE | re.IGNORECASE):
        text = re.sub(r"^sha256=.*$", f"sha256={digest}", text, flags=re.MULTILINE | re.IGNORECASE)
    else:
        text = text.rstrip() + f"\nsha256={digest}\n"
    manifest.write_text(text, encoding="utf-8")
    print(f"{slot_dir.name}: {entry} -> {digest}")


def main() -> int:
    ap = argparse.ArgumentParser(description="Refresh Zephyr Watch boot manifest SHA-256 values")
    ap.add_argument("sdcard", nargs="?", type=Path, default=Path("sdcard"))
    args = ap.parse_args()
    for name in ("SLOTA", "SLOTB"):
        update_slot(args.sdcard / name)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
