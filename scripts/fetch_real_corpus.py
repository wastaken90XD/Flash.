#!/usr/bin/env python3
"""Fetches and verifies real-world SWF corpus files from corpus/real/MANIFEST.toml into corpus/real/cache/."""

import base64
import hashlib
import json
import pathlib
import subprocess
import sys
import tomllib
import urllib.request


def fetch_entry(entry: dict, cache_dir: pathlib.Path) -> pathlib.Path:
    for required in ("name", "url", "sha256", "license", "source_page"):
        val = str(entry.get(required, "")).strip()
        if not val:
            raise ValueError(f"Manifest entry missing required non-empty field '{required}': {entry}")

    name = entry["name"]
    expected_sha = entry["sha256"].lower()
    dest = cache_dir / f"{name}.swf"

    if dest.exists():
        actual = hashlib.sha256(dest.read_bytes()).hexdigest().lower()
        if actual == expected_sha:
            return dest

    raw_bytes = None
    url = entry["url"]
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "swfr-ci/1.0"})
        with urllib.request.urlopen(req, timeout=20) as resp:
            raw_bytes = resp.read()
    except Exception:
        api_path = entry.get("api_path")
        if api_path:
            out = subprocess.check_output(["gh", "api", api_path]).decode()
            info = json.loads(out)
            raw_bytes = base64.b64decode(info["content"])
        else:
            raise

    actual_sha = hashlib.sha256(raw_bytes).hexdigest().lower()
    if actual_sha != expected_sha:
        raise ValueError(
            f"SHA-256 mismatch for '{name}': expected {expected_sha}, got {actual_sha}"
        )

    dest.write_bytes(raw_bytes)
    return dest


def main() -> int:
    root = pathlib.Path(__file__).resolve().parent.parent
    manifest_path = root / "corpus" / "real" / "MANIFEST.toml"
    data = tomllib.loads(manifest_path.read_text())
    files = data.get("file", [])
    if not files:
        print("Error: corpus/real/MANIFEST.toml has no [[file]] entries", file=sys.stderr)
        return 1

    cache_dir = root / "corpus" / "real" / "cache"
    cache_dir.mkdir(parents=True, exist_ok=True)

    for entry in files:
        dest = fetch_entry(entry, cache_dir)
        print(f"Verified {entry['name']} ({dest.stat().st_size} bytes, sha256={entry['sha256'][:12]}...)")

    return 0


if __name__ == "__main__":
    sys.exit(main())
