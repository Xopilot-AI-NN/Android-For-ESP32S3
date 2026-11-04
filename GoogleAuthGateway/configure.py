#!/usr/bin/env python3
"""Import a Google Web OAuth JSON into a private config directory outside Git."""
import argparse
import json
import ipaddress
import subprocess
import os
from pathlib import Path
import secrets
import tempfile
from urllib.parse import urlsplit


def create_local_certificates(directory, address):
    """Generate the LAN certificate locally; no account data or keys leave this PC."""
    def openssl(*arguments):
        subprocess.run(["openssl", *map(str, arguments)], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    ca, key = directory / "local-ca.pem", directory / "local-ca.key"
    old_mask = os.umask(0o077)
    try:
        if not ca.exists() and not key.exists():
            openssl("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "3650",
                    "-subj", "/CN=AOSP Wear Local CA", "-addext", "basicConstraints=critical,CA:TRUE",
                    "-addext", "keyUsage=critical,keyCertSign,cRLSign",
                    "-keyout", key, "-out", ca)
        elif not ca.exists() or not key.exists():
            raise ValueError("Incomplete local CA; restore its certificate and key before continuing")
        with tempfile.TemporaryDirectory(dir=directory) as temporary:
            work = Path(temporary)
            description = subprocess.run(["openssl", "x509", "-in", str(ca), "-noout", "-text"],
                                         check=True, capture_output=True, text=True).stdout
            if "X509v3 Key Usage:" not in description:
                # Repair older local CAs without changing their identity/key or orphaning watches.
                (work / "ca-extensions").write_text(
                    "basicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n")
                openssl("x509", "-in", ca, "-signkey", key, "-days", "3650",
                        "-extfile", work / "ca-extensions", "-out", work / "ca.pem")
                os.replace(work / "ca.pem", ca)
            openssl("req", "-new", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=AOSP Wear LAN",
                    "-keyout", work / "server.key", "-out", work / "server.csr")
            (work / "extensions").write_text(
                f"subjectAltName=IP:{address}\nbasicConstraints=critical,CA:FALSE\n"
                "keyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n")
            openssl("x509", "-req", "-in", work / "server.csr", "-CA", ca, "-CAkey", key,
                    "-set_serial", str(secrets.randbits(128)), "-days", "365",
                    "-extfile", work / "extensions", "-out", work / "server.pem")
            os.replace(work / "server.key", directory / "local-server.key")
            os.replace(work / "server.pem", directory / "local-server.pem")
    finally:
        os.umask(old_mask)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("oauth_json", type=Path, help="downloaded client_secret*.json; do not copy into this repo")
    parser.add_argument("--base-url", default="http://localhost:8080")
    parser.add_argument("--local-ip", help="private IPv4 of this PC; creates local HTTPS certificates")
    args = parser.parse_args()
    if args.local_ip:
        try:
            address = ipaddress.IPv4Address(args.local_ip)
            if not any(address in ipaddress.IPv4Network(network) for network in ("10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16")):
                raise ValueError()
        except ValueError:
            parser.error("--local-ip requires a private LAN IPv4 address")
        args.base_url = f"https://{address}:8443"
    base = args.base_url.rstrip("/")
    uri = urlsplit(base)
    local = uri.scheme == "http" and uri.hostname == "localhost"
    if not uri.hostname or uri.username or uri.password or uri.query or uri.fragment or uri.path or not (local or uri.scheme == "https"):
        parser.error("base URL must be a bare HTTPS origin, or http://localhost:8080 for local testing")
    with args.oauth_json.open() as source:
        document = json.load(source)
        client = document.get("web") or (document.get("installed") if args.local_ip else None)
    if not client or not client.get("client_id") or not client.get("client_secret"):
        parser.error("expected Web OAuth JSON, or Desktop OAuth JSON with --local-ip")
    callback = "http://localhost:8080/oauth/callback" if args.local_ip else base + "/oauth/callback"
    if not (args.local_ip and "installed" in document) and callback not in client.get("redirect_uris", []):
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
    provider_settings = {}
    if target.exists():
        for line in target.read_text().splitlines():
            if line.startswith("STORE_KEY="):
                key = json.loads(line.split("=", 1)[1])
            if line.split('=', 1)[0] in {"BRAVE_SEARCH_API_KEY", "GOOGLE_TRANSLATE_API_KEY"}:
                name, value = line.split('=', 1)
                provider_settings[name] = json.loads(value)
    settings = {"PUBLIC_BASE_URL": base, "DEV_LOCALHOST": "1" if local else "0",
                "GOOGLE_CLIENT_ID": client["client_id"], "GOOGLE_CLIENT_SECRET": client["client_secret"],
                "STORE_KEY": key or secrets.token_hex(32), "PORT": str((uri.port or 80) if local else 8080)}
    if args.local_ip:
        # The private CA survives reconfiguration so already provisioned watches keep trusting it.
        create_local_certificates(directory, str(address))
        settings.update({"LOCAL_IP": str(address), "OAUTH_REDIRECT_URL": callback,
                         "TLS_CERT": str(directory / "local-server.pem"),
                         "TLS_KEY": str(directory / "local-server.key"), "PORT": "8443"})
    settings.update(provider_settings)
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
    if args.local_ip:
        print("Local HTTPS:", base)
        print("Open Google login on this computer; watch and computer must share a LAN.")
        print("Watch CA:", directory / "local-ca.pem")
    if local:
        print("Local test: open the login page on the same phone or PC running Gateway. Another device needs HTTPS.")


if __name__ == "__main__":
    main()
