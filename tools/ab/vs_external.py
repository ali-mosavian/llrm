#!/usr/bin/env python3
"""vs_external.py OURS_DIST OUT.tsv LEVELS FILES...: x86_64 code bytes of ours, gcc and clang per cell.

Both external columns take `external_flags.flags` (no distro defaults: canary, endbr64, frame pointer) and the program's own
flags, gcc in the spelling `gcc_program_flags` gives. A program that splits on the compiler, or that gcc cannot take the flags
of, has no gcc column (0). Unreadable cells print EMPTY.

An external .o that calls a libgcc helper (`__popcountdi2`, `__udivti3`) has the helper's code outside it: the program links
those bytes too, so they are added to that column (the bytes of a call are the bytes of the routine too, as m68k's
`libcall_choice`). The row ends with `gcc_calls` and `clang_calls`, each `name:member_bytes,...` or `-`, saying what was added."""
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import cell_flags  # noqa: E402
import comparable  # noqa: E402
import external_flags  # noqa: E402
import object_bytes  # noqa: E402

INCLUDE = ["-I", str(ROOT / "runtime/shared/posix")]
RELOC = ("-fPIE",)  # what ours emits for x86_64-linux


def external_commands(source: str, level: str, include=INCLUDE, reloc=RELOC) -> dict[str, list[str] | None]:
    """The gcc and clang compile lines (without `-c SRC -o OUT`) for a source; None where a column cannot be quoted."""
    program = cell_flags.cell_flags(source)
    common = list(external_flags.flags("x86", reloc=reloc))
    try:
        gcc_program = external_flags.gcc_program_flags(program)
    except ValueError:
        gcc_program = None
    return {
        "gcc": None if gcc_program is None or comparable.compiler_split(source) else ["gcc", level, "-w", *common, *gcc_program, *include],
        "clang": ["clang", level, "-w", *common, *program, *include],
    }


def libgcc_path() -> Path:
    return Path(subprocess.run(["gcc", "-print-libgcc-file-name"], capture_output=True, text=True).stdout.strip())


def libgcc_members(nm_output: str) -> dict[str, str]:
    """Symbol -> archive member, from `nm -A --defined-only libgcc.a`."""
    found = {}
    for line in nm_output.splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[1] in ("T", "t", "W") and line.count(":") >= 2:
            found[parts[2]] = line.split(":")[1]
    return found


def undefined_helpers(nm_output: str, helpers: dict[str, str]) -> list[str]:
    """The undefined symbols of `nm -u` output that libgcc defines: code the object calls and does not hold."""
    return sorted({parts[-1] for line in nm_output.splitlines() if (parts := line.split()) and parts[0] == "U" and parts[-1] in helpers})


def member_bytes(archive: Path, member: str) -> int:
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / member
        with open(out, "wb") as fh:
            subprocess.run(["ar", "p", str(archive), member], stdout=fh, check=True)
        return object_bytes.object_bytes("x86_64-linux", out)[0]


def library_calls(obj: Path) -> str:
    """`name:bytes,...` of the libgcc helpers `obj` calls, or `-`."""
    archive = libgcc_path()
    helpers = libgcc_members(subprocess.run(["nm", "-A", "--defined-only", str(archive)], capture_output=True, text=True).stdout)
    called = undefined_helpers(subprocess.run(["nm", "-u", str(obj)], capture_output=True, text=True).stdout, helpers)
    return ",".join(f"{name}:{member_bytes(archive, helpers[name])}" for name in called) or "-"


def charged(
    size: int,
    calls: str,
) -> int:
    """`size` plus the bytes of the helpers `calls` (`library_calls`) names: what the program that links them holds."""
    return size + sum(int(part.rsplit(":", 1)[1]) for part in calls.split(",") if ":" in part)


def one(job):
    dist, level, src = job
    text = Path(src).read_text()
    lines = {"ours": [f"{dist}/llrm-cc", level, "-w", *cell_flags.cell_flags(text), *INCLUDE], **external_commands(text, level)}
    sizes = {}
    calls = {"gcc": "-", "clang": "-"}
    with tempfile.TemporaryDirectory() as tmp:
        for name, line in lines.items():
            if line is None:
                sizes[name] = 0
                continue
            out = Path(tmp) / f"{name}.o"
            env = dict(os.environ, LLRM_CC_TARGET="x86_64-linux")
            run = subprocess.run([*line, "-c", src, "-o", str(out)], env=env, capture_output=True, text=True, timeout=600)
            if run.returncode or not out.exists():
                return f"EMPTY\t{src}\t{level}\t{name}"
            sizes[name] = object_bytes.object_bytes("x86_64-linux", out)[0]
            if name in calls:
                calls[name] = library_calls(out)
                sizes[name] = charged(sizes[name], calls[name])
    return f"{src}\t{level}\t{sizes['ours']}\t{sizes['gcc']}\t{sizes['clang']}\t{calls['gcc']}\t{calls['clang']}"


if __name__ == "__main__":
    dist, out, levels = sys.argv[1:4]
    jobs = [(dist, level, file) for file in sys.argv[4:] for level in levels.split(",")]
    with ProcessPoolExecutor(8) as pool, open(out, "w") as fh:
        for row in pool.map(one, jobs):
            fh.write(row + "\n")
