"""Every target's datalayout names its stack alignment (`S`), or the passes that raise an alloca to its type's preferred alignment have no
bound (`exceedsNaturalStackAlignment` is false on an unset S). m16 had none: matmul and nbody carried 32-aligned locals in 2-byte frames."""

import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LAYOUTS = sorted((ROOT / "crates" / "target").glob("*/src/machines/datalayout.toml"))
# Targets whose owners have not declared one yet: each must go, the set may not grow.
PENDING = {"llrm-6502", "llrm-z80"}


def target(path: Path) -> str:
    return path.parents[2].name


def stack_bytes(path: Path) -> int | None:
    found = re.search(r"-S(\d+)\b", tomllib.loads(path.read_text())["datalayout"])
    return int(found.group(1)) // 8 if found else None


def test_every_target_declares_its_stack_alignment():
    missing = {target(path) for path in LAYOUTS if stack_bytes(path) is None}
    assert missing - PENDING == set(), f"datalayout without `S`: {sorted(missing - PENDING)}"


def test_a_pending_target_that_declared_one_leaves_the_list():
    declared = {target(path) for path in LAYOUTS if stack_bytes(path) is not None}
    assert PENDING & declared == set(), f"remove from PENDING: {sorted(PENDING & declared)}"


def test_m16_frames_keep_two_bytes():
    assert stack_bytes(ROOT / "crates/target/llrm-x86-m16/src/machines/datalayout.toml") == 2
