"""Every target's datalayout names its stack alignment (`S`), or the passes that raise an alloca to its type's preferred alignment have no
bound (`exceedsNaturalStackAlignment` is false on an unset S). m16 had none: matmul and nbody carried 32-aligned locals in 2-byte frames."""

import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LAYOUTS = sorted((ROOT / "crates" / "target").glob("*/src/machines/datalayout.toml"))
# Targets whose owners have not declared one yet: each must go, the set may not grow.
PENDING = {"llrm-z80"}
# Crates under crates/target that are not a target's description.
NOT_TARGETS = {"llrm-target", "llrm-x86", "llrm-x86-backend", "llrm-x86-dagisel", "llrm-x86-m16-select", "llrm-x86-m32-select", "llrm-x86-m64-select"}


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


# Each target crate checks S against what its frames keep, and against the pin's string where there is one, with one shared helper.
HELPER = "assert_stack_facts"
# Crates that do not call it yet: the set may not grow (owner and date in the coordinator's list, 2026-11-10 12:00Z).
UNCHECKED = {"llrm-z80", "llrm-m68k", "llrm-arm64", "llrm-wasm", "llrm-x86-m32", "llrm-x86-m64"}


def calls_helper(crate: Path) -> bool:
    return any(HELPER in file.read_text() for file in (crate / "src").rglob("*.rs"))


def test_every_target_crate_calls_the_shared_stack_check():
    crates = {path.parent.name: path.parent for path in (ROOT / "crates" / "target").glob("*/Cargo.toml") if path.parent.name not in NOT_TARGETS}
    missing = {name for name, crate in crates.items() if not calls_helper(crate)}
    assert missing - UNCHECKED == set(), f"no {HELPER} call: {sorted(missing - UNCHECKED)}"
    assert UNCHECKED & (set(crates) - missing) == set(), f"remove from UNCHECKED: {sorted(UNCHECKED & (set(crates) - missing))}"


def test_m16_frames_keep_two_bytes():
    assert stack_bytes(ROOT / "crates/target/llrm-x86-m16/src/machines/datalayout.toml") == 2
