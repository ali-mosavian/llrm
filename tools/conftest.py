"""The landing runner's own settings (runner.env) must not reach the tools' tests: the batch ran them under LAND_FAST=1 and BATCH_CAP, which changed what four tests of
parity_squash.py assert, and #5681 was refused at the tool-tests stage for a test that passes in a plain shell."""

import os

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
