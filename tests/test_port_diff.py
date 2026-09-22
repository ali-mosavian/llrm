"""The port's instrument: a stage diff that misses a divergence proves nothing."""

import os
import sys
import subprocess
from pathlib import Path

import pytest

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


QB = {
    "00-input.bas": "PRINT 1\n",
    "01-hir.json": "{}\n",
    "01-__main-02-mir.txt": "m\n",
    "01-__main-18-inline-x87.txt": "x\n",
    "99-emitted-asm.asm": "a\n",
}


def test_a_qb_dump_diverges_first_at_its_hir_although_it_sorts_after_the_mir(tmp_path: Path) -> None:
    """QB dumps name no pipeline folder: ranking them by C's folders raised ValueError on 00-input.bas."""
    python = _dump(tmp_path / "python", QB)
    rust = _dump(tmp_path / "rust", {**QB, "01-hir.json": "[]\n", "01-__main-02-mir.txt": "n\n"})
    matched, total, first = port_diff.compare(python, rust, flat=True)
    assert (matched, total) == (3, 5)
    assert first == port_diff.Divergence("01-hir.json", 1, "{}", "[]")


def test_the_python_oracle_is_this_checkouts_qbopt(tmp_path: Path) -> None:
    """A script run as tools/qbstages.py imported the main checkout's qbopt, so QB diffs ran a stale compiler."""
    script = tmp_path / "where.py"
    script.write_text("import qbopt\nprint(qbopt.__file__)\n")
    found = subprocess.run(
        [sys.executable, str(script)], env=port_diff.oracle_env(), capture_output=True, text=True, check=True
    )
    assert Path(found.stdout.strip()).parent.parent == port_diff.ROOT


def test_a_sources_flags_reach_every_compiler(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """sum_three --unchecked-bounds was never diffed: --qb compiled every source at default flags."""
    source = tmp_path / "ONE.BAS"
    source.write_text("PRINT 1\n")
    source.with_suffix(".flags").write_text("--dialect\nqb45\n--unchecked-bounds\n")
    commands = []

    def run(command: list[str], **_: object) -> subprocess.CompletedProcess:
        commands.append(command)
        return subprocess.CompletedProcess(command, 0, "", "")

    monkeypatch.setattr(port_diff.subprocess, "run", run)
    port_diff.run_qb(source, tmp_path / "work", Path("llrm-qb"))
    assert len(commands) == 3
    assert all(command[-3:] == ["--dialect", "qb45", "--unchecked-bounds"] for command in commands)


def test_a_modern_source_reaches_both_compilers_through_one_frontend(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """--modern did not exist: the modern compile path had no instrument at all."""
    source = tmp_path / "one.mod"
    source.write_text("fn main() -> i16:\n    return 0\n")
    source.with_suffix(".flags").write_text("--entry\nmain\n-O\ns\n")
    commands, frontends = [], []

    def run(command: list[str], env: dict[str, str], **_: object) -> subprocess.CompletedProcess:
        commands.append(command)
        frontends.append(env["QBOPT_MODERNFRONT"])
        return subprocess.CompletedProcess(command, 0, "", "")

    monkeypatch.setattr(port_diff.subprocess, "run", run)
    port_diff.run_modern(source, tmp_path / "work", Path("llrm-modern"), Path("modernfront"))
    stages, python, rust = commands
    assert stages[1].endswith("tools/modernstages.py") and stages[-2:] == ["-O", "s"]
    assert python[-4:] == rust[-4:] == ["--entry", "main", "-O", "s"]
    assert frontends == ["modernfront"] * 3


def test_the_recorded_corpus_keeps_each_compiles_flags(tmp_path: Path) -> None:
    """A parity source at default flags is already diffed; the same source at other flags is not."""
    import qb_port_corpus

    parity = port_diff.ROOT / "bench/parity/sum_three.bas"
    defaults = ("--dialect", "vbdos")
    recorder = qb_port_corpus.Recorder()
    recorder.record(parity, defaults, defaults)
    recorder.record(parity, (*defaults, "--unchecked-bounds"), defaults)
    qb_port_corpus.write(recorder.compiled, tmp_path)

    [written] = tmp_path.glob("*/*.bas")
    assert written.read_bytes() == parity.read_bytes()
    assert port_diff.qb_flags(written) == [*defaults, "--unchecked-bounds"]


def test_the_select_sweep_is_what_this_checkouts_select_emits() -> None:
    """Run as a script, the sweep read the main checkout's select: Rust's `rol ax,ecx` failed a stale expectation."""
    import select_sweep

    assert select_sweep.OUT.read_text() == select_sweep.rendered(select_sweep.sweep())
