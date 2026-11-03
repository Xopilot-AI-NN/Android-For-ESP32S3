#!/usr/bin/env python3
"""Fail closed for known credential files/Google secrets; never print matched values.

The check reads Git's index, not arbitrary files in the user's home directory.
Runtime credentials should live outside the repository; ignores are a second layer.
"""
from __future__ import annotations
import argparse
from pathlib import Path
import re
import subprocess
import sys

PRIVATE = re.compile(r"(^|/)(?:\.env(?:\..*)?|client_secret[^/]*\.json|credentials\.json|tokens?\.json|[^/]*\.(?:sqlite|db)(?:-.*)?|[^/]*\.key|[^/]*-key\.pem)$", re.I)
SECRET = [
    ("Google OAuth secret", re.compile(rb"GOCSPX-[A-Za-z0-9_-]{16,}")),
    ("Google API key", re.compile(rb"AIza[0-9A-Za-z_-]{30,}")),
    ("Google access token", re.compile(rb"ya29\.[A-Za-z0-9._-]{24,}")),
    ("Google refresh token", re.compile(rb"1//[0-9A-Za-z_-]{24,}")),
    ("private key", re.compile(rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----")),
]


def inspect(name: str, data: bytes) -> list[str]:
    """Report only a rule name and filename; secret bytes never reach stdout/stderr."""
    found = ["private runtime filename"] if PRIVATE.search(name) and not name.endswith(".env.example") else []
    if name.split("/")[0] in {"private", "secrets", "credentials", "data", "debug-logs"}:
        found.append("private runtime directory")
    if re.search(r"(^|/)(PXL_[^/]+|IMG_\d[^/]+|Screenshot[^/]+)\.(png|jpe?g)$", name, re.I):
        found.append("personal photo/capture filename")
    if name.lower().endswith((".img", ".elf", ".log")):
        found.append("private disk/debug output")
    if name.lower().endswith('.json') and re.search(rb'"(?:device_credential|brave_search_api_key|google_translate_api_key)"\s*:\s*"[^"\r\n]{8,}"',data):
        found.append('private device/provider credentials')
    # Large staged files are no exemption: a disk/log can contain credentials too.
    found += [label for label, pattern in SECRET if pattern.search(data)]
    return found


def check(repo: Path) -> list[tuple[str, str]]:
    names = subprocess.check_output(["git", "-C", str(repo), "ls-files", "-z"]).split(b"\0")
    failures = []
    for raw in names:
        if not raw:
            continue
        name = raw.decode("utf-8", "surrogateescape")
        # Gitlinks are checked through their own index below, not treated as files.
        if (repo / name).is_dir():
            continue
        data = subprocess.check_output(["git", "-C", str(repo), "show", f":{name}"])
        failures.extend((name, rule) for rule in inspect(name, data))
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    repos = [args.repo.resolve()] if args.repo else [root, *(root / name for name in ("App", "Bootloader", "Firmware"))]
    failures = [(repo.name, name, rule) for repo in repos for name, rule in check(repo)]
    for repo, name, rule in failures:
        print(f"BLOCKED {repo}/{name}: {rule}", file=sys.stderr)
    if not failures:
        print("Git index: no known credential files or Google secrets detected")
    return bool(failures)


if __name__ == "__main__":
    raise SystemExit(main())
