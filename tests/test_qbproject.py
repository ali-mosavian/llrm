import json
import runpy
import hashlib
import subprocess
from typing import Any
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


def _namespace() -> dict[str, Any]:
    return runpy.run_path(ROOT / "tools/qbproject.py")


def _project_root(tmp_path: Path, names: tuple[str, ...] = ("alpha.bas", "beta.bas")) -> Path:
    root = tmp_path / "source"
    root.mkdir()
    for name in names:
        (root / name).write_text(f'print "{name}"\r\n')
    return root


def _options(namespace: dict[str, Any]) -> object:
    return namespace["Options"]("vbdos", "vbdos", "column-major")


def _stub_emission(monkeypatch: pytest.MonkeyPatch, namespace: dict[str, Any]) -> list[str]:
    globals_ = namespace["emit_project"].__globals__
    parsed: list[str] = []

    def build_release() -> Path:
        return Path("/tool/qbfront")

    def parse(source: Path, **_options: object) -> str:
        parsed.append(source.name)
        return source.name

    def object_bytes(program: str, _source: str) -> bytes:
        return f"object:{program}".encode()

    monkeypatch.setitem(globals_, "_git_revision", lambda _root: "revision")
    monkeypatch.setitem(globals_, "_assert_clean_inputs", lambda _root, _paths: None)
    monkeypatch.setitem(globals_, "emitter_sha256", lambda: "emitter")
    monkeypatch.setattr(globals_["driver"], "build_release", build_release)
    monkeypatch.setattr(globals_["driver"], "parsed", parse)
    monkeypatch.setattr(globals_["qb_compile"], "object_bytes", object_bytes)
    return parsed


def test_emit_project_orders_inputs_and_records_hashes(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    namespace = _namespace()
    parsed = _stub_emission(monkeypatch, namespace)
    root = _project_root(tmp_path, ("ALPHA.BAS", "beta.bas"))
    (root / "includes-a").mkdir()
    (root / "includes-a" / "shared.bi").write_text("declare sub shared\r\n")
    (root / "includes-b").mkdir()
    output = tmp_path / "output"
    monkeypatch.setenv("QBOPT_QBFRONT", "/installed/qbfront")

    manifest = namespace["emit_project"](
        root,
        "revision",
        output,
        (Path("beta.bas"), Path("ALPHA.BAS")),
        (Path("includes-b"), Path("includes-a")),
        _options(namespace),
    )

    assert parsed == ["ALPHA.BAS", "beta.bas"]
    assert [item["path"] for item in manifest["modules"]] == ["ALPHA.BAS", "beta.bas"]
    assert manifest["include_dirs"] == ["includes-b", "includes-a"]
    assert manifest["include_files"] == [
        {"path": "includes-a/shared.bi", "sha256": hashlib.sha256(b"declare sub shared\r\n").hexdigest()}
    ]
    assert manifest["options"] == {
        "alternate_math": False,
        "array_order": "column-major",
        "checked_arrays": False,
        "dialect": "vbdos",
        "huge_arrays": False,
        "mbf": False,
        "runtime": "vbdos",
    }
    assert manifest["outputs"] == [
        {"path": "alpha.obj", "sha256": hashlib.sha256(b"object:ALPHA.BAS").hexdigest(), "size": 16},
        {"path": "beta.obj", "sha256": hashlib.sha256(b"object:beta.bas").hexdigest(), "size": 15},
    ]
    assert json.loads((output / "frontend-manifest.json").read_text()) == manifest
    assert __import__("os").environ["QBOPT_QBFRONT"] == "/installed/qbfront"


def test_emit_project_rejects_colliding_dos_object_names(tmp_path: Path) -> None:
    namespace = _namespace()
    root = _project_root(tmp_path, ("FOO.bas", "foo.bas"))
    output = tmp_path / "output"

    with pytest.raises(ValueError, match="collide"):
        namespace["emit_project"](root, "revision", output, (Path("FOO.bas"), Path("foo.bas")), (), _options(namespace))

    assert not output.exists()


def test_emit_project_rejects_paths_escaping_source_root(tmp_path: Path) -> None:
    namespace = _namespace()
    root = _project_root(tmp_path)
    outside = tmp_path / "outside.bas"
    outside.write_text('print "outside"\r\n')
    outside_include = tmp_path / "outside-include"
    outside_include.mkdir()

    with pytest.raises(ValueError, match="escapes"):
        namespace["emit_project"](
            root, "revision", tmp_path / "output", (Path("../outside.bas"),), (), _options(namespace)
        )
    with pytest.raises(ValueError, match="escapes"):
        namespace["emit_project"](
            root,
            "revision",
            tmp_path / "output",
            (Path("alpha.bas"),),
            (Path("../outside-include"),),
            _options(namespace),
        )


def test_emit_project_rejects_a_mismatched_source_revision(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    namespace = _namespace()
    globals_ = namespace["emit_project"].__globals__
    monkeypatch.setitem(globals_, "_git_revision", lambda _root: "actual")
    root = _project_root(tmp_path)
    output = tmp_path / "output"

    with pytest.raises(ValueError, match="expected"):
        namespace["emit_project"](root, "revision", output, (Path("alpha.bas"),), (), _options(namespace))

    assert not output.exists()


def test_emit_project_rejects_dirty_requested_inputs(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    namespace = _namespace()
    globals_ = namespace["emit_project"].__globals__

    def git_status(*args: object, **kwargs: object) -> subprocess.CompletedProcess[str]:
        return subprocess.CompletedProcess(args, 0, stdout=" M alpha.bas\n", stderr="")

    monkeypatch.setitem(globals_, "_git_revision", lambda _root: "revision")
    monkeypatch.setattr(globals_["subprocess"], "run", git_status)
    root = _project_root(tmp_path)
    output = tmp_path / "output"

    with pytest.raises(ValueError, match="modified"):
        namespace["emit_project"](root, "revision", output, (Path("alpha.bas"),), (), _options(namespace))

    assert not output.exists()


def test_emit_project_removes_partial_outputs_and_never_reuses_stale_ones(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    namespace = _namespace()
    parsed = _stub_emission(monkeypatch, namespace)
    globals_ = namespace["emit_project"].__globals__
    root = _project_root(tmp_path)
    output = tmp_path / "output"

    def parse(source: Path, **_options: object) -> str:
        parsed.append(source.name)
        if source.name == "beta.bas":
            raise ValueError("second source is invalid")
        return source.name

    monkeypatch.setattr(globals_["driver"], "parsed", parse)

    with pytest.raises(ValueError, match="invalid"):
        namespace["emit_project"](
            root,
            "revision",
            output,
            (Path("alpha.bas"), Path("beta.bas")),
            (),
            _options(namespace),
        )

    assert parsed == ["alpha.bas", "beta.bas"]
    assert not (output / "alpha.obj").exists()
    assert not (output / "frontend-manifest.json").exists()

    (output / "old.obj").write_bytes(b"stale")
    with pytest.raises(ValueError, match="absent or empty"):
        namespace["emit_project"](root, "revision", output, (Path("alpha.bas"),), (), _options(namespace))
    assert (output / "old.obj").read_bytes() == b"stale"
