"""Executed-instruction count of an arm64 ELF under unicorn.

The range is chosen, never defaulted: `--kernel` counts the compiled kernels (every function of the object, up to
`report`, the runtime's first function, without `main`'s own bytes); `--whole` counts the program, runtime included. A count with no flag once ran whole and was read as the kernel
(a constant +12..13 offset that was not one: tile clang 980 vs 16866).
"""
import argparse, hashlib, os, struct, sys, tempfile

BENCH = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'bench')


def find_input(name):
    """The file a program opens by its DOS name: ARM64_BENCH_DATA/NAME, else the same name, any case, in a bench/<prog>/ dir."""
    wanted = name.upper()
    if wanted == 'SIGNAL':  # generated, not stored: the corpus the DOS harness writes (tools/hostshim/host.py)
        sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'dosbatch'))
        import signal_gen
        data = signal_gen.signal()
        path = os.path.join(tempfile.gettempdir(), 'arm64-bench-SIGNAL-' + hashlib.md5(data).hexdigest())
        if not os.path.exists(path):
            open(path, 'wb').write(data)
        return path
    if wanted == 'DICKENS':  # the 10 MB corpus, verified against its pinned hash; rt.c reads the first ci_bytes of it
        sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'dosbatch'))
        import corpus
        return str(corpus.path('dickens'))
    if os.environ.get('ARM64_BENCH_DATA'):
        return os.path.join(os.environ['ARM64_BENCH_DATA'], wanted)
    for folder in sorted(os.listdir(BENCH)):
        directory = os.path.join(BENCH, folder)
        for entry in os.listdir(directory) if os.path.isdir(directory) else ():
            if entry.upper() == wanted:
                return os.path.join(directory, entry)
    return wanted


def kernel_range(funcs, sizes):
    """(lo, hi, (main_lo, main_hi)): the program object's text, runtime and `main` left out.

    The object links first and the runtime (rt.c, from `report`) after it, so the kernels are everything
    below `report`. A helper the linker places after `main` is a kernel too: the range once ended at `main` and
    dropped it (jpeg_single `getbit`, 965,879 instrs ours / 1,881,194 clang no-vec, in no count).
    """
    return min(funcs.values()), funcs['report'], (funcs['main'], funcs['main'] + sizes['main'])


def classes(funcs, sizes):
    """Every executable address in exactly one class: [(lo, hi, name)], sorted. `other` is whatever is left."""
    lo, hi, (main_lo, main_hi) = kernel_range(funcs, sizes)
    text_end = max(funcs[name] + sizes[name] for name in funcs)
    return [(lo, main_lo, 'kernel'), (main_lo, main_hi, 'main'), (main_hi, hi, 'kernel'), (hi, text_end, 'runtime')]


def class_of(ranges, address):
    for lo, hi, name in ranges:
        if lo <= address < hi:
            return name
    return 'other'


def check_sum(counted, whole):
    """kernel + main + runtime must be the whole-process count: a range that loses instructions is an error."""
    parts = sum(counted.get(name, 0) for name in ('kernel', 'main', 'runtime'))
    if counted.get('other', 0) or parts != whole:
        raise SystemExit(f'count.py: kernel+main+runtime = {parts}, whole = {whole}, outside every range = {counted.get("other", 0)}')


def histogram_wants(name, kernel):
    """Whether an address of class `name` goes in the histogram: with `kernel` only the kernels' own."""
    return not kernel or name == 'kernel'


def run(path, limit=300_000_000, kernel=None, histogram=None):
    """`histogram`, a dict, is filled with the execution count of every address in the chosen range (tools/arm64/clocks.py prices them)."""
    from unicorn import Uc, UcError, UC_ARCH_ARM64, UC_MODE_ARM, UC_HOOK_CODE, UC_HOOK_INTR
    from unicorn.arm64_const import UC_ARM64_REG_SP, UC_ARM64_REG_CPACR_EL1, UC_ARM64_REG_X30, UC_ARM64_REG_X8, UC_ARM64_REG_X0, UC_ARM64_REG_X1, UC_ARM64_REG_X2, UC_ARM64_REG_PC
    d = open(path, 'rb').read()
    entry = struct.unpack_from('<Q', d, 24)[0]
    phoff, = struct.unpack_from('<Q', d, 32); phsz, phn = struct.unpack_from('<HH', d, 54)
    syms = {}
    funcs = {}
    sizes = {}
    shoff, = struct.unpack_from('<Q', d, 40); shsz, shn, shstr = struct.unpack_from('<HHH', d, 58)
    secs = [struct.unpack_from('<IIQQQQIIQQ', d, shoff + i * shsz) for i in range(shn)]
    for s in secs:
        if s[1] == 2:  # SYMTAB
            strs = secs[s[6]]
            for off in range(s[4], s[4] + s[5], 24):
                nm, info, oth, shn_, val, sz = struct.unpack_from('<IBBHQQ', d, off)
                e = d.index(b'\0', strs[4] + nm); syms[d[strs[4]+nm:e].decode()] = val
                if info & 0xf == 2: funcs[d[strs[4]+nm:e].decode()] = val; sizes[d[strs[4]+nm:e].decode()] = sz
    uc = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
    for i in range(phn):
        t, fl, off, va, pa, fsz, msz, al = struct.unpack_from('<IIQQQQQQ', d, phoff + i * phsz)
        if t != 1: continue
        lo = va & ~0xfff; hi = (va + msz + 0xfff) & ~0xfff
        try: uc.mem_map(lo, hi - lo)
        except UcError: pass
        uc.mem_write(va, d[off:off + fsz])
    uc.mem_map(0x7ff00000, 0x100000)
    uc.reg_write(UC_ARM64_REG_SP, 0x7ffff000)
    uc.reg_write(UC_ARM64_REG_CPACR_EL1, 3 << 20)
    uc.reg_write(UC_ARM64_REG_X30, 0xdead0000)
    ranges = classes(funcs, sizes)
    counted, whole = {}, [0]
    def hook(u, a, s, ud):
        whole[0] += 1
        name = class_of(ranges, a)
        if histogram is not None and histogram_wants(name, kernel): histogram[a] = histogram.get(a, 0) + 1
        counted[name] = counted.get(name, 0) + 1
    uc.hook_add(UC_HOOK_CODE, hook)
    files, written, nextfd = {}, hashlib.md5(), [3]
    def cstr(addr):
        out = b''
        while True:
            c = bytes(uc.mem_read(addr, 1))
            if c == b'\0': return out.decode()
            out += c; addr += 1
    def svc(u, intno, ud):
        # the Linux calls the file programs make: openat, read, write, close, exit
        nr = u.reg_read(UC_ARM64_REG_X8); a0, a1, a2 = (u.reg_read(r) for r in (UC_ARM64_REG_X0, UC_ARM64_REG_X1, UC_ARM64_REG_X2))
        ret = -38
        if nr == 56:
            path = find_input(cstr(a1))
            if a2 & 0x3:  # created: kept in memory, hashed as written
                fd = nextfd[0]; nextfd[0] += 1; files[fd] = ('w', None); ret = fd
            elif os.path.exists(path):
                fd = nextfd[0]; nextfd[0] += 1; files[fd] = ('r', open(path, 'rb')); ret = fd
            else: ret = -2
        elif nr == 63 and a0 in files and files[a0][0] == 'r':
            data = files[a0][1].read(a2); u.mem_write(a1, data); ret = len(data)
        elif nr == 64:
            data = bytes(u.mem_read(a1, a2))
            if a0 in files and files[a0][0] == 'w': written.update(data)
            ret = a2
        elif nr == 57: files.pop(a0, None); ret = 0
        elif nr in (93, 94): u.emu_stop(); return
        u.reg_write(UC_ARM64_REG_X0, ret & (2**64 - 1))
    uc.hook_add(UC_HOOK_INTR, svc)
    try: uc.emu_start(entry, 0xdead0000, count=limit)
    except UcError as e:
        if uc.reg_read(UC_ARM64_REG_PC) != 0xdead0000: return None, whole[0], str(e) + " pc=%x" % uc.reg_read(UC_ARM64_REG_PC)
    rep = struct.unpack('<q', uc.mem_read(syms['reported'], 8))[0] if 'reported' in syms else None
    if written.digest() != hashlib.md5().digest(): rep = f'{rep}:{written.hexdigest()[:8]}'
    check_sum(counted, whole[0])
    return rep, (counted.get('kernel', 0) if kernel else whole[0]), ''
if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('elf')
    where = ap.add_mutually_exclusive_group(required=True)
    where.add_argument('--kernel', action='store_true', help='count the compiled kernels only')
    where.add_argument('--whole', action='store_true', help='count the whole program')
    args = ap.parse_args()
    r, n, err = run(args.elf, kernel=args.kernel); print(r, n, err)
