"""List the cells whose columns differ in the runtime calls the kernel makes (memset, memcpy...).

Usage: libcalls.py OUT  (the OUT dir of tools/arm64/cell.sh runs)
       libcalls.py --object OBJ  (the runtime calls of one object, as cell.sh records them; V: llvm-objdump's directory)
 A kernel-only count flatters the side that calls
the runtime and charges the side that inlines it: compare these cells on the whole program (count.py --whole).
"""
import glob, os, subprocess, sys

OBJDUMP = os.path.join(os.environ.get('V') or '/usr/lib/llvm-20/bin', 'llvm-objdump')
RUNTIME = {'memset', 'memcpy', 'memmove', 'memcmp', 'bcmp', 'strlen', 'strcpy', 'strcmp'}


def of_object(path):
    """The runtime functions `path` calls, sorted; an llvm-objdump that fails raises."""
    out = subprocess.run([OBJDUMP, '-r', path], capture_output=True, text=True, check=True).stdout
    fields = (line.split() for line in out.splitlines())
    return sorted({f[2] for f in fields if len(f) > 2 and ('CALL26' in f[1] or 'JUMP26' in f[1])} & RUNTIME)


def calls(out):
    """{(prog, level): {column: set of called names}} from the PROG.LEVEL.COLUMN.libcalls files."""
    cells = {}
    for path in glob.glob(os.path.join(out, '*.libcalls')):
        prog, level, column = os.path.basename(path)[:-len('.libcalls')].rsplit('.', 2)
        cells.setdefault((prog, level), {})[column] = set(open(path).read().split())
    return cells


def differing(cells):
    """The cells where the columns' sets are not all equal, sorted."""
    return sorted((key, columns) for key, columns in cells.items() if len({frozenset(v) for v in columns.values()}) > 1)


if __name__ == '__main__':
    if sys.argv[1] == '--object':
        print(' '.join(of_object(sys.argv[2])))
        raise SystemExit
    for (prog, level), columns in differing(calls(sys.argv[1])):
        print(prog, level, *(f'{c}={",".join(sorted(v)) or "-"}' for c, v in sorted(columns.items())))
