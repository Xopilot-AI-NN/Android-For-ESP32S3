#!/usr/bin/env python3
"""Push files through Android ADB, preparing photos for the watch decoder."""
from __future__ import annotations
import argparse
from contextlib import contextmanager
from pathlib import Path
import shutil
import subprocess
import tempfile


@contextmanager
def prepared_image(source: Path, original: bool = False, max_side: int = 960):
    """Bound photo size, preserve aspect/orientation, and never modify the source."""
    if original or source.suffix.lower() not in ('.jpg', '.jpeg', '.png'):
        yield source
        return
    try:
        from PIL import Image, ImageOps
    except ImportError as error:
        raise RuntimeError('Image push needs Pillow: python3 -m pip install Pillow; or use --original') from error
    with tempfile.TemporaryDirectory(prefix='watch-push-') as directory:
        target = Path(directory) / source.name
        with Image.open(source) as image:
            image.seek(0)
            image = ImageOps.exif_transpose(image)
            image.thumbnail((max_side, max_side), Image.Resampling.LANCZOS)
            if source.suffix.lower() in ('.jpg', '.jpeg'):
                image.convert('RGB').save(target, 'JPEG', quality=82, optimize=True, progressive=False)
            else:
                image = image.convert('RGBA' if 'transparency' in image.info or 'A' in image.getbands() else 'RGB')
                image.info.clear()
                image.save(target, 'PNG', optimize=True)
        # Always use the normalized output: even a smaller original can be a
        # progressive JPEG or interlaced PNG unsupported by the device.
        print(f'Image: {source.stat().st_size} → {target.stat().st_size} bytes', flush=True)
        yield target


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('-s', '--serial')
    parser.add_argument('command', choices=['push'])
    parser.add_argument('source', type=Path)
    parser.add_argument('destination')
    parser.add_argument('--original', action='store_true', help='send exact source bytes')
    parser.add_argument('--max-side', type=int, default=960, help='photo longest side, 240..1920 (default 960)')
    args = parser.parse_args(argv)
    if not args.source.is_file():
        parser.error('source must be a file')
    if not 240 <= args.max_side <= 1920:
        parser.error('--max-side must be 240..1920')
    executable = shutil.which('adb')
    wrappers = {Path(__file__).resolve().parent / 'adb', Path(__file__).resolve().parent.parent / 'adb'}
    if executable is None or Path(executable).resolve() in wrappers:
        executable = '/usr/bin/adb'
    command = [executable] + (['-s', args.serial] if args.serial else [])
    with prepared_image(args.source, args.original, args.max_side) as source:
        return subprocess.run(command + ['push', str(source), args.destination], check=False).returncode


if __name__ == '__main__':
    raise SystemExit(main())
