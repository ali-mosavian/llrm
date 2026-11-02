#!/usr/bin/env python3
"""`-g` changes only debug sections.

Each program is compiled by NEW without `-g` and with it. The objects must hold the same sections with the same bytes, except the `.debug*` ones.
A `-g` object with no `.debug*` section is a leg that did not run `-g`: it counts as refused, never as identical.
"""
import argparse
import sys
import tempfile
from pathlib import Path

import objsweep


def differences(off: dict[str, bytes], on: dict[str, bytes]) -> list[str]:
    """The non-debug sections that differ or that only one side has."""
    keep = lambda sections: {name: data for name, data in sections.items() if ".debug" not in name}  # noqa: E731
    a, b = keep(off), keep(on)
    return sorted(name for name in a.keys() | b.keys() if a.get(name) != b.get(name))


def has_debug(sections: dict[str, bytes]) -> bool:
    return any(name.startswith(".debug") for name in sections)


def sections_of(data: bytes) -> dict[str, bytes]:
    """The sections of an ELF object (the compile hands back its bytes and removes the file)."""
    import io  # noqa: PLC0415

    from elftools.elf.elffile import ELFFile  # noqa: PLC0415

    return {section.name: section.data() for section in ELFFile(io.BytesIO(data)).iter_sections() if section.name}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", help="NEW llrm-cc")
    parser.add_argument("--src", required=True)
    parser.add_argument("--targets", default="m32,m64", help="ELF legs only")
    parser.add_argument("--levels", default="O2,Os")
    parser.add_argument("--limit", type=int, default=40, help="the first N programs by name")
    args = parser.parse_args()
    sources = sorted(Path(args.src).glob("*.c"))[: args.limit]
    scratch = Path(tempfile.mkdtemp(prefix="dbgid-"))
    same = changed = refused = 0
    for source in sources:
        for target in args.targets.split(","):
            for level in args.levels.split(","):
                tag = f"{source.stem}-{target}-{level}"
                off, _ = objsweep.compile_one(args.compiler, source, target, level, scratch / f"{tag}.off.o", "dag", None, ["-fobject-format=elf"])
                on, _ = objsweep.compile_one(args.compiler, source, target, level, scratch / f"{tag}.on.o", "dag", None, ["-g", "-fobject-format=elf"])
                if off is None or on is None:
                    refused += 1
                    continue
                a, b = sections_of(off), sections_of(on)
                if not has_debug(b):
                    refused += 1
                    print(f"  no .debug section with -g: {tag}")
                    continue
                if (diff := differences(a, b)):
                    changed += 1
                    print(f"  differs {tag}: {', '.join(diff)}")
                else:
                    same += 1
    print(f"debug_identity: {same + changed} compared, {refused} refused of {len(sources) * len(args.targets.split(',')) * len(args.levels.split(','))}; {changed} differ, {same} identical")
    return 2 if same + changed == 0 else (1 if changed else 0)


if __name__ == "__main__":
    sys.exit(main())
