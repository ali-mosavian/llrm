"""tools/lowercase-includes.sh fails on a source with no files.

An existing but empty qctc/inc made an empty mirror and exit 0; LLRM_BORLAND_INC then pointed at nothing and every Borland row failed on its first #include."""

import subprocess
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "lowercase-includes.sh"


def mirror(src: Path, dst: Path):
    return subprocess.run(["sh", str(SCRIPT), str(src), str(dst)], capture_output=True, text=True)


def test_an_empty_source_is_an_error_not_an_empty_mirror(tmp_path):
    (tmp_path / "inc").mkdir()
    done = mirror(tmp_path / "inc", tmp_path / "out")
    assert done.returncode == 1 and "holds no files" in done.stderr


def test_a_source_with_files_is_mirrored_in_lower_case(tmp_path):
    (tmp_path / "inc").mkdir()
    (tmp_path / "inc/ALLOC.H").write_bytes(b"int a;\x1a")
    done = mirror(tmp_path / "inc", tmp_path / "out")
    assert done.returncode == 0 and (tmp_path / "out/alloc.h").read_bytes() == b"int a;"
