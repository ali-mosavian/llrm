"""Compare Mach-O __compact_unwind encodings of two arm64 objects, per function.

usage: compact_unwind_diff.py PIN.o OURS.o [--objdump PATH]
Exit 1 when either object has no entries: clang drops the table without
-funwind-tables, and an empty table once read as "0 differences".
"""
import re
import subprocess
import sys

ENTRY = re.compile(r'start:\s+(0x[0-9a-f]+)\s+(\S+)\s*\n\s*length:.*\n\s*compact encoding:\s+(0x[0-9a-f]+)')


def entries(text):
    # The function at the start of __text is named by the section's local
    # label (`ltmp0`) in clang's objects: key it by address 0.
    found = {('@0' if int(m.group(1), 16) == 0 else m.group(2)): int(m.group(3), 16) for m in ENTRY.finditer(text)}
    if not found:
        raise ValueError('no __compact_unwind entries')
    return found


def compare(pin, ours):
    """Per function in both: 'dwarf' (pin frame, ours DWARF), 'mode', or 'mask'."""
    out = {}
    for name, a in pin.items():
        b = ours.get(name)
        if b is None or a == b:
            continue
        if a >> 24 == 4 and b >> 24 == 3:
            out[name] = 'dwarf'
        elif a >> 24 != b >> 24:
            out[name] = 'mode'
        else:
            out[name] = 'mask'
    return out


def main(argv):
    objdump = 'llvm-objdump'
    if '--objdump' in argv:
        at = argv.index('--objdump')
        objdump = argv[at + 1]
        del argv[at:at + 2]
    pin_path, ours_path = argv
    tables = []
    for path in (pin_path, ours_path):
        text = subprocess.run([objdump, '--unwind-info', path], capture_output=True, text=True).stdout
        try:
            tables.append(entries(text))
        except ValueError:
            print(f'{path}: no __compact_unwind entries (built without -funwind-tables?)', file=sys.stderr)
            return 1
    differences = compare(*tables)
    common = len(set(tables[0]) & set(tables[1]))
    print(f'{common} common functions; differences: {differences}')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
