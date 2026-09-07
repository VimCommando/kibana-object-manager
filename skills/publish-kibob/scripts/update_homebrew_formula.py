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
import io
import re
import sys
import tarfile
import tomllib
import urllib.request
from pathlib import Path

REPO = "VimCommando/kibana-object-manager"
URL_TEMPLATE = "https://github.com/{repo}/archive/refs/tags/v{version}.tar.gz"
MAX_ARCHIVE_BYTES = 128 * 1024 * 1024
VERSION = re.compile(r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True, help="Release version, e.g. 0.2.0")
    parser.add_argument("--formula", required=True, help="Path to Formula/kibob.rb")
    return parser.parse_args()


def tarball_url(version: str) -> str:
    if not VERSION.fullmatch(version):
        raise ValueError("Version must be a semantic version without a v prefix")
    return URL_TEMPLATE.format(repo=REPO, version=version)


def validate_archive(data: bytes, version: str) -> None:
    """Inspect source members without extracting or executing downloaded code."""
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        members = archive.getmembers()
        names = [Path(member.name) for member in members]
        if not names or any(path.is_absolute() or ".." in path.parts for path in names):
            raise ValueError("Source archive contains unsafe paths")
        roots = {path.parts[0] for path in names if path.parts}
        if len(roots) != 1:
            raise ValueError("Source archive must have one root directory")
        root = roots.pop()
        if len({member.name for member in members}) != len(members):
            raise ValueError("Source archive contains duplicate members")

        def read(name: str) -> str:
            member = archive.getmember(f"{root}/{name}")
            if not member.isfile() or member.size > 4 * 1024 * 1024:
                raise ValueError(f"Invalid source member: {name}")
            stream = archive.extractfile(member)
            if stream is None:
                raise ValueError(f"Unreadable source member: {name}")
            return stream.read().decode("utf-8")

        # Old source releases retain their original license entry.
        license_name = "LICENCE.md" if f"{root}/LICENCE.md" in archive.getnames() else "LICENSE"
        license_text = read(license_name)
        if "Apache License" not in license_text or "Version 2.0" not in license_text:
            raise ValueError("Source archive is missing the declared Apache-2.0 license")
        workspace = tomllib.loads(read("Cargo.toml"))["workspace"]
        lock = tomllib.loads(read("Cargo.lock"))
        for package in ("kibana-sync", "kibana-object-manager"):
            path = f"crates/{package}"
            if path not in workspace["members"]:
                raise ValueError(f"Workspace omits {package}")
            manifest = tomllib.loads(read(f"{path}/Cargo.toml"))["package"]
            if manifest["name"] != package or manifest["license"] != "Apache-2.0":
                raise ValueError(f"Unexpected package metadata for {package}")
            if not VERSION.fullmatch(manifest["version"]):
                raise ValueError(f"Invalid version for {package}")
            if package == "kibana-object-manager" and manifest["version"] != version:
                raise ValueError("CLI archive version does not match the selected release")
            if not any(p["name"] == package and p["version"] == manifest["version"] for p in lock["package"]):
                raise ValueError(f"Lockfile version mismatch for {package}")


def sha256_url(url: str, version: str) -> str:
    with urllib.request.urlopen(url, timeout=30) as response:
        data = response.read(MAX_ARCHIVE_BYTES + 1)
    if len(data) > MAX_ARCHIVE_BYTES:
        raise ValueError("Source archive exceeds 128 MiB")
    validate_archive(data, version)
    return hashlib.sha256(data).hexdigest()


def patch_formula(content: str, new_url: str, new_sha256: str) -> str:
    url_pattern = re.compile(r'^(\s*url\s+")[^"]+("\s*)$', re.MULTILINE)
    sha_pattern = re.compile(r'^(\s*sha256\s+")[0-9a-fA-F]+("\s*)$', re.MULTILINE)

    if not url_pattern.search(content):
        raise ValueError("Could not find `url` line in formula")
    if not sha_pattern.search(content):
        raise ValueError("Could not find `sha256` line in formula")

    content = url_pattern.sub(
        lambda m: f'{m.group(1)}{new_url}{m.group(2)}', content, count=1
    )
    content = sha_pattern.sub(
        lambda m: f'{m.group(1)}{new_sha256}{m.group(2)}', content, count=1
    )
    return content


def main() -> int:
    args = parse_args()
    formula_path = Path(args.formula)
    if not formula_path.exists():
        print(f"Formula file not found: {formula_path}", file=sys.stderr)
        return 1

    try:
        url = tarball_url(args.version)
        digest = sha256_url(url, args.version)
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
