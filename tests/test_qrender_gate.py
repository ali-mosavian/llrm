"""Focused provenance guards for the deliberately narrow qrender gate."""

from __future__ import annotations

import runpy
import hashlib
from typing import Any
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


def _gate() -> dict[str, Any]:
    return runpy.run_path(ROOT / "tools/qrender_gate.py")


def _project_root(tmp_path: Path) -> Path:
    root = tmp_path / "qrender"
    for directory in ("src/host", "src/render", "src/qgl"):
        (root / directory).mkdir(parents=True, exist_ok=True)
    (root / "tools").mkdir()
    (root / "Makefile").write_text("SRC_DIRS := src/host src/render src/qgl\n")
    for name in ("main", "common", "h_bench", "qglstub", "d_mdl", "qglchk", "qgldiff", "qglarr", "qglface"):
        directory = (
            "host" if name in {"main", "common", "h_bench", "qglstub"} else "render" if name == "d_mdl" else "qgl"
        )
        (root / "src" / directory / f"{name}.bas").write_text(f'print "{name}"\n')
    (root / "src/host/shared.bi").write_text("declare sub shared\n")
    for name in ("bc.sh", "bcc-qr.sh", "link-qr.sh", "dosbox.sh"):
        (root / "tools" / name).write_text("#!/bin/sh\n")
    return root


def _object(path: Path, data: bytes) -> str:
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def test_discover_uses_current_makefile_layout_and_qgl_split(tmp_path: Path) -> None:
    namespace = _gate()
    project = namespace["discover"](_project_root(tmp_path))

    assert project.production == ("main", "common", "h_bench", "qglstub", "d_mdl")
    assert project.oracle == ("main", "common", "h_bench", "d_mdl", "qglarr", "qglchk", "qgldiff", "qglface")
    assert [path.as_posix().split("/")[-1] for path in project.include_dirs] == ["host"]
    assert all(not path.is_absolute() for path in (*project.modules, *project.include_dirs))


def test_fresh_basic_rejects_stale_or_missing_module_before_link(tmp_path: Path) -> None:
    namespace = _gate()
    emitted, build = tmp_path / "frontend", tmp_path / "candidate"
    emitted.mkdir()
    build.mkdir()
    source = emitted / "main.obj"
    _object(source, b"fresh")
    _object(build / "main.obj", b"stale")

    with pytest.raises(ValueError, match="stale or missing"):
        namespace["_assert_fresh_basic"](build, ("main",), {"main": source})

    (build / "main.obj").unlink()
    with pytest.raises(ValueError, match="missing required artifact"):
        namespace["_assert_fresh_basic"](build, ("main",), {"main": source})


def test_oracle_seed_copies_the_fresh_production_artifacts_with_mtimes(tmp_path: Path) -> None:
    namespace = _gate()
    baseline, oracle = tmp_path / "baseline", tmp_path / "oracle"
    baseline.mkdir()
    object_path = baseline / "RENDER.OBJ"
    _object(object_path, b"non-basic")
    __import__("os").utime(object_path, ns=(1_000_000_000, 1_000_000_000))
    manifest = namespace["_nonbasic_manifest"](baseline, ("render",))

    seed = namespace["_seed_oracle"](baseline, oracle, manifest)
    assert seed["preserve_mtimes"] is True
    assert (oracle / "RENDER.OBJ").stat().st_mtime_ns == object_path.stat().st_mtime_ns


def test_response_coverage_requires_main_first_once_and_excludes_oracles(tmp_path: Path) -> None:
    namespace = _gate()
    response = tmp_path / "link.rsp"
    response.write_text("/NOE main.obj+common.obj+\nqglstub.obj\nqrender.exe\n")
    namespace["assert_response_coverage"](response, ("main", "common", "qglstub"), {"qglchk"})

    response.write_text("/NOE common.obj+main.obj+qglstub.obj+qglchk.obj\n")
    with pytest.raises(ValueError, match="main first"):
        namespace["assert_response_coverage"](response, ("main", "common", "qglstub"), {"qglchk"})

    response.write_text("/NOE main.obj+common.obj+qglstub.obj+qglchk.obj\n")
    with pytest.raises(ValueError, match="coverage differs"):
        namespace["assert_response_coverage"](response, ("main", "common", "qglstub"), {"qglchk"})


def test_baseline_response_is_the_authoritative_link_order(tmp_path: Path) -> None:
    namespace = _gate()
    response = tmp_path / "link.rsp"
    response.write_text("/NOE main.obj+qglstub.obj+common.obj+RENDER.OBJ\nqrender.exe\n")

    assert namespace["basic_order_from_response"](response, {"main", "common", "qglstub", "qglchk"}, {"qglchk"}) == (
        "main",
        "qglstub",
        "common",
    )


def test_runtime_clears_stale_artifacts_and_rejects_missing_fresh_completion(tmp_path: Path) -> None:
    namespace = _gate()
    build, source = tmp_path / "candidate", tmp_path / "source"
    build.mkdir()
    source.mkdir()
    (build / "RAN.TXT").write_text("DONE\n")
    (build / "BENCH.BMP").write_bytes(b"x" * 1001)
    (build / "BENCH.TXT").write_text("ticks 60\nsc_test 1\n")

    def does_not_run(*_args: object) -> object:
        return namespace["Process"](0)

    options = namespace["Options"](source, "0" * 40, tmp_path / "out", timeout=1)
    with pytest.raises(ValueError, match="fresh RAN.TXT DONE"):
        namespace["_run_bench"](does_not_run, source, tmp_path, build, options)
    assert not (build / "RAN.TXT").exists()
    assert not (build / "BENCH.BMP").exists()


def test_qglface_is_recorded_as_known_failure_not_a_pass(tmp_path: Path) -> None:
    namespace = _gate()
    build, source = tmp_path / "oracle", tmp_path / "source"
    build.mkdir()
    source.mkdir()

    def writes_face(_command: object, _cwd: object, _env: object, _timeout: object) -> object:
        (build / "QGLFACE.LOG").write_text("coverage 1756\r\nRESULT FAIL\r\n", encoding="latin1")
        (build / "RAN.TXT").write_text("DONE\r\n", encoding="latin1")
        return namespace["Process"](0)

    options = namespace["Options"](source, "0" * 40, tmp_path / "out", timeout=1)
    record = namespace["_run_oracle"](writes_face, source, tmp_path, build, "qglface", options, require_pass=False)
    assert record["status"] == "known-failure"
    assert record["final"] == "RESULT FAIL"


def test_oracle_pass_flags_match_the_current_qrender_gate(tmp_path: Path) -> None:
    namespace = _gate()
    build, source = tmp_path / "oracle", tmp_path / "source"
    build.mkdir()
    source.mkdir()
    observed: list[str] = []

    def writes_pass(_command: object, _cwd: object, environment: dict[str, str], _timeout: object) -> object:
        observed.append(environment["QFLAGS"])
        (build / "QGLARR.LOG").write_text("RESULT PASS\r\n", encoding="latin1")
        (build / "RAN.TXT").write_text("DONE\r\n", encoding="latin1")
        return namespace["Process"](0)

    options = namespace["Options"](source, "0" * 40, tmp_path / "out", timeout=1)
    namespace["_run_oracle"](writes_pass, source, tmp_path, build, "qglarr", options, require_pass=True)
    assert observed == ["-qglarr"]


def test_benchmark_invariants_reject_an_implausible_pt_mean(tmp_path: Path) -> None:
    namespace = _gate()
    bench = tmp_path / "BENCH.TXT"
    bench.write_text("ticks 60\nsc_test 1\nfp_sites 3\nmdl_bf_bad 0\npt_draw 1 2 3\npt_q_n 1 8 9\npt_q_noclip 1 7 8\n")
    assert namespace["_validate_bench"](bench)["ticks"] == "60"

    bench.write_text("ticks 60\nsc_test 1\nfp_sites 3\nmdl_bf_bad 0\npt_draw 1 4 3\n")
    with pytest.raises(ValueError, match="outside min..max"):
        namespace["_validate_bench"](bench)

    bench.write_text("ticks 60\nsc_test 1\nfp_sites nan\nmdl_bf_bad 0\npt_draw 1 2 3\n")
    with pytest.raises(ValueError, match="not finite"):
        namespace["_validate_bench"](bench)


def test_run_keeps_failed_phase_and_error_in_partial_receipt(tmp_path: Path) -> None:
    namespace = _gate()
    source, output = _project_root(tmp_path), tmp_path / "receipt"
    revision = "0" * 40

    def fails_after_fresh_source(command: tuple[str, ...], _cwd: Path, _env: object, _timeout: int) -> object:
        if command[0] == "git":
            value = "" if "status" in command else revision
            return namespace["Process"](0, value + "\n")
        assert command[0] == "make"
        return namespace["Process"](0)

    with pytest.raises(ValueError, match="missing required artifact"):
        namespace["run"](namespace["Options"](source, revision, output, timeout=1), runner=fails_after_fresh_source)

    receipt = __import__("json").loads((output / "qrender-gate.json").read_text())
    assert receipt["status"] == "failed"
    assert receipt["phase"] == "baseline"
    assert "missing required artifact" in receipt["error"]


def test_run_passes_source_relative_inputs_to_the_project_emitter(tmp_path: Path) -> None:
    namespace = _gate()
    source, output = _project_root(tmp_path), tmp_path / "receipt"
    revision = "0" * 40
    observed: dict[str, object] = {}

    def build_then_stop(command: tuple[str, ...], _cwd: Path, _env: object, _timeout: int) -> object:
        if command[0] == "git":
            return namespace["Process"](0, ("" if "status" in command else revision) + "\n")
        build = Path(next(item.removeprefix("BUILD=") for item in command if item.startswith("BUILD=")))
        (build / "link.out").write_text("linked\n")
        (build / "qrender.exe").write_bytes(b"exe")
        (build / "link.rsp").write_text("/NOE main.obj+common.obj+qglstub.obj+RENDER.OBJ\nqrender.exe\n")
        return namespace["Process"](0)

    def capture_emitter(
        _root: Path,
        _revision: str,
        _output: Path,
        modules: tuple[Path, ...],
        includes: tuple[Path, ...],
        _options: object,
    ) -> dict[str, object]:
        observed["modules"], observed["includes"] = modules, includes
        raise ValueError("stop after emitter inputs")

    with pytest.raises(ValueError, match="stop after emitter inputs"):
        namespace["run"](
            namespace["Options"](source, revision, output, timeout=1), runner=build_then_stop, emitter=capture_emitter
        )
    assert all(not path.is_absolute() for path in observed["modules"])
    assert all(not path.is_absolute() for path in observed["includes"])


def _map(path: Path, *, basic: int, complete_extra: int = 0) -> None:
    path.write_text(
        "0001:00000000 00000000H "
        + f"{basic:08X}H MAIN_CODE BC_CODE\n"
        + "0002:00000000 00000000H "
        + f"{complete_extra:08X}H RUNTIME_CODE CODE\n"
    )


def test_quality_refuses_generated_code_regression(tmp_path: Path) -> None:
    namespace = _gate()
    baseline, candidate = tmp_path / "baseline", tmp_path / "candidate"
    baseline.mkdir()
    candidate.mkdir()
    _map(baseline / "qrender.map", basic=10, complete_extra=5)
    _map(candidate / "qrender.map", basic=11, complete_extra=5)

    with pytest.raises(ValueError, match="footprint regressed"):
        namespace["_quality"](baseline, candidate)
