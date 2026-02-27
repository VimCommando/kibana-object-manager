#!/usr/bin/env python3
"""Update kibob Homebrew formula URL and SHA256 for a release tag.

Usage:
  python update_homebrew_formula.py --version 0.2.0 --formula /path/to/Formula/kibob.rb

This script fetches the GitHub release tarball for the given version,
computes SHA256, updates `url` and `sha256` fields in the formula, and
prints the resulting values.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
import urllib.request
from pathlib import Path

REPO = "VimCommando/kibana-object-manager"
URL_TEMPLATE = "https://github.com/{repo}/archive/refs/tags/v{version}.tar.gz"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True, help="Release version, e.g. 0.2.0")
    parser.add_argument("--formula", required=True, help="Path to Formula/kibob.rb")
    return parser.parse_args()


def tarball_url(version: str) -> str:
    return URL_TEMPLATE.format(repo=REPO, version=version)


def sha256_url(url: str) -> str:
    hasher = hashlib.sha256()
    with urllib.request.urlopen(url) as response:
        while True:
            chunk = response.read(1024 * 1024)
            if not chunk:
                break
            hasher.update(chunk)
    return hasher.hexdigest()


def patch_formula(content: str, new_url: str, new_sha256: str) -> str:
    url_pattern = re.compile(r'^(\s*url\s+")[^"]+("\s*)$', re.MULTILINE)
    sha_pattern = re.compile(r'^(\s*sha256\s+")[0-9a-fA-F]+("\s*)$', re.MULTILINE)

    if not url_pattern.search(content):
        raise ValueError("Could not find `url` line in formula")
    if not sha_pattern.search(content):
        raise ValueError("Could not find `sha256` line in formula")

    content = url_pattern.sub(rf'\1{new_url}\2', content, count=1)
    content = sha_pattern.sub(rf'\1{new_sha256}\2', content, count=1)
    return content


def main() -> int:
    args = parse_args()
    formula_path = Path(args.formula)
    if not formula_path.exists():
        print(f"Formula file not found: {formula_path}", file=sys.stderr)
        return 1

    url = tarball_url(args.version)
    try:
        digest = sha256_url(url)
    except Exception as exc:  # pragma: no cover
        print(f"Failed to download tarball for checksum: {exc}", file=sys.stderr)
        return 1

    original = formula_path.read_text()
    try:
        updated = patch_formula(original, url, digest)
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 1

    formula_path.write_text(updated)

    print(f"Updated formula: {formula_path}")
    print(f"url: {url}")
    print(f"sha256: {digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
