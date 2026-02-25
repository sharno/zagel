#!/usr/bin/env python3
"""Create a SHA256SUMS file for release archives."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


class ReleaseArtifactsError(RuntimeError):
    pass


def sha256_for_file(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "dist_dir",
        type=Path,
        help="Directory containing packaged release artifacts",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("SHA256SUMS"),
        help="Output file path (default: SHA256SUMS in current directory)",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    dist_dir = args.dist_dir
    output_path = args.output

    if not dist_dir.exists() or not dist_dir.is_dir():
        raise ReleaseArtifactsError(f"dist directory does not exist: {dist_dir}")

    files = sorted(
        [
            file
            for file in dist_dir.iterdir()
            if file.is_file() and file.name != output_path.name
        ],
        key=lambda file: file.name,
    )
    if not files:
        raise ReleaseArtifactsError(f"no release artifacts found in {dist_dir}")

    lines = [f"{sha256_for_file(file)}  {file.name}" for file in files]
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
