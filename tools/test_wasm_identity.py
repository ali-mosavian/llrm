import os
import subprocess
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / "wasm_identity.sh"

FAKE_CC = """#!/bin/bash
# a fake llrm-cc: the object holds the name of the directory it lives in; three programs are refused
src=; out=
while [ $# -gt 0 ]; do case $1 in -o) out=$2; shift ;; *.c) src=$1 ;; esac; shift; done
case $src in
  *regparm*) echo "$src:1:1: error: expected ';'" >&2; exit 1 ;;
  *stft_single*) echo 'llrm-wasm-llc: @f: Legalize("Unsupported { op: BSwap }")' >&2; exit 1 ;;
esac
basename "$(dirname "$0")" > "$out"
"""


def dist(root: Path, name: str, llc: str, wasm_llc: str) -> Path:
    folder = root / name
    folder.mkdir()
    (folder / "llrm-cc").write_text(FAKE_CC)
    (folder / "llrm-cc").chmod(0o755)
    (folder / "llrm-llc").write_text(llc)
    (folder / "llrm-wasm-llc").write_text(wasm_llc)
    return folder


def run(base: Path, head: Path, out: Path):
    return subprocess.run([str(SCRIPT), str(base), str(head), str(out)], capture_output=True, text=True, env={**os.environ, "JOBS": "8"})


def test_two_dists_sharing_one_llrm_wasm_llc_are_refused_before_any_row_is_made(tmp_path):
    """The wasm identity run of #6710 read 0 DIFF because the two dists held byte-identical llrm-wasm-llc (their llrm-llc differed, so an llrm-llc check passes the pair)."""
    base, head = dist(tmp_path, "base", "a", "same"), dist(tmp_path, "head", "b", "same")
    done = run(base, head, tmp_path / "out")
    assert done.returncode == 2 and "same llrm-wasm-llc" in done.stderr, done.stderr
    assert not (tmp_path / "out" / "rows.txt").exists()


def test_a_front_end_rejection_is_na_and_a_backend_refusal_is_fail_and_a_changed_object_is_diff(tmp_path):
    """FAIL must mean a wasm gap: 158 of a sweep's 168 FAIL cells were DOS-only sources. The fake dists differ, so every compiled cell is DIFF and the run fails."""
    base, head = dist(tmp_path, "base", "a", "x"), dist(tmp_path, "head", "b", "y")
    done = run(base, head, tmp_path / "out")
    rows = (tmp_path / "out" / "rows.txt").read_text().splitlines()
    kinds = {row.split()[0] for row in rows}
    assert {"DIFF", "NA", "FAIL"} <= kinds, kinds
    assert done.returncode == 1
    assert all(row.split()[4] == row.split()[5] for row in rows if row.startswith("FAIL")), "a FAIL with equal codes on both sides is not an identity break"
