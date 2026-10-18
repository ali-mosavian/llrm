"""A timed-only program is run and its output compared in the default bench: sized only, a miscompile reached no gate."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import bench  # noqa: E402

KEY = ("jpeg_single", "c", "O2")
SIZED = {"code_bytes": 10, "data_bytes": 0, "bss_bytes": 0}


def fake(monkeypatch, result):
    ran = []
    monkeypatch.setattr(bench, "time_programs", lambda exes, work, needs: ran.append(sorted(exes)) or {key: result for key in exes})
    return ran


def test_without_time_a_timed_only_program_is_run_and_marked_checked(monkeypatch, tmp_path):
    ran = fake(monkeypatch, {"cycles": 5, "ms": 1.0})
    out = {KEY: dict(SIZED), ("sieve", "c", "O2"): {"instructions": 9}}
    bench.run_timed(out, {KEY: Path("a.exe"), ("sieve", "c", "O2"): Path("b.exe")}, {KEY: {"files": (), "want": ["1"]}}, tmp_path, timing=False)
    assert ran == [[KEY]], "only the timed-only program (the one with an input) runs"
    assert out[KEY] == SIZED | {"checked": True} and "cycles" not in out[KEY], "no times without --time"
    assert out[("sieve", "c", "O2")] == {"instructions": 9}


def test_a_timed_only_program_that_prints_something_else_is_an_error_row(monkeypatch, tmp_path):
    fake(monkeypatch, {"error": "prints ['7'], want ['9']"})
    out = {KEY: dict(SIZED)}
    bench.run_timed(out, {KEY: Path("a.exe")}, {KEY: {"files": (), "want": ["9"]}}, tmp_path, timing=False)
    assert out[KEY] == {"error": "prints ['7'], want ['9']"}


def test_a_missing_corpus_leaves_the_row_unchecked_not_checked_or_failed_as_a_miscompile(monkeypatch, tmp_path):
    fake(monkeypatch, {"error": "skipped: corpus not here"})
    out = {KEY: dict(SIZED)}
    bench.run_timed(out, {KEY: Path("a.exe")}, {KEY: {"files": (), "want": []}}, tmp_path, timing=False)
    assert out[KEY] == SIZED | {"unchecked": "corpus not here"}


def test_the_default_bench_feeds_a_program_its_ci_bytes_under_the_corpus_name_and_wants_the_ci_output(monkeypatch, tmp_path):
    """grep read all 10 MB or nothing: unavailable on CI it was never run. The gate runs it on ci_bytes of the corpus (same file name) against NAME.ci.out."""
    corpus_file = tmp_path / "cache" / "dickens"
    corpus_file.parent.mkdir()
    corpus_file.write_bytes(bytes(range(256)) * 8)
    directory = tmp_path / "grep"
    directory.mkdir()
    (directory / "grep.out").write_text("9\n9\n")
    (directory / "grep.ci.out").write_text("1\n2\n")
    monkeypatch.setattr(bench.corpus, "path", lambda name: corpus_file)
    variant = bench.Variant("grep", "c", directory / "grep.c", "bench_grep", None, "qb45", True, ("@dickens",), 100)
    small = bench.timing_input(variant, directory, tmp_path / "ci")
    assert small["want"] == ["1", "2"] and [f.name for f in small["files"]] == ["dickens"] and small["files"][0].stat().st_size == 100
    full = bench.timing_input(variant, directory)
    assert full["want"] == ["9", "9"] and full["files"][0].stat().st_size == 2048, "--time keeps the whole corpus"


def test_a_flat_row_measure_flat_already_ran_on_the_whole_corpus_is_not_run_again_beside_the_reduced_ones(monkeypatch, tmp_path):
    """grep bq32 -O2/-Os printed 115 against 4745 on CI: it was run again here in one DOSBox batch with the reduced copies of the corpus under the same name."""
    ran = fake(monkeypatch, {"cycles": 5, "ms": 1.0})
    flat = ("grep", "bq32", "O2")
    out = {KEY: dict(SIZED), flat: {"instructions": 623}}
    needs = {KEY: {"files": (), "want": ["1"]}, flat: {"files": (), "want": ["9"]}}
    bench.run_timed(out, {KEY: Path("a.exe"), flat: Path("b.exe")}, needs, tmp_path, timing=False, already={flat})
    assert ran == [[KEY]] and out[flat] == {"instructions": 623}


def test_a_kernel_the_compiler_folded_away_reads_as_unmeasured_not_as_a_failure(monkeypatch, tmp_path):
    """`mov eax, <crc>` is all that is left of crc/nib at -O3: maps.locate finds no kernel and no single call from main."""
    monkeypatch.setattr(bench.maps, "locate", lambda exe, listing, region: None)
    done = bench.measure(("V000", tmp_path / "x.exe", tmp_path / "x.map", "bench_crc", ["1"], tmp_path / "x.obj"))
    assert done["error"].startswith("unmeasured: kernel folded away: bench_crc is not in the map")
