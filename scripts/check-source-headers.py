#!/usr/bin/env python3
# This file is part of CORD – https://cord.network

# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later

# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

"""Check or rewrite tracked source files to the canonical CORD GPL-3 header."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SLASH_SUFFIXES = {".rs", ".ts", ".tsx", ".js", ".mjs", ".cjs", ".hbs", ".sol", ".go", ".java", ".kt", ".kts", ".swift", ".c", ".cc", ".cpp", ".h", ".hpp"}
HASH_SUFFIXES = {".py", ".sh"}
SPECIAL_HASH_FILES = {"Dockerfile"}
SPDX = "SPDX-License-Identifier:"


def tracked_source_files() -> list[Path]:
    raw = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT)
    paths = []
    for value in raw.split(b"\0"):
        if not value:
            continue
        path = Path(value.decode())
        if path.suffix in SLASH_SUFFIXES | HASH_SUFFIXES or path.name in SPECIAL_HASH_FILES:
            paths.append(path)
    return sorted(paths)


def canonical_header(comment: str) -> str:
    source = (ROOT / "HEADER-GPL3").read_text(encoding="utf-8")
    if comment == "//":
        return source
    return "\n".join(
        (comment + line[2:]) if line.startswith("//") else line for line in source.splitlines()
    ) + "\n"


def comment_style(path: Path) -> str:
    return "#" if path.suffix in HASH_SUFFIXES or path.name in SPECIAL_HASH_FILES else "//"


def split_shebang(text: str, style: str) -> tuple[str, str]:
    if style == "#" and text.startswith("#!"):
        end = text.find("\n")
        if end < 0:
            return text + "\n", ""
        return text[: end + 1], text[end + 1 :]
    return "", text


def strip_existing_license(body: str, style: str) -> str:
    lines = body.splitlines(keepends=True)
    spdx_index = next((index for index, line in enumerate(lines[:80]) if SPDX in line), None)
    if spdx_index is None:
        return body

    prefix = lines[:spdx_index]
    if any(line.strip() and not line.lstrip().startswith(style) for line in prefix):
        return body

    end = spdx_index + 1
    while end < len(lines):
        stripped = lines[end].lstrip()
        if not stripped.strip():
            end += 1
            continue
        if stripped.startswith("//!") or stripped.startswith("///"):
            break
        if stripped.startswith(style):
            end += 1
            continue
        break
    return "".join(lines[end:]).lstrip("\n")


def expected_text(path: Path, text: str) -> str:
    style = comment_style(path)
    shebang, body = split_shebang(text, style)
    header = canonical_header(style)
    if body.startswith(header):
        body = body[len(header) :]
        if body.startswith("\n"):
            body = body[1:]
    else:
        body = strip_existing_license(body, style)
    return f"{shebang}{header}\n{body}"


def check_file(path: Path, fix: bool) -> str | None:
    absolute = ROOT / path
    try:
        current = absolute.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return "is not valid UTF-8"
    expected = expected_text(path, current)
    if current == expected:
        style = comment_style(path)
        spdx_lines = [
            line for line in current.splitlines() if line.lstrip().startswith(style) and SPDX in line
        ]
        if len(spdx_lines) != 1:
            return "does not contain exactly one SPDX declaration"
        return None
    if fix:
        absolute.write_text(expected, encoding="utf-8")
        return None
    return "does not start with the canonical HEADER-GPL3 content"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fix", action="store_true", help="rewrite non-conforming source headers")
    args = parser.parse_args()

    failures = []
    files = tracked_source_files()
    for path in files:
        failure = check_file(path, args.fix)
        if failure:
            failures.append(f"{path}: {failure}")

    if failures:
        print("FAIL source header policy", file=sys.stderr)
        print("\n".join(failures), file=sys.stderr)
        return 1
    action = "normalized" if args.fix else "verified"
    print(f"PASS source header policy: {action} {len(files)} tracked source files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
