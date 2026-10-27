"""A pytest plugin: a test skipped for want of QCport is counted and named, and on a host that has QCport it fails the run.

A skip that says "QCport is not on this host" is true on a laptop and a lie on the landing host: the test's own look for QCport (an unset QCPORT,
a moved directory) can hide a blind layer, as /dev/null hid the encoder from the sweep. QCport is on a host when its source directory is
(LLRM_QCPORT_DIR, else ~/scratch/qcport/src)."""
import os
from pathlib import Path

import pytest

skipped: list[str] = []


def qcport_here() -> bool:
    return Path(os.environ.get("LLRM_QCPORT_DIR") or Path.home() / "scratch/qcport/src").expanduser().is_dir()


def pytest_runtest_logreport(report):
    if report.skipped and "qcport" in str(report.longrepr).lower():
        skipped.append(report.nodeid)


def pytest_terminal_summary(terminalreporter):
    terminalreporter.write_line(f"QCPORT SKIPPED: {len(skipped)}" + (f" ({'; '.join(skipped[:5])}{' ...' if len(skipped) > 5 else ''})" if skipped else ""))
    if skipped and qcport_here():
        terminalreporter.write_line("QCPORT SKIPPED ON A HOST THAT HAS QCPORT: a blind layer", red=True)


@pytest.hookimpl(trylast=True)
def pytest_sessionfinish(session, exitstatus):
    if skipped and qcport_here() and session.exitstatus == 0:
        session.exitstatus = pytest.ExitCode.TESTS_FAILED
