"""The m16 QB runtime's start-up zeroes its uninitialised data from whichever object the linker met first.

start.asm zeroed from its own empty `_BSS` fragment, which is the first byte of the near uninitialised data only where start.obj is
the first object with any: with it last the start-up cleared 4 bytes of 1,176 and the statics (`redirected`, the console's driver
pointer) kept what the last program left in memory, so the first PRINT jumped through 0000:0000 (#1488: color_pairs at -O0, and at
-O2 by the layout). `BSSB`, an empty segment ahead of `_BSS` in every object that has one, fixes where it begins."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbbuild  # noqa: E402

pytestmark = pytest.mark.slow_dos

PROGRAM = 'PRINT "hello"\nEND\n'


def link(program: Path, objects: dict[str, Path], exe: Path, order: list[Path]) -> None:
    command = [str(dosbatch.BIN / "jwlink"), "option", "quiet", "format", "dos", "name", str(exe), "file", str(program)]
    for one in order:
        command += ["file", str(one)]
    dosbatch._host(command)


def ran_dirty(exe: Path, work: Path) -> str:
    """`exe` run under DIRTY.COM, which fills free memory with 0A5h first, its output."""
    dirty = work.parent / "dirty.com"
    dosbatch._host([str(dosbatch.BIN / "jwasm"), "-q", "-bin", f"-Fo{dirty}", str(dosbatch.ROOT / "tools" / "dosbatch" / "dirty.asm")])
    job = dosbatch.Job("QBBSS", "exe", exe, files=(dirty,), runner="DIRTY.COM", budget_ms=60000)
    result = dosbatch.run([job], work, conf=qb32.virtual_conf(2000))["QBBSS"]
    assert result.status == "ok", result.detail
    return result.text.strip()


@pytest.mark.parametrize("start", ["first", "last"])
def test_the_statics_are_zero_whatever_object_is_linked_first(tmp_path, start: str):
    """PRINT reads the console's `redirected`, zero statics: on memory DOS left dirty, with start.obj first or last of the runtime."""
    objects = qbbuild.runtimes(tmp_path / "rt")[qbbuild.REAL]
    source = tmp_path / "hello.bas"
    source.write_text(PROGRAM)
    obj = tmp_path / "hello.obj"
    done = dosbatch.subprocess.run(
        [str(dosbatch.BIN / "llrm-qb"), str(source), "--dialect", "qb45", "-fqb-runtime=llrm", "-O2", "-o", str(obj)], capture_output=True, text=True
    )
    assert done.returncode == 0, done.stderr
    needed = qbbuild.needed((obj,), objects)
    begin = objects["start"]
    rest = [one for one in needed if one != begin]
    order = [begin, *rest] if start == "first" else [*rest, begin]
    exe = tmp_path / f"hello_{start}.exe"
    link(obj, objects, exe, order)
    assert ran_dirty(exe, tmp_path / "run") == "hello"
