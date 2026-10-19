"""`slotted(command)`: a cargo command under `buildslot` (4 shared slots, nice 10) so builds and tests of many sessions do not pile up on the host.

`BUILDSLOT_BYPASS=1` leaves a command as it is (the integration landing runs at normal priority). A command that is not cargo is not touched. A host without
`buildslot` on PATH or in ~/.local/bin fails loudly: running unslotted by accident is what this exists to stop.
"""
from __future__ import annotations

import os
import re
import shutil
from pathlib import Path

CARGO = re.compile(r"(^|&&\s*|;\s*|\|\|\s*)cargo(\s)")


def tool() -> str:
    found = shutil.which("buildslot") or str(Path.home() / ".local/bin/buildslot")
    if not Path(found).exists():
        raise SystemExit("buildslot is not installed (~/.local/bin/buildslot): refusing to run cargo outside a build slot (BUILDSLOT_BYPASS=1 to skip)")
    return found


def slotted(command):
    """`command` (an argv list or a shell string) with every cargo invocation run by buildslot."""
    if os.environ.get("BUILDSLOT_BYPASS") == "1":
        return command
    if isinstance(command, str):
        return CARGO.sub(lambda m: f"{m[1]}{tool()} cargo{m[2]}", command)
    return [tool(), *command] if command and Path(command[0]).name == "cargo" else command
