#!/usr/bin/env python3
"""Fetch and SHA-256-verify real-world SWF corpus files from corpus/real/MANIFEST.toml."""

import base64
import hashlib
import json
import pathlib
import sys
import tomllib
import urllib.request


def download_bytes(url: str) -> bytes:
    """Download from raw URL, falling back to api.github.com when raw host is restricted."""
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "swfr-corpus-fetcher/1.0"})
        with urllib.request.urlopen(req, timeout=20) as resp:
            return resp.read()
    except Exception:
        prefix = "https://raw.githubusercontent.com/"
        if not url.startswith(prefix):
            raise
        parts = url[len(prefix) :].split("/", 3)
        if len(parts) != 4:
            raise
        owner, repo, ref, rel_path = parts
        api_url = f"https://api.github.com/repos/{owner}/{repo}/contents/{rel_path}?ref={ref}"
        req = urllib.request.Request(
            api_url,
            headers={
                "User-Agent": "swfr-corpus-fetcher/1.0",
                "Accept": "application/vnd.github.v3+json",
            },
        )
        with urllib.request.urlopen(req, timeout=20) as resp:
            payload = json.loads(resp.read().decode("utf-8"))
        return base64.b64decode(payload["content"])


def main() -> int:
    repo_root = pathlib.Path(__file__).resolve().parent.parent
    manifest_path = repo_root / "corpus" / "real" / "MANIFEST.toml"
    cache_dir = repo_root / "corpus" / "real" / "cache"
    cache_dir.mkdir(parents=True, exist_ok=True)

    with manifest_path.open("rb") as f:
        manifest = tomllib.load(f)

    entries = manifest.get("swf", [])
    if not entries:
        print("ERROR: corpus/real/MANIFEST.toml contains no [[swf]] entries", file=sys.stderr)
        return 1

    for entry in entries:
        name = entry["name"]
        filename = entry["file"]
        url = entry["url"]
        expected_sha256 = entry["sha256"].lower()
        dest = cache_dir / filename

        if dest.exists():
            actual = hashlib.sha256(dest.read_bytes()).hexdigest().lower()
            if actual == expected_sha256:
                print(f"[cached] {name} ({filename}) sha256={actual[:16]}...")
                continue

        print(f"[fetch]  {name} <- {url}")
        data = download_bytes(url)
        actual = hashlib.sha256(data).hexdigest().lower()
        if actual != expected_sha256:
            print(
                f"ERROR: SHA-256 mismatch for {name}: expected {expected_sha256}, got {actual}",
                file=sys.stderr,
            )
            return 1

        dest.write_bytes(data)
        print(f"[verified] {name} ({len(data)} bytes) sha256={actual[:16]}...")

    print(f"Successfully verified {len(entries)} real-world SWF files in {cache_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
