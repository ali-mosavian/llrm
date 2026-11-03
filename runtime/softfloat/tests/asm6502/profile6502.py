#!/usr/bin/env python3
"""profile6502.py BIN [-l LABELS] [--dbg DBGFILE] [--maxcycles N]: run a sim65 binary on an NMOS 6502 model and print cycles and call
counts per function plus an opcode-class histogram. BIN is cc65's sim65 format (12-byte header, program at the load address).
Cycles are the documented NMOS counts: +1 for a page crossing on indexed reads, +1 when a branch is taken, +1 more when it crosses
a page (judged from the page of the branch opcode, as sim65 does; real silicon uses the next instruction's). The paravirt calls at $FFF4..$FFF9 (open close read write args exit) are the ones cc65's sim6502 runtime uses; exit
stops the run with A as the exit code.
Labels: ld65 -Ln file ('al 00ABCD .name') and/or --dbgfile (every label, including unexported ones). A function is a JSR target
reached while running (or an exported label); its range runs to the next such address. Exclusive cycles go to the range holding PC,
inclusive cycles to every frame of a shadow call stack (a function counts once per outermost activation).
Importable: run(binary_bytes, ...) -> Result."""
import argparse, re, sys

# opcode -> (mnemonic, mode, base cycles, class). Classes: load store alu shift incdec cmp branch jump stack xfer flag misc
_T = {}
def _op(code, name, mode, cyc, cls): _T[code] = (name, mode, cyc, cls)
def _fill():
    # (name, class, {mode: (opcode, cycles)}); 'r' modes add a page-cross cycle when indexed
    groups = [
        ('LDA', 'load',  dict(imm=(0xA9, 2), zp=(0xA5, 3), zpx=(0xB5, 4), abs=(0xAD, 4), absx=(0xBD, 4), absy=(0xB9, 4), indx=(0xA1, 6), indy=(0xB1, 5))),
        ('LDX', 'load',  dict(imm=(0xA2, 2), zp=(0xA6, 3), zpy=(0xB6, 4), abs=(0xAE, 4), absy=(0xBE, 4))),
        ('LDY', 'load',  dict(imm=(0xA0, 2), zp=(0xA4, 3), zpx=(0xB4, 4), abs=(0xAC, 4), absx=(0xBC, 4))),
        ('STA', 'store', dict(zp=(0x85, 3), zpx=(0x95, 4), abs=(0x8D, 4), absx=(0x9D, 5), absy=(0x99, 5), indx=(0x81, 6), indy=(0x91, 6))),
        ('STX', 'store', dict(zp=(0x86, 3), zpy=(0x96, 4), abs=(0x8E, 4))),
        ('STY', 'store', dict(zp=(0x84, 3), zpx=(0x94, 4), abs=(0x8C, 4))),
        ('ADC', 'alu',   dict(imm=(0x69, 2), zp=(0x65, 3), zpx=(0x75, 4), abs=(0x6D, 4), absx=(0x7D, 4), absy=(0x79, 4), indx=(0x61, 6), indy=(0x71, 5))),
        ('SBC', 'alu',   dict(imm=(0xE9, 2), zp=(0xE5, 3), zpx=(0xF5, 4), abs=(0xED, 4), absx=(0xFD, 4), absy=(0xF9, 4), indx=(0xE1, 6), indy=(0xF1, 5))),
        ('AND', 'alu',   dict(imm=(0x29, 2), zp=(0x25, 3), zpx=(0x35, 4), abs=(0x2D, 4), absx=(0x3D, 4), absy=(0x39, 4), indx=(0x21, 6), indy=(0x31, 5))),
        ('ORA', 'alu',   dict(imm=(0x09, 2), zp=(0x05, 3), zpx=(0x15, 4), abs=(0x0D, 4), absx=(0x1D, 4), absy=(0x19, 4), indx=(0x01, 6), indy=(0x11, 5))),
        ('EOR', 'alu',   dict(imm=(0x49, 2), zp=(0x45, 3), zpx=(0x55, 4), abs=(0x4D, 4), absx=(0x5D, 4), absy=(0x59, 4), indx=(0x41, 6), indy=(0x51, 5))),
        ('CMP', 'cmp',   dict(imm=(0xC9, 2), zp=(0xC5, 3), zpx=(0xD5, 4), abs=(0xCD, 4), absx=(0xDD, 4), absy=(0xD9, 4), indx=(0xC1, 6), indy=(0xD1, 5))),
        ('CPX', 'cmp',   dict(imm=(0xE0, 2), zp=(0xE4, 3), abs=(0xEC, 4))),
        ('CPY', 'cmp',   dict(imm=(0xC0, 2), zp=(0xC4, 3), abs=(0xCC, 4))),
        ('BIT', 'cmp',   dict(zp=(0x24, 3), abs=(0x2C, 4))),
        ('ASL', 'shift', dict(acc=(0x0A, 2), zp=(0x06, 5), zpx=(0x16, 6), abs=(0x0E, 6), absx=(0x1E, 7))),
        ('LSR', 'shift', dict(acc=(0x4A, 2), zp=(0x46, 5), zpx=(0x56, 6), abs=(0x4E, 6), absx=(0x5E, 7))),
        ('ROL', 'shift', dict(acc=(0x2A, 2), zp=(0x26, 5), zpx=(0x36, 6), abs=(0x2E, 6), absx=(0x3E, 7))),
        ('ROR', 'shift', dict(acc=(0x6A, 2), zp=(0x66, 5), zpx=(0x76, 6), abs=(0x6E, 6), absx=(0x7E, 7))),
        ('INC', 'incdec', dict(zp=(0xE6, 5), zpx=(0xF6, 6), abs=(0xEE, 6), absx=(0xFE, 7))),
        ('DEC', 'incdec', dict(zp=(0xC6, 5), zpx=(0xD6, 6), abs=(0xCE, 6), absx=(0xDE, 7))),
        ('INX', 'incdec', dict(imp=(0xE8, 2))), ('INY', 'incdec', dict(imp=(0xC8, 2))),
        ('DEX', 'incdec', dict(imp=(0xCA, 2))), ('DEY', 'incdec', dict(imp=(0x88, 2))),
        ('BPL', 'branch', dict(rel=(0x10, 2))), ('BMI', 'branch', dict(rel=(0x30, 2))), ('BVC', 'branch', dict(rel=(0x50, 2))),
        ('BVS', 'branch', dict(rel=(0x70, 2))), ('BCC', 'branch', dict(rel=(0x90, 2))), ('BCS', 'branch', dict(rel=(0xB0, 2))),
        ('BNE', 'branch', dict(rel=(0xD0, 2))), ('BEQ', 'branch', dict(rel=(0xF0, 2))),
        ('JMP', 'jump', dict(abs=(0x4C, 3), ind=(0x6C, 5))), ('JSR', 'jump', dict(abs=(0x20, 6))),
        ('RTS', 'jump', dict(imp=(0x60, 6))), ('RTI', 'jump', dict(imp=(0x40, 6))), ('BRK', 'jump', dict(imp=(0x00, 7))),
        ('PHA', 'stack', dict(imp=(0x48, 3))), ('PHP', 'stack', dict(imp=(0x08, 3))),
        ('PLA', 'stack', dict(imp=(0x68, 4))), ('PLP', 'stack', dict(imp=(0x28, 4))),
        ('TAX', 'xfer', dict(imp=(0xAA, 2))), ('TXA', 'xfer', dict(imp=(0x8A, 2))), ('TAY', 'xfer', dict(imp=(0xA8, 2))),
        ('TYA', 'xfer', dict(imp=(0x98, 2))), ('TSX', 'xfer', dict(imp=(0xBA, 2))), ('TXS', 'xfer', dict(imp=(0x9A, 2))),
        ('CLC', 'flag', dict(imp=(0x18, 2))), ('SEC', 'flag', dict(imp=(0x38, 2))), ('CLI', 'flag', dict(imp=(0x58, 2))),
        ('SEI', 'flag', dict(imp=(0x78, 2))), ('CLV', 'flag', dict(imp=(0xB8, 2))), ('CLD', 'flag', dict(imp=(0xD8, 2))),
        ('SED', 'flag', dict(imp=(0xF8, 2))), ('NOP', 'misc', dict(imp=(0xEA, 2))),
    ]
    for name, cls, modes in groups:
        for mode, (code, cyc) in modes.items(): _op(code, name, mode, cyc, cls)
_fill()
CLASSES = ['load', 'store', 'alu', 'cmp', 'shift', 'incdec', 'branch', 'jump', 'stack', 'xfer', 'flag', 'misc']
_SIZE = dict(imp=1, acc=1, imm=2, zp=2, zpx=2, zpy=2, rel=2, indx=2, indy=2, abs=3, absx=3, absy=3, ind=3)
# indexed reads pay a cycle on a page crossing; stores and read-modify-write have their fixed count already
_PAGE_PENALTY = {n for n in ('LDA', 'LDX', 'LDY', 'ADC', 'SBC', 'AND', 'ORA', 'EOR', 'CMP')}

PV_OPEN, PV_CLOSE, PV_READ, PV_WRITE, PV_ARGS, PV_EXIT = 0xFFF4, 0xFFF5, 0xFFF6, 0xFFF7, 0xFFF8, 0xFFF9

class Result:
    def __init__(self): self.cycles = 0; self.exit = None; self.insns = 0; self.classes = {}; self.funcs = {}; self.fclasses = {}; self.addr = {}; self.mem = None; self.rng = []

def parse_labels(path):
    """ld65 -Ln file: name -> address."""
    out = {}
    for l in open(path):
        m = re.match(r'al ([0-9A-Fa-f]+) \.(\S+)', l)
        if m: out.setdefault(m.group(2), int(m.group(1), 16))
    return out

def parse_dbg(path):
    """ld65 --dbgfile: every label (type=lab) with its address."""
    out = {}
    for l in open(path):
        if l.startswith('sym\t'):
            n = re.search(r'name="([^"]+)"', l); t = re.search(r'type=(\w+)', l); v = re.search(r'val=0x([0-9A-Fa-f]+)', l)
            if n and t and v and t.group(1) == 'lab': out.setdefault(n.group(1), int(v.group(1), 16))
    return out

def run(binary, labels=None, args=('sim65',), maxcycles=500_000_000, trace=None, blocks=False):
    """Execute a sim65 image. labels: name -> address. Returns Result with .cycles .exit .classes .funcs {entry: [name, calls, excl, incl]}."""
    if binary[:5] != b'sim65': raise ValueError('not a sim65 binary')
    cpu, spzp, load, reset = binary[6], binary[7], binary[8] | binary[9] << 8, binary[10] | binary[11] << 8
    if cpu != 0: raise ValueError('cpu type %d: only the NMOS 6502 is modelled' % cpu)
    mem = bytearray(65536); prog = binary[12:]; mem[load:load + len(prog)] = prog
    names = {}
    for n, a in sorted((labels or {}).items(), key=lambda kv: (len(kv[0]), kv[0]), reverse=True): names[a] = n
    entries = set()
    res = Result(); funcs = res.funcs
    A = X = Y = 0; S = 0xFF; PC = reset
    N = V = Z = C = 0; D = 0; I = 1
    cyc = 0; ninsn = 0
    stack = []  # (entry, cycle at entry)
    active = {}
    calls = {}
    incl = {}
    ex_addr = {}
    entries.add(reset)
    def push(v): nonlocal S; mem[0x100 | S] = v & 255; S = (S - 1) & 255
    def pull():
        nonlocal S; S = (S + 1) & 255; return mem[0x100 | S]
    def getP(b): return (N << 7) | (V << 6) | 0x20 | (b << 4) | (D << 3) | (I << 2) | (Z << 1) | C
    op_cycles = _T
    while True:
        if PC >= PV_OPEN and PC <= PV_EXIT:  # paravirt: acts as the routine and returns
            if PC == PV_EXIT:
                # sim65 does not charge the JMP/JSR that enters the exit hook
                cyc -= last_c; ninsn -= 1; e = ex_addr[last_ip]; e[0] -= 1; e[1] -= last_c
                res.exit = A; break
            if PC == PV_ARGS:
                # argc in A/X, argv built on the C stack at the zero-page pointer the header names
                sp = mem[spzp] | mem[spzp + 1] << 8; strs = [s.encode() + b'\0' for s in args]
                sp -= sum(len(s) for s in strs); p = sp; ptrs = []
                for s in strs: mem[p:p + len(s)] = s; ptrs.append(p); p += len(s)
                sp -= 2 * (len(strs) + 1)
                for i, q in enumerate(ptrs + [0]): mem[sp + 2 * i] = q & 255; mem[sp + 2 * i + 1] = q >> 8
                mem[spzp], mem[spzp + 1] = sp & 255, sp >> 8
                A, X = len(strs) & 255, len(strs) >> 8; mem[0x2], mem[0x3] = sp & 255, sp >> 8
            else: raise NotImplementedError('paravirt $%04X' % PC)
            PC = ((pull() | pull() << 8) + 1) & 0xFFFF
            continue
        if cyc > maxcycles: raise RuntimeError('cycle limit')
        op = mem[PC]
        try: name, mode, base, cls = op_cycles[op]
        except KeyError: raise NotImplementedError('opcode $%02X at $%04X' % (op, PC))
        ipc = PC
        sz = _SIZE[mode]
        lo = mem[(PC + 1) & 0xFFFF]; hi = mem[(PC + 2) & 0xFFFF]
        PC = (PC + sz) & 0xFFFF
        c = base
        ea = None
        if mode == 'imm': val = lo
        elif mode == 'zp': ea = lo
        elif mode == 'zpx': ea = (lo + X) & 255
        elif mode == 'zpy': ea = (lo + Y) & 255
        elif mode in ('abs', 'ind'): ea = lo | hi << 8
        elif mode == 'absx':
            b = lo | hi << 8; ea = (b + X) & 0xFFFF
            if name in _PAGE_PENALTY and (b ^ ea) & 0xFF00: c += 1
        elif mode == 'absy':
            b = lo | hi << 8; ea = (b + Y) & 0xFFFF
            if name in _PAGE_PENALTY and (b ^ ea) & 0xFF00: c += 1
        elif mode == 'indx':
            z = (lo + X) & 255; ea = mem[z] | mem[(z + 1) & 255] << 8
        elif mode == 'indy':
            b = mem[lo] | mem[(lo + 1) & 255] << 8; ea = (b + Y) & 0xFFFF
            if name in _PAGE_PENALTY and (b ^ ea) & 0xFF00: c += 1
        if ea is not None and mode != 'ind' and name not in ('JMP', 'JSR', 'STA', 'STX', 'STY'): val = mem[ea] if mode not in ('acc',) else A
        if name == 'LDA': A = val; N = A >> 7; Z = A == 0
        elif name == 'LDX': X = val; N = X >> 7; Z = X == 0
        elif name == 'LDY': Y = val; N = Y >> 7; Z = Y == 0
        elif name == 'STA': mem[ea] = A
        elif name == 'STX': mem[ea] = X
        elif name == 'STY': mem[ea] = Y
        elif name == 'ADC':
            if D: raise NotImplementedError('decimal mode')
            t = A + val + C; V = ((~(A ^ val) & (A ^ t)) >> 7) & 1; C = t >> 8; A = t & 255; N = A >> 7; Z = A == 0
        elif name == 'SBC':
            if D: raise NotImplementedError('decimal mode')
            v2 = val ^ 255; t = A + v2 + C; V = ((~(A ^ v2) & (A ^ t)) >> 7) & 1; C = t >> 8; A = t & 255; N = A >> 7; Z = A == 0
        elif name == 'AND': A &= val; N = A >> 7; Z = A == 0
        elif name == 'ORA': A |= val; N = A >> 7; Z = A == 0
        elif name == 'EOR': A ^= val; N = A >> 7; Z = A == 0
        elif name == 'CMP': t = A - val; C = A >= val; N = (t >> 7) & 1; Z = (t & 255) == 0
        elif name == 'CPX': t = X - val; C = X >= val; N = (t >> 7) & 1; Z = (t & 255) == 0
        elif name == 'CPY': t = Y - val; C = Y >= val; N = (t >> 7) & 1; Z = (t & 255) == 0
        elif name == 'BIT': N = val >> 7; V = (val >> 6) & 1; Z = (A & val) == 0
        elif name in ('ASL', 'LSR', 'ROL', 'ROR'):
            v = A if mode == 'acc' else mem[ea]
            if name == 'ASL': C = v >> 7; v = (v << 1) & 255
            elif name == 'LSR': C = v & 1; v >>= 1
            elif name == 'ROL': t = C; C = v >> 7; v = ((v << 1) | t) & 255
            else: t = C; C = v & 1; v = (v >> 1) | t << 7
            N = v >> 7; Z = v == 0
            if mode == 'acc': A = v
            else: mem[ea] = v
        elif name == 'INC': v = (mem[ea] + 1) & 255; mem[ea] = v; N = v >> 7; Z = v == 0
        elif name == 'DEC': v = (mem[ea] - 1) & 255; mem[ea] = v; N = v >> 7; Z = v == 0
        elif name == 'INX': X = (X + 1) & 255; N = X >> 7; Z = X == 0
        elif name == 'INY': Y = (Y + 1) & 255; N = Y >> 7; Z = Y == 0
        elif name == 'DEX': X = (X - 1) & 255; N = X >> 7; Z = X == 0
        elif name == 'DEY': Y = (Y - 1) & 255; N = Y >> 7; Z = Y == 0
        elif mode == 'rel':
            t = {'BPL': not N, 'BMI': N, 'BVC': not V, 'BVS': V, 'BCC': not C, 'BCS': C, 'BNE': not Z, 'BEQ': Z}[name]
            if t:
                tgt = (PC + (lo - 256 if lo > 127 else lo)) & 0xFFFF
                c += 1 + (((ipc ^ tgt) & 0xFF00) != 0); PC = tgt  # sim65 compares the branch opcode's page; an NMOS the next instruction's
        elif name == 'JMP':
            if mode == 'abs': PC = ea
            else: PC = mem[ea] | mem[(ea & 0xFF00) | ((ea + 1) & 255)] << 8  # the NMOS page-wrap bug
        elif name == 'JSR':
            ret = (PC - 1) & 0xFFFF; push(ret >> 8); push(ret & 255); PC = ea
            entries.add(ea)
            f = ea
            calls[f] = calls.get(f, 0) + 1
            stack.append((f, cyc + c))
            active[f] = active.get(f, 0) + 1
        elif name == 'RTS':
            PC = ((pull() | pull() << 8) + 1) & 0xFFFF
            if stack:
                f, t0 = stack.pop(); active[f] -= 1
                if not active[f]: incl[f] = incl.get(f, 0) + cyc + c - t0
        elif name == 'RTI':
            p = pull(); N, V, D, I, Z, C = p >> 7, (p >> 6) & 1, (p >> 3) & 1, (p >> 2) & 1, (p >> 1) & 1, p & 1; PC = pull() | pull() << 8
        elif name == 'BRK': raise NotImplementedError('BRK at $%04X' % ipc)
        elif name == 'PHA': push(A)
        elif name == 'PHP': push(getP(1))
        elif name == 'PLA': A = pull(); N = A >> 7; Z = A == 0
        elif name == 'PLP': p = pull(); N, V, D, I, Z, C = p >> 7, (p >> 6) & 1, (p >> 3) & 1, (p >> 2) & 1, (p >> 1) & 1, p & 1
        elif name == 'TAX': X = A; N = X >> 7; Z = X == 0
        elif name == 'TXA': A = X; N = A >> 7; Z = A == 0
        elif name == 'TAY': Y = A; N = Y >> 7; Z = Y == 0
        elif name == 'TYA': A = Y; N = A >> 7; Z = A == 0
        elif name == 'TSX': X = S; N = X >> 7; Z = X == 0
        elif name == 'TXS': S = X
        elif name == 'CLC': C = 0
        elif name == 'SEC': C = 1
        elif name == 'CLI': I = 0
        elif name == 'SEI': I = 1
        elif name == 'CLV': V = 0
        elif name == 'CLD': D = 0
        elif name == 'SED': D = 1
        elif name == 'NOP': pass
        cyc += c; ninsn += 1
        e = ex_addr.get(ipc)
        if e is None: e = ex_addr[ipc] = [0, 0, cls]
        e[0] += 1; e[1] += c
        last_c, last_ip = c, ipc
        if trace: trace(ipc, name, cyc)
    # the exit hook is entered by JSR or JMP: its call and the frames still open are charged as they stand
    import bisect
    if blocks: entries |= {a for a in names if load <= a < load + len(prog)}  # every label starts a range: block-level profile
    rng = sorted(entries)
    ex_by = {}; fcls = res.fclasses
    for a_, (n_, c_, cl_) in ex_addr.items():
        i = bisect.bisect_right(rng, a_) - 1; k_ = rng[i] if i >= 0 else 0
        ex_by[k_] = ex_by.get(k_, 0) + c_
        v = fcls.setdefault(k_, {}).setdefault(cl_, [0, 0]); v[0] += n_; v[1] += c_
    for f, t0 in stack:
        active[f] -= 1
        if not active[f]: incl[f] = incl.get(f, 0) + cyc - t0
    res.cycles = cyc; res.insns = ninsn; res.addr = ex_addr; res.mem = bytes(mem); res.rng = rng
    for fc in fcls.values():
        for c_, (n_, cy_) in fc.items():
            o = res.classes.get(c_, (0, 0)); res.classes[c_] = (o[0] + n_, o[1] + cy_)
    def nm(a): return names.get(a) or '$%04X' % a
    for a in set(ex_by) | set(calls):
        res.funcs[a] = [nm(a), calls.get(a, 0), ex_by.get(a, 0), incl.get(a, 0)]
    return res

def disasm(mem, a, names=None):
    name, mode, _, _ = _T[mem[a]]; lo, hi = mem[a + 1], mem[a + 2]; w = lo | hi << 8
    f = lambda v: (names or {}).get(v) or '$%04X' % v
    op = {'imp': '', 'acc': ' a', 'imm': ' #$%02X' % lo, 'zp': ' $%02X' % lo, 'zpx': ' $%02X,x' % lo, 'zpy': ' $%02X,y' % lo,
          'abs': ' ' + f(w), 'absx': ' %s,x' % f(w), 'absy': ' %s,y' % f(w), 'ind': ' (%s)' % f(w), 'indx': ' ($%02X,x)' % lo,
          'indy': ' ($%02X),y' % lo, 'rel': ' ' + f((a + 2 + (lo - 256 if lo > 127 else lo)) & 0xFFFF)}[mode]
    return name.lower() + op, _SIZE[mode]

def annotate(res, labels, func, out=sys.stdout):
    """Listing of one function's range (by name) with execution count and cycles per instruction, label at each block."""
    names = {}
    for n, a in sorted(labels.items(), key=lambda kv: (len(kv[0]), kv[0]), reverse=True): names[a] = n
    start = labels[func]; nxt = [a for a in res.rng if a > start]; end = nxt[0] if nxt else start + 1
    a = start
    while a < end:
        if a in names: print('%s:' % names[a], file=out)
        t, n = disasm(res.mem, a, names); e = res.addr.get(a)
        print('  %04X %9s %9s  %s' % (a, e[0] if e else '', e[1] if e else '', t), file=out); a += n

def merge(results):
    """Sum several Results (same binary layout)."""
    t = Result()
    for r in results:
        t.cycles += r.cycles; t.insns += r.insns; t.mem = r.mem; t.rng = sorted(set(t.rng) | set(r.rng))
        for a, (n, cy, cl) in r.addr.items():
            o = t.addr.setdefault(a, [0, 0, cl]); o[0] += n; o[1] += cy
        for c, (n, cy) in r.classes.items():
            o = t.classes.get(c, (0, 0)); t.classes[c] = (o[0] + n, o[1] + cy)
        for a, (nm_, ca, ex, inc) in r.funcs.items():
            o = t.funcs.setdefault(a, [nm_, 0, 0, 0]); o[1] += ca; o[2] += ex; o[3] += inc
        for a, fc in r.fclasses.items():
            for c, (n, cy) in fc.items():
                v = t.fclasses.setdefault(a, {}).setdefault(c, [0, 0]); v[0] += n; v[1] += cy
    return t

def report(r, top=12, out=sys.stdout):
    print('total %d cycles, %d instructions, exit %s' % (r.cycles, r.insns, r.exit), file=out)
    print('%-28s %9s %6s %9s %9s %6s' % ('function', 'calls', '', 'excl', 'incl', 'excl%'), file=out)
    for a, (nm_, ca, ex, inc) in sorted(r.funcs.items(), key=lambda kv: -kv[1][2])[:top]:
        print('%-28s %9d %6s %9d %9d %5.1f%%' % (nm_, ca, '', ex, inc, 100 * ex / max(r.cycles, 1)), file=out)
    print('class      insns   cycles  cyc%', file=out)
    for c in CLASSES:
        n, cy = r.classes.get(c, (0, 0))
        print('%-8s %8d %8d %5.1f%%' % (c, n, cy, 100 * cy / max(r.cycles, 1)), file=out)

def labels_for(lbl=None, dbg=None):
    d = {}
    if dbg: d.update(parse_dbg(dbg))
    if lbl: d.update(parse_labels(lbl))
    return d

if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('bin'); ap.add_argument('-l', '--labels'); ap.add_argument('--dbg'); ap.add_argument('--top', type=int, default=12)
    ap.add_argument('--annotate', help='print this function with per-instruction counts')
    ap.add_argument('--maxcycles', type=int, default=500_000_000)
    a = ap.parse_args()
    r = run(open(a.bin, 'rb').read(), labels_for(a.labels, a.dbg), maxcycles=a.maxcycles)
    report(r, a.top)
    if a.annotate: annotate(r, labels_for(a.labels, a.dbg), a.annotate)
