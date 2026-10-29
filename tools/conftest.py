"""The landing runner's own settings (runner.env) must not reach the tools' tests: the batch ran them under LAND_FAST=1 and BATCH_CAP, which changed what four tests of
parity_squash.py assert, and #5681 was refused at the tool-tests stage for a test that passes in a plain shell."""

import os
import shutil
from pathlib import Path

import pytest

RUNNER_ENV = (
    "HEAD_MIN_AGE",
    "ALONE_JOBS",
    "ALONE_CAP",
    "LAND_SKIP_LINK",
    "LAND_SKIP_ALONE",
    "LAND_SKIP_RESCAN",
    "LAND_FAST",
    "LAND_FAST_TESTS",
    "BATCH_MAX_SECONDS",
    "HEAD_RED_HOURS",
    "OVERLAP_MAX_LOAD",
    "PRIORITY",
    "WAITS",
    "BATCH_CAP",
    "PUSH_MODE",
    "BUILDSLOT_LANDING",  # parity_squash runs the whole batch inside buildslot.landing(); its tool tests inherit it
)

for name in RUNNER_ENV:
    os.environ.pop(name, None)


def _stub(directory, name, body):
    path = directory / name
    path.write_text("#!/bin/sh\n" + body)
    path.chmod(0o755)


@pytest.fixture(autouse=True)
def _host_tools(tmp_path_factory, monkeypatch):
    """A host without buildslot or rustc (the hosted gate image) failed 5 xfail and 4 buildslot tests: stubs stand in for the ones the tests only need to find."""
    stubs = tmp_path_factory.mktemp("stubs")
    if not (shutil.which("buildslot") or (Path.home() / ".local/bin/buildslot").exists()):
        _stub(stubs, "buildslot", 'exec "$@"\n')
    if not shutil.which("rustc"):
        _stub(stubs, "rustc", "echo 'host: x86_64-unknown-linux-gnu'\n")
    monkeypatch.setenv("PATH", f"{stubs}{os.pathsep}{os.environ['PATH']}")
