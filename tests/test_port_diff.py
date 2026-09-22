"""The port's instrument: a stage diff that misses a divergence proves nothing."""

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))

import port_diff  # noqa: E402


def _dump(root: Path, files: dict[str, str]) -> Path:
    for at, (name, text) in enumerate(files.items()):
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        os.utime(path, ns=(at, at))
    return root


PYTHON = {
    "stream": "s\n",
    "hir": "h\n",
    "phases/f.00-FarIndirectCalls": "p0\n",
    "mir": "m {frozenset({Slice(a=1), Slice(a=2)})}\n",
    "asm": "a\n",
}


def test_identical_dumps_match_every_stage(tmp_path: Path) -> None:
    python = _dump(tmp_path / "python", PYTHON)
    rust = _dump(tmp_path / "rust", PYTHON)
    assert port_diff.compare(python, rust) == (5, 5, None)


def test_a_perturbed_stage_is_the_first_divergence(tmp_path: Path) -> None:
    python = _dump(tmp_path / "python", PYTHON)
    rust = _dump(tmp_path / "rust", {**PYTHON, "phases/f.00-FarIndirectCalls": "p1\n", "asm": "b\n"})
    matched, total, first = port_diff.compare(python, rust)
    assert (matched, total) == (3, 5)
    assert first == port_diff.Divergence("phases/f.00-FarIndirectCalls", 1, "p0", "p1")


def test_mir_ranks_before_phases_although_python_writes_it_after(tmp_path: Path) -> None:
    python = _dump(tmp_path / "python", PYTHON)
    rust = _dump(tmp_path / "rust", {**PYTHON, "mir": "x\n", "phases/f.00-FarIndirectCalls": "p1\n"})
    assert port_diff.compare(python, rust)[2].stage == "mir"


def test_a_missing_rust_stage_diverges(tmp_path: Path) -> None:
    python = _dump(tmp_path / "python", PYTHON)
    rust = _dump(tmp_path / "rust", {"stream": "s\n", "hir": "h\n"})
    assert port_diff.compare(python, rust)[2] == port_diff.Divergence("mir", 0, "", "missing")


def test_frozenset_order_is_not_a_divergence(tmp_path: Path) -> None:
    python = _dump(tmp_path / "python", PYTHON)
    rust = _dump(tmp_path / "rust", {**PYTHON, "mir": "m {frozenset({Slice(a=2), Slice(a=1)})}\n"})
    assert port_diff.compare(python, rust)[2] is None
    assert (
        port_diff.normalized("frozenset({'b', frozenset({2, 1}), 'a'})") == "frozenset({'a', 'b', frozenset({1, 2})})"
    )


def test_a_relative_source_is_named_from_the_repository(monkeypatch) -> None:
    """A relative path crashed the report: it was resolved to test, not to relativize."""
    monkeypatch.chdir(port_diff.ROOT / "fixtures")
    assert port_diff._display(Path("c/parity/scalar.cgs")) == Path("fixtures/c/parity/scalar.cgs")


def test_sources_sharing_a_stem_get_their_own_dumps(tmp_path: Path) -> None:
    """c/control.cgs and c/parity/control.cgs shared one work folder, each overwriting the other."""
    one = port_diff._workdir(tmp_path, port_diff.ROOT / "fixtures/c/control.cgs")
    other = port_diff._workdir(tmp_path, port_diff.ROOT / "fixtures/c/parity/control.cgs")
    assert one != other
