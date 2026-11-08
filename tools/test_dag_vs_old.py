"""dag_vs_old.sh at m16 compared OMF file sizes (fixups, symbols, pool) and a failing code_bytes was swallowed: -Os read 'zero Dag-larger' where 28 cells were."""
import os
import subprocess
import textwrap
from pathlib import Path

SCRIPT = Path(__file__).with_name("dag_vs_old.sh")


def sandbox(tmp: Path, sizes_body: str) -> Path:
    (tmp / "tools").mkdir()
    (tmp / "tools/pyproject.toml").write_text('[project]\nname = "t"\nversion = "0"\n')
    (tmp / "tools/llrmbin.py").write_text("def stale_binaries(*_):\n    return ['x']\n")
    (tmp / "tools/sizes.py").write_text(sizes_body)
    (tmp / "tools/dag_vs_old.sh").write_text(SCRIPT.read_text())
    (tmp / "tools/dag_vs_old.sh").chmod(0o755)
    (tmp / "tests/run/c").mkdir(parents=True)
    (tmp / "tests/run/c/a.c").write_text("")
    (tmp / "bench").mkdir()
    for name in ("llrm-cc-bin", "llrm-llc"):
        (tmp / "target/release").mkdir(parents=True, exist_ok=True)
        (tmp / "target/release" / name).write_text('#!/bin/bash\nout=; while [ $# -gt 0 ]; do [ "$1" = -o ] && out=$2; case $1 in --isel=dag) n=900;; --isel=old) n=100;; esac; shift; done\nhead -c $n /dev/zero > $out\n')
        (tmp / "target/release" / name).chmod(0o755)
    subprocess.run(["git", "init", "-q"], cwd=tmp, check=True)
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "x"], cwd=tmp, check=True)
    return tmp / "tools/dag_vs_old.sh"


def test_release_is_measured_even_when_a_debug_build_exists(tmp_path):
    """A stale target/debug/llrm-cc-bin was picked over release: struct_odd_sizes read 139 B (Dag larger) where release gave 128 (smaller)."""
    script = sandbox(tmp_path, "def code_bytes(obj):\n    return 50\n")
    debug = tmp_path / "target/debug"
    debug.mkdir(parents=True)
    for name in ("llrm-cc-bin", "llrm-llc"):
        (debug / name).write_text("#!/bin/bash\nexit 1\n")
        (debug / name).chmod(0o755)
    done = subprocess.run([str(script), "x86_16-dos", "-march=i386", "-Os"], cwd=tmp_path, capture_output=True, text=True)
    assert "target/release" in done.stdout and "target/debug" not in done.stdout and "Dag fails" in done.stdout
    assert "\na FAIL" not in done.stdout


def test_an_elf_target_with_no_omf_cells_still_reports(tmp_path):
    """The code_bytes guard exited 1 for every ELF target (no OMF rows to convert): the m32 and m64 sweeps printed a header and nothing else."""
    script = sandbox(tmp_path, "def code_bytes(obj):\n    raise SystemExit('boom')\n")
    size = tmp_path / "bin"
    size.mkdir()
    (size / "size").write_text("#!/bin/bash\necho 'section size addr'\necho '.text 40 0'\n")
    (size / "size").chmod(0o755)
    env = {**os.environ, "PATH": f"{size}:{os.environ['PATH']}"}
    done = subprocess.run([str(script), "x86_32-linux", "-m32", "-Os"], cwd=tmp_path, capture_output=True, text=True, env=env)
    assert done.returncode == 0 and "totals: both 1 sources" in done.stdout, done.stdout + done.stderr


def run(tmp_path: Path, sizes_body: str):
    script = sandbox(tmp_path, sizes_body)
    return subprocess.run([str(script), "x86_16-dos", "-march=i386", "-Os"], cwd=tmp_path, capture_output=True, text=True)


def test_an_omf_cell_is_measured_by_code_bytes_not_file_size(tmp_path):
    """File sizes 100 vs 900 are the same code (50 B): no Dag-larger cell."""
    done = run(tmp_path, "from pathlib import Path\ndef code_bytes(obj):\n    return 50\n")
    assert "a 50 50" not in done.stdout and "Dag larger than Old" in done.stdout
    assert "\na " not in done.stdout.split("Dag larger than Old")[1].split("==")[0], done.stdout


def test_a_failing_code_bytes_stops_the_sweep(tmp_path):
    done = run(tmp_path, "def code_bytes(obj):\n    raise SystemExit('boom')\n")
    assert done.returncode != 0 and "Dag larger than Old" not in done.stdout
