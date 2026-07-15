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

"""Inherited audit-hook tracer for repository reads and subprocess ancestry."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path


TRACE_FILE = os.environ.get("CORD_EVIDENCE_TRACE_FILE")
TRACE_ROOTS = tuple(
    Path(value).resolve() for value in json.loads(os.environ.get("CORD_EVIDENCE_TRACE_ROOTS", "[]"))
)
TRACE_FD = os.open(TRACE_FILE, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600) if TRACE_FILE else None


def repository_path(value: object) -> str | None:
    if not isinstance(value, (str, bytes, os.PathLike)):
        return None
    try:
        path = Path(os.fsdecode(value))
        absolute = (Path.cwd() / path).resolve() if not path.is_absolute() else path.resolve()
    except (OSError, ValueError):
        return None
    if any(root in (absolute, *absolute.parents) for root in TRACE_ROOTS):
        return str(absolute)
    return None


def write(record: dict[str, object]) -> None:
    if TRACE_FD is None:
        return
    record["pid"] = os.getpid()
    record["ppid"] = os.getppid()
    os.write(TRACE_FD, json.dumps(record, sort_keys=True, separators=(",", ":")).encode() + b"\n")


def is_read_mode(mode: object) -> bool:
    if isinstance(mode, str):
        return "r" in mode or "+" in mode or not any(token in mode for token in "wax")
    if isinstance(mode, int):
        return mode & os.O_ACCMODE != os.O_WRONLY
    return True


def audit(event: str, arguments: tuple[object, ...]) -> None:
    if event == "open" and arguments and is_read_mode(
        arguments[2] if len(arguments) > 2 else arguments[1] if len(arguments) > 1 else None
    ):
        path = repository_path(arguments[0])
        if path:
            write({"event": "read", "path": path, "provenance": "python-audit-open"})
    elif event == "import" and len(arguments) > 1:
        path = repository_path(arguments[1])
        if path:
            write({"event": "read", "path": path, "provenance": "python-audit-import"})
    elif event == "subprocess.Popen":
        executable = str(arguments[0]) if arguments else ""
        argv = arguments[1] if len(arguments) > 1 else []
        write({"event": "subprocess", "executable": executable, "argv": list(argv) if isinstance(argv, (list, tuple)) else [str(argv)]})


if TRACE_FD is not None:
    sys.addaudithook(audit)
