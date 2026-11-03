#!/usr/bin/env python3
"""Import a Google Web OAuth JSON into a private config directory outside Git."""
import argparse
import json
import os
from pathlib import Path
import secrets
import tempfile
from urllib.parse import urlsplit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("oauth_json", type=Path, help="downloaded client_secret*.json; do not copy into this repo")
    parser.add_argument("--base-url", default="http://localhost:8080")
    args = parser.parse_args()
    base = args.base_url.rstrip("/")
    uri = urlsplit(base)
    local = uri.scheme == "http" and uri.hostname == "localhost"
    if not uri.hostname or uri.username or uri.password or uri.query or uri.fragment or uri.path or not (local or uri.scheme == "https"):
        parser.error("base URL must be a bare HTTPS origin, or http://localhost:8080 for local testing")
    with args.oauth_json.open() as source:
        client = json.load(source).get("web")
    if not client or not client.get("client_id") or not client.get("client_secret"):
        parser.error("expected a Web application OAuth JSON")
    callback = base + "/oauth/callback"
    if callback not in client.get("redirect_uris", []):
        parser.error("add the exact callback in Google Console and download JSON again: " + callback)
    directory = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "aosp-wear-gateway"
    project = Path(__file__).resolve().parent
    repository = next((p for p in (project, *project.parents) if (p / ".git").exists()), project)
    if directory.resolve().is_relative_to(repository):
        parser.error("private config must be outside the Git checkout")
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(directory, 0o700)
    target = directory / "config.env"
    # Preserve the encryption key on reconfiguration; changing it would orphan stored tokens.
    key = None
    if target.exists():
        for line in target.read_text().splitlines():
            if line.startswith("STORE_KEY="):
                key = json.loads(line.split("=", 1)[1])
    settings = {"PUBLIC_BASE_URL": base, "DEV_LOCALHOST": "1" if local else "0",
                "GOOGLE_CLIENT_ID": client["client_id"], "GOOGLE_CLIENT_SECRET": client["client_secret"],
                "STORE_KEY": key or secrets.token_hex(32), "PORT": str((uri.port or 80) if local else 8080)}
    if any("\n" in value or "\r" in value for value in settings.values()):
        parser.error("invalid multiline credential value")
    # A unique, exclusively created file avoids following a stale temporary symlink.
    descriptor, temporary = tempfile.mkstemp(prefix=".config-", dir=directory)
    if hasattr(os, "fchmod"):
        os.fchmod(descriptor, 0o600)
    with os.fdopen(descriptor, "w") as file:
        file.write("".join(f"{name}={json.dumps(value)}\n" for name, value in settings.items()))
    os.replace(temporary, target)
    print("Private config saved outside Git:", target)
    print("Callback:", callback)
    if local:
        print("Local test: open the login page on the same phone or PC running Gateway. Another device needs HTTPS.")


if __name__ == "__main__":
    main()
