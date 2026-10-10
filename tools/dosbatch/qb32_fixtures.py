"""GORILLA (or any BASIC source) built for dos32 as an LE executable beside DOS32A.EXE, with its linker map, with and
without debug info (and with `debug all` and `debug codeview`): the fixtures for symbol loading in the DOSBox fork.

    python3 tools/dosbatch/qb32_fixtures.py SOURCE.BAS OUTDIR
"""

from __future__ import annotations

import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402


def build(source: Path, out: Path) -> list[Path]:
    out.mkdir(parents=True, exist_ok=True)
    runtime = qb32.build(out / "runtime")
    made = []
    for tag, flags, debug in (("", ("-O2",), ()), ("_G", ("-O2", "-g"), ("debug", "all")), ("_CV", ("-O2", "-g"), ("debug", "codeview"))):
        name = source.stem.upper() + tag
        obj = out / f"{name}.obj"
        if reason := qb32.compile_basic(source, obj, flags):
            raise SystemExit(f"compile {name}: {reason}")
        exe, listing = out / f"{name}.EXE", out / f"{name}.MAP"
        loaders = dosbatch.link_target(
            qb32.TARGET, obj, exe, out, listing=listing, before=debug, runtime=(qb32.START_FILES, []), objects_after=tuple(qb32.closure(obj, runtime))
        )
        for loader in loaders:
            shutil.copy(loader, out / "DOS32A.EXE")
        made += [exe, listing]
    return made + [out / "DOS32A.EXE"]


if __name__ == "__main__":
    for path in build(Path(sys.argv[1]), Path(sys.argv[2])):
        print(path)
