import os
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import llvmpin  # noqa: E402


def fake(path: Path, line: str) -> str:
    path.write_text(f'#!/bin/sh\necho "{line}"\n')
    path.chmod(0o755)
    return str(path)


@pytest.fixture
def world(tmp_path, monkeypatch):
    """A pinned bin dir with a clang 24, and a distro clang 20.1.8 first on PATH."""
    pinned = tmp_path / "pinned"
    distro = tmp_path / "distro"
    pinned.mkdir()
    distro.mkdir()
    fake(pinned / "clang", "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc)")
    fake(distro / "clang", "Ubuntu clang version 20.1.8 (0ubuntu4)")
    monkeypatch.setattr(llvmpin, "PINNED_BIN", pinned)
    monkeypatch.setenv("PATH", f"{distro}{os.pathsep}{os.environ['PATH']}")
    return pinned, distro


def test_the_default_is_the_pinned_tool_even_with_a_distro_clang_first_on_path(world):
    """compile-gap and the per-pass table ran `clang` from PATH: 20.1.8, 555 M where the pinned 24.0.0git costs 415 M, and nothing said so."""
    pinned, _ = world
    assert llvmpin.resolve("clang") == str(pinned / "clang")
    assert "24.0.0git" in llvmpin.header(llvmpin.resolve("clang"))


def test_a_distro_tool_is_refused_by_name_unless_the_override_flag_is_given(world):
    pinned, distro = world
    with pytest.raises(SystemExit, match="20.1.8.*not the pinned.*24.0.0git"):
        llvmpin.resolve("clang", str(distro / "clang"))
    assert llvmpin.resolve("clang", str(distro / "clang"), allow_other=True) == str(distro / "clang")
    assert llvmpin.resolve("clang", str(pinned / "clang")) == str(pinned / "clang")


def test_without_the_pinned_build_nothing_falls_back_to_path(world, monkeypatch):
    pinned, _ = world
    (pinned / "clang").unlink()
    with pytest.raises(SystemExit, match="not built here"):
        llvmpin.resolve("clang")


def test_the_tools_that_set_llrm_beside_llvm_take_it_from_llvmpin():
    root = Path(__file__).parent
    for name in ("compile-gap.py", "compile_map.py", "clang_pass_budgets.py"):
        assert "llvmpin" in (root / name).read_text(), name


def test_a_gcc_that_is_not_the_reference_version_is_refused_unless_named(tmp_path):
    """The gcc column is a reference too: 13.4.0, printed, and refused when another gcc is first on PATH."""
    old = fake(tmp_path / "gcc-old", "gcc (Debian 12.2.0) 12.2.0")
    good = fake(tmp_path / "gcc-new", "gcc (Ubuntu 13.4.0-4ubuntu1) 13.4.0")
    with pytest.raises(SystemExit, match="12.2.0.*pinned gcc 13.4.0"):
        llvmpin.gcc(old)
    assert llvmpin.gcc(old, allow_other=True) == old
    assert llvmpin.gcc(good) == good


def test_the_vsgcc_build_takes_both_references_from_the_pin():
    build = (Path(__file__).parent.parent / "crates/target/llrm-x86-m32/vsgcc/build.sh").read_text()
    assert 'llvmpin.py" path clang' in build and 'llvmpin.py" path gcc' in build and "\n  clang $F" not in build


@pytest.fixture
def absent(monkeypatch):
    monkeypatch.setattr(llvmpin, "missing", lambda: "the pinned clang is not built here")
    monkeypatch.delenv("RUNNER_ENVIRONMENT", raising=False)


def test_a_test_needing_the_pin_skips_with_the_line_where_the_pin_is_absent_on_a_hosted_job(absent, monkeypatch):
    """A hosted runner has gcc 13.3 and no pinned clang: the python group failed on it (#2137). The skip says what it is, so it counts and reads as no pass."""
    import pinned_tests

    monkeypatch.setenv("RUNNER_ENVIRONMENT", "github-hosted")
    with pytest.raises(pytest.skip.Exception, match="SKIPPED: pinned toolchain absent"):
        pinned_tests.require_pin()


def test_a_test_needing_the_pin_fails_on_a_self_hosted_job_that_lacks_it(absent, monkeypatch):
    """The jobs that have the pin must run these tests: a skip there would pass a job with no toolchain."""
    import pinned_tests

    monkeypatch.setenv("RUNNER_ENVIRONMENT", "self-hosted")
    with pytest.raises(pytest.fail.Exception, match="must be present"):
        pinned_tests.require_pin()


def test_a_test_needing_the_pin_runs_where_the_pin_is_present(monkeypatch):
    import pinned_tests

    monkeypatch.setattr(llvmpin, "missing", lambda: None)
    monkeypatch.setenv("RUNNER_ENVIRONMENT", "self-hosted")
    assert pinned_tests.require_pin() is None
