"""objsweep and rowdiff tell a changed object and a worse row from an unchanged one, and say a compile that failed instead of counting it unchanged."""
from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def fake_llrm(path: Path, body: str) -> str:
    """A stand-in llrm-c: writes `body` to the -o file; a source containing FAIL makes it exit 1."""
    path.write_text(f'#!/bin/bash\n# {'#' * 5000 * (1 + ord(body[0]) % 2)}\nfor a; do [ "$p" = -o ] && o=$a; p=$a; done\ngrep -q FAIL "$a" && {{ echo "llrm-c: boom" >&2; exit 1; }}\necho {body} > "$o"\n')
    path.chmod(0o755)
    return str(path)


def test_objsweep_names_a_changed_object_and_a_one_sided_failure(tmp_path):
    src = tmp_path / "b"
    src.mkdir()
    (src / "same.c").write_text("int a;")
    (src / "broken.c").write_text("FAIL")
    old, new = fake_llrm(tmp_path / "old", "x"), fake_llrm(tmp_path / "new", "y")
    done = subprocess.run([sys.executable, str(HERE / "objsweep.py"), old, new, "--src", str(src), "--levels", "O2", "--targets", "m32"], capture_output=True, text=True)
    assert done.returncode == 1
    assert "1 objects changed" in done.stdout and "changed same m32 -O2" in done.stdout
    assert "failed on both sides" in done.stdout and "1 failed on both sides" in done.stdout


def test_rowdiff_counts_rows_above_a_and_names_a_row_on_one_side():
    rowdiff = load("rowdiff")
    row = lambda c, b: {"clocks": c, "code": b}
    a = {("p", "llrm"): row(100, 50), ("q", "llrm"): row(10, 10)}
    b = {("p", "llrm"): row(120, 50), ("r", "llrm"): row(1, 1)}
    lines, above = rowdiff.diff(a, b, set())
    assert above == 1
    assert "p -O2: clocks 100->120 (+20.0%), code 50->50 (+0.0%)" in lines
    assert "q -O2: only in A" in lines and "r -O2: only in B" in lines


def test_the_stub_defines_strlen(tmp_path):
    """The pinned clang turns x_strlen's loop into a call to strlen; the stub lacked it and all four clang rows of x_strlen failed to link (the table had no reference)."""
    stub = HERE.parents[1] / "crates/target/llrm-x86-m32/vsgcc/stub.s"
    subprocess.run(["gcc", "-m32", "-c", str(stub), "-o", str(tmp_path / "s.o")], check=True)
    symbols = subprocess.run(["nm", str(tmp_path / "s.o")], capture_output=True, text=True, check=True).stdout
    assert " T strlen" in symbols


def test_a_pair_of_one_build_relinked_is_void(tmp_path):
    """Two llrm-c built into one shared target dir differed by 83 path bytes; objsweep read 0 objects changed of two copies of one compiler."""
    voidpair = load("voidpair")
    base = bytes(range(256)) * 1024
    a, b, c = tmp_path / "a", tmp_path / "b", tmp_path / "c"
    a.write_bytes(base)
    b.write_bytes(base[:100] + b"x" * 83 + base[183:])
    c.write_bytes(bytes(reversed(base)))
    assert voidpair.void(str(a), str(b)).startswith("VOID PAIR")
    assert voidpair.void(str(a), str(c)) == ""
    done = subprocess.run([sys.executable, str(HERE / "objsweep.py"), str(a), str(b), "--src", str(tmp_path)], capture_output=True, text=True)
    assert done.returncode == 2 and "VOID PAIR" in done.stdout


def test_runpinned_stops_when_the_dist_lacks_the_linker(tmp_path):
    """A VM without jwasm/jwlink/dosbox-x in the dist ran no test and the pair's run diff was empty, read as 'no new failure'."""
    dist = tmp_path / "dist"
    dist.mkdir()
    (dist / "llrm-c").write_text("")
    repo = HERE.parents[1]
    done = subprocess.run(["bash", str(HERE / "runpinned.sh"), "0" * 9, str(dist), str(tmp_path / "out.txt")], capture_output=True, text=True, env={"PIN_TREES": str(tmp_path), "PATH": "/usr/bin:/bin:/usr/local/bin", "HOME": str(tmp_path)}, cwd=repo)
    assert done.returncode == 2 and "is missing" in done.stderr, done.stderr
    assert not (tmp_path / "out.txt").exists()


def test_rundiff_sets_aside_unbuilt_programs_and_names_a_new_failure():
    """A VM without V:\\LINK failed 137 programs per side; pairruns called the pair void, and the 7 that ran were not compared."""
    rundiff = load("rundiff")
    group = "[c] 3 programs: 1 pass, 0 known, 2 fail\n"
    base = group + 'FAIL  qb/a.bas: not built: Bad command or filename - "V:\\LINK"\n' + "FAIL  c/x.c [-O2]: wrong output\n"
    new = group + 'FAIL  qb/a.bas: not built: Bad command or filename - "V:\\LINK"\n' + "FAIL  c/y.c [-O2]: wrong output\n"
    added, fixed, census = rundiff.compare(base, new)
    assert added == ["NEW FAIL c/y.c [-O2]: wrong output"] and fixed == ["c/x.c [-O2]"]
    assert "not built (V:\\LINK etc.): base 1, new 1" in census


def test_bisect_stops_when_the_command_cannot_run(tmp_path):
    """A runner that refused stale binaries exited 1 like a failing test; every later probe read as bad and 40bc8dd60 was named the culprit of stft_single."""
    source = (HERE / "firstbad.py").read_text()
    assert "not 0 or 1" in source and "dist.glob(\"llrm-*\")" in source


def test_board_and_cbisect_say_what_they_need_instead_of_running():
    """Run without QCPORT or a worktree they print their usage and exit 2: a board with no corpus measured an empty compile once."""
    for script in ("board.py", "cbisect.py"):
        done = subprocess.run([sys.executable, str(HERE / script)], capture_output=True, text=True, env={"PATH": "/usr/bin:/bin"})
        assert done.returncode == 2, (script, done.stderr)
