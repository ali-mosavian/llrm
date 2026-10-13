"""The slow DOS tier: tests marked `slow_dos` (the demos played, the graphics sweeps) run only when asked for, with
`-m slow_dos` or LLRM_DOS_SLOW=1.  The quick tier (the probes and the bench programs, about a minute) runs on every change."""

import os

import pytest

SLOW = "slow_dos"


def pytest_configure(config):
    config.addinivalue_line("markers", f"{SLOW}: a DOS test that takes minutes (demos played, graphics sweeps); run with -m {SLOW}")


def asked_for(markexpr: str, environ) -> bool:
    """Whether the run named the slow tier: `-m` with the marker in it, or the environment."""
    return SLOW in (markexpr or "") or environ.get("LLRM_DOS_SLOW") == "1"


def pytest_collection_modifyitems(config, items):
    if asked_for(config.getoption("-m", default=""), os.environ):
        return
    skip = pytest.mark.skip(reason=f"slow DOS tier: run with -m {SLOW} (tools/dosbatch/conftest.py)")
    for item in items:
        if SLOW in item.keywords:
            item.add_marker(skip)
