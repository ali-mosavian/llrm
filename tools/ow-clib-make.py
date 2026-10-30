"""Open Watcom's DOS C library as one GNU Makefile: wmake's dry run of each library directory, flattened, so `make -jN` builds it.

    ow-clib-make.py OWROOT OUT.mk VARIANT...      VARIANT: msdos.086/mm, msdos.386/ms_s ...; mathlib:msdos.387/ms_r for the math library

OWROOT is a copy of the Open Watcom tree (objects are written into it); the environment has WATCOM, and wmake and the
build tools on PATH (the `bw*` compilers). The generated headers (`bld/hdr/dos`) must be built once with wmake.
"""
import re
import subprocess
import sys
from pathlib import Path

root, out, variants = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve(), sys.argv[3:]
if not variants:
    sys.exit(__doc__)


def steps(directory: Path) -> list[tuple[str, str]]:
    """(name from the `echo` line, command) in order, from wmake's dry run in `directory`."""
    done = subprocess.run(["wmake", "-h", "-n"], cwd=directory, capture_output=True, text=True)
    if done.returncode:
        sys.exit(f"{directory}: {done.stdout[-400:]}{done.stderr[-400:]}")
    found, name = [], ""
    for line in done.stdout.splitlines():
        if line.startswith("echo "):
            name = line.split(None, 2)[2]
        elif line.strip() and not line.startswith("mkdir"):
            found.append((name, line))
    unique = {}
    for name, command in found:
        unique.setdefault(name, command)
    return list(unique.items())


rules, libs = [], []
for variant in variants:
    project, _, variant = variant.rpartition(":")
    project = project or "clib"
    base = root / "bld" / project / "library" / variant
    if project == "clib":
        categories = re.findall(r"\$\(pfx__\)(\w+)\$\(__sfx\)", (base.parent / "libs.mif").read_text())
    else:
        categories = ["alias"]  # mathlib builds its one category, `alias`, from its c and a directories
    merged = []
    for category in categories:
        directory = root / "bld" / project / category / "library" / variant
        if not (directory / "makefile").exists():
            continue
        objects = []
        for name, command in steps(directory):
            if name.endswith(".lib"):
                target = directory / name
                rules.append(f"{target}: {' '.join(map(str, objects))}\n\tcd {directory} && {command}\n")
                merged.append(target)
            else:
                target = directory / name
                source = re.search(r"\S+\.(?:c|asm|gas)\b", command)
                prerequisite = f" {(directory / source.group()).resolve()}" if source else ""
                rules.append(f"{target}:{prerequisite}\n\tcd {directory} && {command}\n")
                objects.append(target)
    final = base
    # The merge directory's dry run wants the category libraries to exist; empty ones stand in for it.
    stand_ins = [one for one in merged if not one.exists()]
    for one in stand_ins:
        one.touch()
    merge_steps = steps(final)
    for one in stand_ins:
        one.unlink()
    own = []
    for name, command in merge_steps:
        target = final / name
        if name.endswith(".lib"):
            # wmake's `%create`/`%append` lines: the response file lists the category libraries, then this directory's objects.
            response = re.search(r"@(\S+\.lbc)", command)
            entries = "".join(f"+{one}\\n" for one in [*merged, *own])
            write = f"\t@printf '{entries}' > {final / response.group(1)}\n" if response else ""
            rules.append(f"{target}: {' '.join(map(str, [*merged, *own]))}\n\t@rm -f {target}\n{write}\tcd {final} && {command}\n")
            libs.append(target)
        else:
            source = re.search(r"\S+\.(?:c|asm|gas)\b", command)
            prerequisite = f" {(final / source.group()).resolve()}" if source else ""
            rules.append(f"{target}:{prerequisite}\n\tcd {final} && {command}\n")
            own.append(target)
out.write_text(f".PHONY: all\nall: {' '.join(map(str, libs))}\n\n" + "\n".join(rules))
print(f"{len(rules)} rules, libraries: {' '.join(map(str, libs))}")
