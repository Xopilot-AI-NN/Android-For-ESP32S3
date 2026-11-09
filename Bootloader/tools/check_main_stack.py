#!/usr/bin/env python3
"""Reject known ESP32-S3 main/disk/input stack overflows before packaging.

This checks the named call paths plus 32 KiB headroom, not whole-program WCET
or an exhaustive stack proof. Runtime guard monitoring remains enabled.
"""
from pathlib import Path
import argparse
import re
import shutil
import subprocess

HEADROOM = 32 * 1024


def frame_size(disassembly: str) -> int:
    entry = re.search(r'\bentry\s+a1,\s*(0x[0-9a-f]+|\d+)', disassembly)
    if not entry:
        raise ValueError('unrecognized Xtensa function prologue')
    size = int(entry[1], 0)
    # Large frames use a minimal ENTRY then a literal load/SUB/MOVSP.
    if re.search(r'\bmovsp\s+a1,\s*a8', disassembly):
        literal = re.search(r'\bl32r\s+a8,.*?\(([0-9a-f]+)\s', disassembly)
        if not literal or not re.search(r'\bsub\s+a8,\s*a1,\s*a8', disassembly):
            raise ValueError('unrecognized extended Xtensa frame')
        size += int(literal[1], 16)
    return size


def remaining(stack: int, frames: list[int]) -> int:
    available = stack - sum(frames)
    if available < HEADROOM:
        raise ValueError(f'main stack headroom {available} bytes; need >= {HEADROOM}')
    return available


def tool(name: str) -> str:
    found = shutil.which(f'xtensa-esp-elf-{name}')
    if found:
        return found
    candidates = sorted((Path.home() / '.rustup/toolchains/esp').glob(f'**/bin/xtensa-esp-elf-{name}'))
    if not candidates:
        raise ValueError(f'Xtensa {name} unavailable; load export-esp.sh')
    return str(candidates[0])


def check(elf: Path):
    table = subprocess.check_output([tool('nm'), '-n', str(elf)], text=True)
    symbols = {}
    for line in table.splitlines():
        fields = line.split()
        if len(fields) == 3:
            symbols[fields[2]] = int(fields[0], 16)
    total = symbols['_stack_start_cpu0'] - symbols['_stack_end_cpu0']
    def frame(fragment, *, inlined=False):
        matches = [address for name, address in symbols.items()
                   if (name.endswith(fragment) if fragment == '4init3run' else fragment in name)]
        if not matches and inlined:
            # LLVM folded this helper into its caller; its locals are already
            # included in that caller's prologue, so do not count them twice.
            return 0
        if len(matches) != 1:
            raise ValueError(f'expected one {fragment} symbol, got {len(matches)}')
        address = matches[0]
        disassembly = subprocess.check_output(
            [tool('objdump'), '-d', f'--start-address={address}',
             f'--stop-address={address + 32}', str(elf)], text=True)
        return frame_size(disassembly)

    init = frame('4init3run')
    render = frame('7Runtime6render')
    # Scene construction and enrichment run sequentially, so budget the larger
    # child. Include cold scene construction, not just the small cache-hit path.
    render_child = max(frame('10SceneCache6render'), frame('7Runtime12enrich_frame'))
    paths = {
        'disk/redraw/scene': [init, frame('21service_disk_requests'),
                              frame('17redraw_after_disk'), render, render_child],
        'input/pager/scene': [init, frame('19dispatch_with_pager'),
                              frame('7Runtime8dispatch'), render, render_child],
        'browser/keyboard paint': [init, frame('21service_disk_requests'),
                                    frame('17redraw_after_disk'), frame('11BootDisplay7runtime'),
                                    frame('7Surface6render'), frame('18native_application'),
                                    frame('12browser_page', inlined=True), frame('15remote_app_page'),
                                    frame('13text_keyboard')],
    }
    for label, frames in paths.items():
        free = remaining(total, frames)
        print(f'Main stack: {total} bytes; {label} frames {frames}; reserve {free} bytes')



if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('elf', type=Path)
    args = parser.parse_args()
    try:
        check(args.elf)
    except (ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'Stack budget check failed: {error}\n')
