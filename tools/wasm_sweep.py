#!/usr/bin/env python3
"""wasm_sweep.py DIST OUT LIST [LEVEL...]: wasm32 bytes of ours (DIST/llrm-cc) against the pin's clang, per program of LIST.

A cell's bytes are the object's Code and Data sections. Code alone read a table the pin keeps in Data as free: a compiler that
moves constants out of the code looked smaller. The only external column is the pin's clang (gcc has no wasm target): the header says
so, and names the external flag config. Rows: `LEVEL file ours=N pin=M code_ours=C code_pin=D flags=...`, N or M `NA` when no object,
the format `wasm_rows.py sweep` reads. Needs LLRM_CLANG_WASM (the pin clang with the wasm target) and wasm-objdump.
"""
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import corpus_targets  # noqa: E402
import external_flags  # noqa: E402
import wasm_pin  # noqa: E402
from ab.cell_flags import cell_flags  # noqa: E402

SECTION = re.compile(r"^\s*(\w+) start=\S+ end=\S+ \(size=0x([0-9a-f]+)\)", re.M)


def sections(headers: str) -> dict[str, int]:
    """Section name -> bytes, from `wasm-objdump -h`."""
    return {name: int(size, 16) for name, size in SECTION.findall(headers)}


def code_bytes(headers: str) -> int:
    return sections(headers).get("Code", 0)


def module_bytes(headers: str) -> int:
    """Code plus Data: what the program costs, constants included."""
    found = sections(headers)
    return found.get("Code", 0) + found.get("Data", 0)


def measured(path: Path):
    """(Code+Data, Code) of an object, or None where there is none."""
    if not path.is_file() or path.stat().st_size == 0:
        return None
    done = subprocess.run(["wasm-objdump", "-h", str(path)], capture_output=True, text=True)
    return (module_bytes(done.stdout), code_bytes(done.stdout)) if done.returncode == 0 and "Code" in done.stdout else None


def header(dist: str) -> list[str]:
    return [
        wasm_pin.header(),
        "# external: the pin's clang only (gcc has no wasm target); not a best-of-gcc/clang bar",
        "# bytes: Code + Data sections",
        f"# dist: {dist}",
    ]


def main(dist, out, listing, levels):
    clang = os.environ["LLRM_CLANG_WASM"]
    env = {**os.environ, "LLRM_CC_TARGET": "wasm32"}
    rows = header(dist)
    with tempfile.TemporaryDirectory() as tmp:
        for level in levels:
            for name in Path(listing).read_text().split():
                source = Path(name).read_text()
                if not corpus_targets.applies("wasm32", source):
                    continue
                flags = cell_flags(source)
                ours, pin = Path(tmp) / "ours.o", Path(tmp) / "pin.o"
                for path in (ours, pin):
                    path.unlink(missing_ok=True)
                subprocess.run(
                    [f"{dist}/llrm-cc", *flags, f"-{level}", *wasm_pin.INCLUDES, "-c", name, "-o", str(ours)],
                    env=env, capture_output=True, timeout=300,
                )
                subprocess.run(wasm_pin.pin_command(clang, name, str(pin), level, flags), capture_output=True, timeout=300)
                a, b = measured(ours), measured(pin)
                text = lambda value, at: str(value[at]) if value else "NA"  # noqa: E731
                rows.append(f"{level} {name} ours={text(a, 0)} pin={text(b, 0)} code_ours={text(a, 1)} code_pin={text(b, 1)} flags={' '.join(flags) or '-'}")
    rows.append("end")
    Path(out).write_text("\n".join(rows) + "\n")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:] or ["O2", "Os"])
