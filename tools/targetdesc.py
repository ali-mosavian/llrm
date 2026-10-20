"""The targets' LLVM facts, read from `crates/target/*/src/machines/datalayout.toml`: tools name no triple, CPU or datalayout themselves."""
from __future__ import annotations

import os
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def targets() -> dict[str, dict]:
    """`m<mode>` -> its datalayout.toml (mode, datalayout, llvm_triple, llvm_cpu, tti)."""
    found = (tomllib.loads(path.read_text()) for path in sorted((ROOT / "crates/target").glob("*/src/machines/datalayout.toml")))
    return {f"m{one['mode']}": one for one in found}


def default() -> str:
    """The target the oracles compare at unless LLRM_DIFFTEST_TARGET (or `host`) says otherwise: the widest."""
    all_ = targets()
    return os.environ.get("LLRM_DIFFTEST_TARGET", max(all_, key=lambda name: all_[name]["mode"]))


def triple_cpu(which: str | None = None) -> list[str]:
    """opt's `-mtriple` and `-mcpu` for the target; none for `host`, which restates an old claim at opt's own default."""
    which = which or default()
    if which == "host":
        return []
    one = targets()[which]
    triple, cpu = one["llvm_triple"], one["llvm_cpu"]
    return [f"-mtriple={triple}", f"-mcpu={cpu}"]


def opt_args(which: str | None = None) -> list[str]:
    """`triple_cpu` and `--data-layout`: a dump carries neither, and opt prices for x86-64 without them."""
    which = which or default()
    return [] if which == "host" else [*triple_cpu(which), f"--data-layout={targets()[which]['datalayout']}"]
