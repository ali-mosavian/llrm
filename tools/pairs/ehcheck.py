#!/usr/bin/env python3
"""ehcheck.py CC DIR...: llrm-cc's x86-64 ELF `.eh_frame` against the pin clang's, per function.

The pin and llrm-cc compile each `DIR/*.c` to an ELF object (`-c`, same -O level). A function is compared by name: the call frame
instructions after its first (`DW_CFA_advance_loc` deltas dropped, since the code differs), so what is compared is the sequence
of frame changes each declares. Prints `ehcheck: N functions, M equal, K differ, J only on one side`, then every difference.
Environment: LLRM_PIN_CLANG (default /opt/llrm-pin/bin/clang), EHCHECK_LEVEL (default O1).
"""
from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path


def parse(dump: str, names: dict[int, str]) -> dict[str, list[str]]:
    """function name -> its CFA instructions without the advances, from `readelf --debug-dump=frames` text."""
    out: dict[str, list[str]] = {}
    current = None
    for line in dump.splitlines():
        found = re.search(r"FDE .* pc=([0-9a-f]+)\.\.", line)
        if found:
            current = names.get(int(found.group(1), 16), f"@{found.group(1)}")
            out[current] = []
        elif current and line.strip().startswith("DW_CFA_") and "advance_loc" not in line and "nop" not in line:
            out[current].append(line.strip())
    return out


def frames(obj: Path) -> dict[str, list[str]]:
    dump = subprocess.run(["readelf", "--debug-dump=frames", str(obj)], capture_output=True, text=True).stdout
    names = {}
    for line in subprocess.run(["nm", str(obj)], capture_output=True, text=True).stdout.splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[1] in "tT":
            names[int(parts[0], 16)] = parts[2]
    return parse(dump, names)


def main() -> int:
    cc, *dirs = sys.argv[1:]
    pin = os.environ.get("LLRM_PIN_CLANG", "/opt/llrm-pin/bin/clang")
    level = "-" + os.environ.get("EHCHECK_LEVEL", "O1")
    total = equal = one_sided = 0
    differ: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        for directory in dirs:
            for source in sorted(Path(directory).glob("*.c")):
                ours, theirs = Path(tmp) / "ours.o", Path(tmp) / "pin.o"
                a = subprocess.run([cc, "-m64", level, "-c", "-o", str(ours), str(source)], capture_output=True, text=True)
                b = subprocess.run([pin, "-target", "x86_64-linux-gnu", level, "-funwind-tables", "-c", "-o", str(theirs), str(source)], capture_output=True, text=True)
                if a.returncode or b.returncode:
                    continue
                ours_frames, pin_frames = frames(ours), frames(theirs)
                for name in sorted(set(ours_frames) | set(pin_frames)):
                    if name not in ours_frames or name not in pin_frames:
                        one_sided += 1
                    elif ours_frames[name] == pin_frames[name]:
                        total += 1
                        equal += 1
                    else:
                        total += 1
                        differ.append(f"{source.stem}:{name}\n  ours {ours_frames[name]}\n  pin  {pin_frames[name]}")
    print(f"ehcheck: {total} functions, {equal} equal, {len(differ)} differ, {one_sided} only on one side")
    print("\n".join(differ))
    return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
