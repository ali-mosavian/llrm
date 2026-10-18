"""An LLRM_* switch as the compilers read it (crates/support/llrm-env): on unless unset, empty, `0`, `false` or `off`.

`os.environ.get(name)` as a condition made `LLRM_REQUIRE_BINARIES=0` mean on, and `== "1"` made `=true` mean off; the compilers read neither.
"""
from __future__ import annotations

import os

OFF = ("", "0", "false", "off")


def on(value: str | None) -> bool:
    return value is not None and value.lower() not in OFF


def flag(name: str, env: dict | None = None) -> bool:
    return on((os.environ if env is None else env).get(name))
