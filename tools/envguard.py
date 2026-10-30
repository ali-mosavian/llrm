"""An A/B whose two runs differ in a switch nobody named is not a comparison: refuse it.

`LLRM_CHECK_*` oracles and `LLRM_EXP_*` experiment switches change what a run does (llvm-order-poc blamed a callee-first order for a failure that came from
CHECK flags on in one run only). A tool that times or counts two runs takes the environment it was given, lists the switches of those two families in it,
and refuses unless every one is on its allow list: the switch under test, named on the command line, set alike for both sides.
"""

from __future__ import annotations

import os

FAMILIES = ("LLRM_CHECK_", "LLRM_EXP_", "LLRM_NO_")


def stray(env: dict | None = None, allow: tuple[str, ...] | list[str] = ()) -> list[str]:
    """The `NAME=value` of every CHECK/EXP switch in `env` that `allow` does not name."""
    env = os.environ if env is None else env
    return sorted(f"{k}={v}" for k, v in env.items() if k.startswith(FAMILIES) and k not in allow)


def require_clean(who: str, env: dict | None = None, allow: tuple[str, ...] | list[str] = ()) -> None:
    """SystemExit naming the switches, and how to allow the one under test."""
    found = stray(env, allow)
    if found:
        raise SystemExit(f"{who}: refusing to run with {', '.join(found)} set: unset it, or name it with --allow-env if it is the switch under test (the same value on both sides)")
