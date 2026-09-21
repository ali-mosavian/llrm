import json
import subprocess
from pathlib import Path
from typing import Never

import pytest
import conftest

from qbopt.frontend.qb import driver as qb_driver


def test_pytest_frontend_setup_builds_once_and_configures_producer(monkeypatch: pytest.MonkeyPatch) -> None:
    # The old per-parse cargo run made the 458-test loop take 84 seconds.
    executable = qb_driver.ROOT / "temporary-qbfront"
    builds: list[None] = []

    def build_release() -> Path:
        builds.append(None)
        return executable

    monkeypatch.delenv("QBOPT_QBFRONT", raising=False)
    monkeypatch.setattr(qb_driver, "build_release", build_release)

    assert conftest.configure_qb_frontend() == executable
    assert conftest.configure_qb_frontend() is None
    assert builds == [None]
    assert qb_driver.command() == (str(executable),)


def test_pytest_frontend_setup_preserves_an_explicit_producer(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("QBOPT_QBFRONT", "/tools/qbfront")

    def unexpected_build() -> Never:
        raise AssertionError("explicit QBOPT_QBFRONT must avoid an in-tree build")

    monkeypatch.setattr(qb_driver, "build_release", unexpected_build)

    assert conftest.configure_qb_frontend() is None
    assert qb_driver.command() == ("/tools/qbfront",)


def test_build_release_uses_cargos_qbfront_artifact(monkeypatch: pytest.MonkeyPatch) -> None:
    executable = qb_driver.ROOT / "target" / "release" / "qbfront"
    reports = (
        {"reason": "compiler-artifact", "target": {"name": "another-bin", "kind": ["bin"]}, "executable": "/bad"},
        {"reason": "compiler-artifact", "target": {"name": "qbfront", "kind": ["lib"]}, "executable": None},
        {"reason": "compiler-artifact", "target": {"name": "qbfront", "kind": ["bin"]}, "executable": str(executable)},
    )

    def run(*args: object, **kwargs: object) -> subprocess.CompletedProcess[str]:
        return subprocess.CompletedProcess(
            args, 0, stdout="\n".join(json.dumps(report) for report in reports), stderr=""
        )

    monkeypatch.setattr(qb_driver.subprocess, "run", run)

    assert qb_driver.build_release() == executable


def test_build_release_rejects_invalid_cargo_report(monkeypatch: pytest.MonkeyPatch) -> None:
    def run(*args: object, **kwargs: object) -> subprocess.CompletedProcess[str]:
        return subprocess.CompletedProcess(args, 0, stdout="not json", stderr="")

    monkeypatch.setattr(qb_driver.subprocess, "run", run)

    with pytest.raises(qb_driver.FrontendError, match="invalid JSON"):
        qb_driver.build_release()


def test_build_release_rejects_cargo_report_without_qbfront(monkeypatch: pytest.MonkeyPatch) -> None:
    def run(*args: object, **kwargs: object) -> subprocess.CompletedProcess[str]:
        return subprocess.CompletedProcess(args, 0, stdout=json.dumps({"reason": "build-finished"}), stderr="")

    monkeypatch.setattr(qb_driver.subprocess, "run", run)

    with pytest.raises(qb_driver.FrontendError, match="did not report"):
        qb_driver.build_release()


def test_build_release_reports_cargo_start_failure(monkeypatch: pytest.MonkeyPatch) -> None:
    def run(*args: object, **kwargs: object) -> Never:
        raise OSError("cargo is unavailable")

    monkeypatch.setattr(qb_driver.subprocess, "run", run)

    with pytest.raises(qb_driver.FrontendError, match="could not start"):
        qb_driver.build_release()


def test_common_hir_profiles_do_not_become_qb_frontend_options() -> None:
    """Adding modern/freestanding to common HIR once made QB's driver advertise both."""
    source = qb_driver.ROOT / "not-read.bas"
    with pytest.raises(qb_driver.FrontendError, match="unknown QB dialect"):
        qb_driver.syntax_checked(source, dialect="modern")
    with pytest.raises(qb_driver.FrontendError, match="unknown QB runtime"):
        qb_driver.syntax_checked(source, runtime="freestanding")
