"""`slotted(command)`: a cargo command under `buildslot` (4 shared slots, nice 10) so builds and tests of many sessions do not pile up on the host.

`BUILDSLOT_BYPASS=1` leaves a command as it is (the integration landing runs at normal priority). The landing itself (parity_squash merge, merge-batch,
head-failures, and the smoke they start) runs inside `landing()`, which sets the same priority for its own commands and the subprocesses they start, so no
caller has to remember a variable: batches of the gate took ~22 minutes, mostly waiting for a slot behind other sessions' builds. A command that is not cargo is not touched. A host without
`buildslot` on PATH or in ~/.local/bin fails loudly: running unslotted by accident is what this exists to stop.
"""
from __future__ import annotations

import contextlib
import os
import re
import shutil
from pathlib import Path

LANDING = "BUILDSLOT_LANDING"

CARGO = re.compile(r"(^|&&\s*|;\s*|\|\|\s*)cargo(\s)")


def tool() -> str:
    found = shutil.which("buildslot") or str(Path.home() / ".local/bin/buildslot")
    if not Path(found).exists():
        raise SystemExit("buildslot is not installed (~/.local/bin/buildslot): refusing to run cargo outside a build slot (BUILDSLOT_BYPASS=1 to skip)")
    return found


@contextlib.contextmanager
def landing():
    """Inside it every `slotted` command (this process's and its subprocesses') runs at the landing's priority: unslotted."""
    before = os.environ.get(LANDING)
    os.environ[LANDING] = "1"
    try:
        yield
    finally:
        if before is None:
            os.environ.pop(LANDING, None)
        else:
            os.environ[LANDING] = before


def landing_cap() -> list[str]:
    """What a landing cargo runs under: cores 0-15 at nice 5 and 8 build jobs (the caller's CARGO_BUILD_JOBS wins), so one batch cannot take all 32 cores (1m load 34 with no admission, 2026-11-06)."""
    jobs = [] if "CARGO_BUILD_JOBS" in os.environ else ["env", "CARGO_BUILD_JOBS=8"]
    return ["taskset", "-c", "0-15", "nice", "-n", "5", *jobs]


def slotted(command):
    """`command` (an argv list or a shell string) with every cargo invocation run by buildslot, unless the landing runs it (capped, not slotted)."""
    if os.environ.get("BUILDSLOT_BYPASS") == "1":
        return command
    if os.environ.get(LANDING) == "1":
        cap = landing_cap()
        if isinstance(command, str):
            return CARGO.sub(lambda m: f"{m[1]}{' '.join(cap)} cargo{m[2]}", command)
        return [*cap, *command] if command and Path(command[0]).name == "cargo" else command
    if isinstance(command, str):
        return CARGO.sub(lambda m: f"{m[1]}{tool()} cargo{m[2]}", command)
    return [tool(), *command] if command and Path(command[0]).name == "cargo" else command
