#!/usr/bin/env python3
"""Decode ESP32-S3 addresses using the ELF from the exact flashed build."""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import shutil
import subprocess
import sys


class Decoder:
    """Cache symbol lookups; ordinary log lines never invoke a subprocess."""
    def __init__(self, elf: Path):
        self.elf = elf.resolve()
        if not self.elf.is_file():
            raise ValueError(f"ELF not found: {self.elf}")
        self.tool = shutil.which("xtensa-esp-elf-addr2line")
        if self.tool is None:
            candidates = list((Path.home() / ".rustup/toolchains/esp").glob("**/bin/xtensa-esp-elf-addr2line"))
            if candidates:
                self.tool = str(candidates[0])
        if self.tool is None:
            raise ValueError("xtensa-esp-elf-addr2line not found; load the ESP toolchain")
        self.cache: dict[str, str] = {}

    def decode(self, text: str) -> list[str]:
        """Match instruction addresses, not arbitrary hexadecimal payloads or secrets."""
        out = []
        for address in dict.fromkeys(re.findall(r"\b0x4[02][0-9a-fA-F]{6}\b", text)):
            if address not in self.cache:
                result = subprocess.run([self.tool, "-a", "-f", "-C", "-i", "-p", "-e", str(self.elf), address],
                                        text=True, capture_output=True, timeout=5, check=True)
                self.cache[address] = result.stdout.strip()
            out.append(self.cache[address])
        return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--elf", type=Path, required=True)
    parser.add_argument("input", type=Path, nargs="?")
    args = parser.parse_args()
    decoder = Decoder(args.elf)
    source = args.input.open() if args.input else sys.stdin
    try:
        for line in source:
            print(line.rstrip(), flush=True)
            for symbol in decoder.decode(line):
                print(f"  {symbol}", flush=True)
    finally:
        if args.input:
            source.close()


if __name__ == "__main__":
    main()
