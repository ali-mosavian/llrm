#!/usr/bin/env python3
"""Corpus rows compiled by llrm-z80 and run on ucsim_z80, against the best SDCC variant per row and metric.

    measure.py --z80c PATH --ir-dir DIR --baseline baseline.md [--kernels add8,max8] [-O2]

Cost = run with the call minus run without it (argument set-up, call, return and the result store), as in
bench_sdcc.py. Bytes = the kernel's code, plus the runtime routines it calls. A row counts only if the result equals the
value worked out by hand. The driver is compiled by this compiler, so both sides' argument set-up is their own.
"""
import argparse, atexit, os, re, shutil, subprocess, sys, tempfile

U8, U16 = "i8", "i16"
R = 0x8F00
# name, return type, [(param type, argument)], expected
K = [
    ("zero", "i8", [], 0), ("add8", "i8", [("i8", "200"), ("i8", "100")], 44),
    ("add16", "i16", [("i16", "300"), ("i16", "5000")], 5300), ("max8", "i8", [("i8", "3"), ("i8", "9")], 9),
    ("max16", "i16", [("i16", "1000"), ("i16", "300")], 1000), ("sum_arr", "i8", [("i8", "10")], 39),
    ("find", "i8", [("i8", "9")], 5), ("fill", "i8", [("i8", "8")], 7), ("popcount", "i8", [("i8", "181")], 5),
    ("mul8", "i8", [("i8", "7"), ("i8", "6")], 42), ("fib", "i16", [("i8", "12")], 144),
    ("gcd", "i8", [("i8", "48"), ("i8", "18")], 6), ("sum4", "i8", [], 9),
    ("slen", "i16", [("ptr", "@str")], 11), ("scmp", "i16", [("ptr", "@str"), ("ptr", "%str6")], 0xFFF1),
    ("mcopy", "i8", [("ptr", "%buf8"), ("ptr", "@buf"), ("i8", "8")], 6),
    ("crc16", "i16", [("ptr", "@buf"), ("i8", "12")], 0xA166),
    ("mul16", "i16", [("i16", "1234"), ("i16", "56")], 3568), ("digits", "i8", [("i16", "40000")], 5),
    ("reverse", "i8", [("ptr", "@buf"), ("i8", "8")], 6),
    ("bsearch8", "i8", [("ptr", "@tab"), ("i8", "8"), ("i8", "17")], 5),
    ("sum16", "i16", [("ptr", "@buf"), ("i8", "16")], 80), ("schr", "i8", [("ptr", "@str"), ("i8", "111")], 4),
    ("lz4", "i16", [("ptr", "@lz"), ("ptr", "@lzout"), ("i16", "634")], 634),
    ("lz4c", "i16", [("ptr", "@lz"), ("ptr", "@lzout"), ("i16", "634")], 52785),
    ("xs8", "i8", [("i8", "165"), ("i8", "5")], 107), ("minmax", "i8", [("ptr", "@buf"), ("i8", "16")], 8),
    ("fletcher", "i16", [("ptr", "@buf"), ("i8", "16")], 15952), ("hash16", "i16", [("ptr", "@str")], 51393),
    ("dotx", "i8", [("ptr", "@buf"), ("ptr", "@tab"), ("i8", "8")], 101),
]
# Correctness-only rows: no SDCC baseline, run only when named in --kernels.
EXTRA = [
    # the carry out of a 32-bit add is a compare used as a value (SETCC8)
    # the caller stores the 8th and 9th byte argument on the stack
    ("stackargs", "i8", [("i8", "5")], 30),
    # a 300-byte local: the frame objects below it are out of IX's byte displacement
    ("bigframe", "i8", [("i8", "5")], 5),
    # a call through a function pointer
    ("indirect", "i16", [("i16", "21")], 42),
    ("carry32", "i16", [("i32", "65535"), ("i8", "1")], 1),
]
HEADER = 'target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"\n'
CRT0 = "\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n"
DATA = """\t.module data
\t.area _DABS (ABS)
\t.org 0x9000
\t.globl _buf, _str, _tab, _r, _lz, _lzout
_r = 0x8F00
_buf:\t.db 3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3
_str:\t.ascii "hello world"
\t.db 0,0,0,0,0
_tab:\t.db 2,5,8,11,14,17,20,23
\t.org 0x9100
_lz:\t.db 48,97,98,99,3,0,17,100,2,0,255,5,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,7,0,255,26,63,120,121,122,40,0,0,242,255,0,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,113,1,0,48,101,110,100
\t.org 0x9300
_lzout:
"""


def sh(cmd, cwd, inp=None):
    return subprocess.run(cmd, cwd=cwd, input=inp, capture_output=True, text=True, timeout=300)


def driver(name, ret, args, call):
    lines = [HEADER, "@buf = external global i8", "@str = external global i8", "@tab = external global i8", "@lz = external global i8", "@lzout = external global i8",
             f"@r = external global {ret}" if ret != "void" else ""]
    if call:
        lines.append(f"declare {ret} @{name}({', '.join(t for t, _ in args)})")
    lines.append("define void @main() {")
    if call:
        lines.append("  %str6 = getelementptr i8, ptr @str, i16 6")
        lines.append("  %buf8 = getelementptr i8, ptr @buf, i16 8")
        lines.append(f"  %v = call {ret} @{name}({', '.join(f'{t} {a}' for t, a in args)})")
        lines.append(f"  store volatile {ret} %v, ptr @r")
    lines.append("  ret void\n}")
    return "\n".join(lines) + "\n"


def code_size(rel):
    return sum(int(m.group(2), 16) for m in (re.match(r"A (\S+) size ([0-9A-Fa-f]+) flags", l) for l in open(rel)) if m and m.group(1) == "_CODE")


def run(ihx, cwd):
    p = sh(["ucsim_z80", "-t", "Z80", "-q", "-w", ihx, "-c", "-"], cwd, f"run\nstate\ndump rom 0x{R:x} 0x{R+1:x}\nquit\n")
    out = p.stdout
    assert "Halted" in out, "did not halt:\n" + out[-500:]
    clks = int(re.search(r"Total time since last reset=.*\((\d+) clks\)", out).group(1))
    inst = int(re.search(r"Inst=\s*(\d+)", out).group(1))
    m = re.search(rf"0x{R:04x}\s+([0-9a-f]{{2}})\s+([0-9a-f]{{2}})", out)
    return clks, inst, int(m.group(1), 16) | int(m.group(2), 16) << 8


def shutil_copy(a, b):
    open(b, "w").write(open(a).read())


def runtime_calls(path):
    """The runtime's routines (the `__name` labels) and the routines each one calls, jumps to or falls into. A jump
    to a local label reaches the routine that holds it (the byte count is the distance between map symbols, and the
    map has only the global ones): `__modsi3` falls into `__udivmodsi4`, `__udivdi3` jumps into `udivmoddi`, which
    sits under `__moddi3`. Both were left out, and a program that divides a long was counted without the divide."""
    rows, owner, current = [], {}, None
    for line in open(path):
        line = line.split(";")[0]
        label = re.match(r"\s*(\w+)::?", line)
        if label:
            name = label.group(1)
            if name.startswith("__"):
                current = name
            owner[name] = current
            line = line[label.end():]
        line = re.sub(r"^\s*\w+\$:", "", line)  # a numbered local label may share the line with an instruction
        words = line.split()
        rows.append((label.group(1) if label else None, current, [] if words and words[0].startswith(".") else words))
    graph, previous = {}, None
    for name, current, words in rows:
        if name and name.startswith("__"):
            graph.setdefault(name, set())
            if previous and previous[0] != name and falls_through(previous[1]):
                graph[previous[0]].add(name)
        if words and current:
            for target in re.findall(r"\b(?:call|jp|jr)\s+(?:\w+\s*,\s*)?(\w+)", " ".join(words)):
                target = target if target.startswith("__") else owner.get(target)
                if target and target != current:
                    graph[current].add(target)
            previous = (current, words)
    return graph


def falls_through(words):
    """Whether control can run past the statement `words` into the next label: not after `ret`, `halt` or an
    unconditional `jp` / `jr`."""
    if words[0] in ("ret", "halt", "reti", "retn"):
        return len(words) > 1
    if words[0] in ("jp", "jr"):
        return "," in " ".join(words[1:])
    return True


def runtime_bytes(w, graph, kernel_text, defined=None):
    """Bytes of the runtime routines the kernel reaches: symbol address differences in the link map. `defined`: the
    routines of the runtime object; a name of another object (the soft-float library, linked whole and already in the
    image) is not charged a second time."""
    wanted = set(re.findall(r"\b(__\w+)", kernel_text))
    if defined is not None:
        wanted &= defined
    todo = list(wanted)
    while todo:
        for callee in graph.get(todo.pop(), ()):
            if callee not in wanted and (defined is None or callee in defined):
                wanted.add(callee); todo.append(callee)
    if not wanted:
        return 0
    # the map lists several `address name` pairs per row
    text = open(f"{w}/x.map").read()
    symbols = sorted((int(a, 16), n) for a, n in re.findall(r"\b([0-9A-F]{8})\s+([A-Za-z_]\w*)\b", text))
    addr = {n: a for a, n in symbols}
    size = 0
    for name in wanted:
        # the map cuts a name at 9 characters: `__udivmodsi4` is listed as `__udivmod`
        name = name if name in addr else name[:9]
        if name not in addr:
            continue
        following = [a for a, n in symbols if a > addr[name] and not n.startswith(("s__", "l__"))]
        if following:
            size += min(following) - addr[name]
    return size


def baseline(path):
    best = {}
    for line in open(path):
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) == 5 and re.match(r"\d+ / \d+ / \d+", cells[1]):
            rows = [tuple(int(x) for x in c.split(" / ")) for c in cells[1:] if c != "FAIL"]
            best[cells[0]] = (min(r[0] for r in rows), min(r[1] for r in rows), min(r[2] for r in rows))
    return best


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--z80c", required=True)
    ap.add_argument("--ir-dir", required=True)
    ap.add_argument("--baseline", required=True)
    ap.add_argument("--runtime", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../runtime/z80_rt.s"))
    ap.add_argument("--kernels")
    ap.add_argument("-O", dest="level", default="2")
    args = ap.parse_args()
    want = args.kernels.split(",") if args.kernels else [k[0] for k in K]
    sdcc = baseline(args.baseline)
    w = tempfile.mkdtemp(prefix="z80ours-")
    atexit.register(shutil.rmtree, w, True)  # /tmp has a fixed inode count
    open(f"{w}/crt0.s", "w").write(CRT0)
    open(f"{w}/data.s", "w").write(DATA)
    for s in ("crt0", "data"):
        assert sh(["sdasz80", "-o", f"{s}.rel", f"{s}.s"], w).returncode == 0

    shutil_copy(args.runtime, f"{w}/rt.s")
    assert sh(["sdasz80", "-g", "-o", "rt.rel", "rt.s"], w).returncode == 0
    graph = runtime_calls(args.runtime)

    def compile_ir(ir_path, stem):
        p = sh([args.z80c, f"-O{args.level}", ir_path], w)
        if "REFUSED" in p.stderr or p.returncode:
            return None, (p.stderr.strip().splitlines() or ["failed"])[0]
        if not p.stdout.strip():
            return None, "no assembly on stdout: --z80c is the z80c example, not llc"
        open(f"{w}/{stem}.s", "w").write(p.stdout)
        q = sh(["sdasz80", "-g", "-o", f"{stem}.rel", f"{stem}.s"], w)
        return (f"{stem}.rel", None) if q.returncode == 0 else (None, "sdasz80: " + (q.stdout + q.stderr)[-200:])

    print("| kernel | ours B | ours T | ours insn | SDCC B | SDCC T | SDCC insn | B | T | insn |\n|---|---|---|---|---|---|---|---|---|---|")
    for name, ret, params, expect in K + EXTRA:
        if name not in want:
            continue
        src = f"{args.ir_dir}/{name}.ll"
        kernel, why = compile_ir(src, "k")
        if not kernel:
            print(f"| {name} | refused: {why[:70]} | | | | | | | | |")
            continue
        open(f"{w}/d.ll", "w").write(driver(name, ret, params, True))
        open(f"{w}/e.ll", "w").write(driver(name, ret, params, False))
        d, why = compile_ir(f"{w}/d.ll", "d")
        e, why2 = compile_ir(f"{w}/e.ll", "e")
        if not d or not e:
            print(f"| {name} | driver refused: {(why or why2)[:70]} | | | | | | | | |")
            continue
        times = []
        for objs in (["e.rel"], ["d.rel", "k.rel"]):
            p = sh(["sdldz80", "-i", "-m", "-b", "_CODE=0x100", "x.ihx", "crt0.rel", "data.rel", *objs, "rt.rel"], w)
            if p.returncode:
                times.append(None); print(f"| {name} | link: {(p.stdout+p.stderr)[-80:]} | | | | | | | | |"); break
            times.append(run("x.ihx", w))
        if None in times:
            continue
        (t0, i0, _), (t1, i1, got) = times
        mask = 0xFFFF if ret == "i16" else 0xFF
        if got & mask != expect & mask:
            print(f"| {name} | WRONG: got {got & mask:#x}, hand-derived {expect & mask:#x} | | | | | | | | |")
            continue
        size = code_size(f"{w}/k.rel") + runtime_bytes(w, graph, open(f"{w}/k.s").read())
        assert size < 4096, f"{name}: {size} B of code: a runtime routine was sized up to a symbol far away (end the runtime with __rt_end)"
        b, t, n = sdcc.get(name, (None, None, None))
        verdict = lambda ours, theirs: "" if theirs is None else ("win" if ours < theirs else "tie" if ours == theirs else "LOSE")
        print(f"| {name} | {size} | {t1 - t0} | {i1 - i0} | {b} | {t} | {n} | {verdict(size, b)} | {verdict(t1 - t0, t)} | {verdict(i1 - i0, n)} |")


if __name__ == "__main__":
    main()
