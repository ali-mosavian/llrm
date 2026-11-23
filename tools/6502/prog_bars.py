#!/usr/bin/env python3
"""The 6502 program-corpus bars: the tests/run/c programs of tools/6502/pass82.txt built by cc65, sdcc and llvm-mos at O1/O2/O3/Os,
bytes and clocks, against ours (tools/6502/ours_cr.py). The hand corpus' counterpart (crates/target/llrm-6502/tests/bench/bars.rs).

  prog_bars.py bars   [--refresh] [--jobs N] [--levels O1,O2] [--progs a,b] [--out prog_bars.tsv]
  prog_bars.py ours   --cc LLRM_CC --rt MOS_RT.s --rtlib RT.lib --out DIR [--levels ..] [--progs ..]
  prog_bars.py matrix --ours DIR --tip SHA --out FILE.tsv

Method. Every compiler builds the same image: driver.c (log_init, prog_main, check) + rep.s (`report` in assembly, the body ours' driver
has) + the program with `main` renamed prog_main. Program output = the longs it report()s, logged to RAM; a CHECK build compares them to
the .out and exits 0/1 (a cell whose output differs is `wrong`, not a bar). Bytes = linked image of the timing build minus the image of the
same build with an empty prog_main (the floor: startup, driver, report; no section left out: cc65 sim65 file, sdcc ihx data bytes, llvm-mos
load segments except the vectors). Clocks = run of the timing build minus the floor's run (cc65, sdcc and ours in sim65 -c; llvm-mos in
mos-sim --cycles). Ours' clocks are netted by its own empty main (overlay/__void__.c); ours' bytes are ours_cr's (the program and the runtime
members it pulls, not the driver). Both sides count the empty main's `rts`. A routine the floor already links (cc65's pushax) is free to the
program: a bar can only be smaller than the whole, never larger. External results do not change, so they are kept in prog_bars.tsv (flags
beside each row; a row whose flags differ is built again): only ours is rerun.
"""
import argparse, concurrent.futures as cf, os, re, shutil, subprocess, sys, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import corpus_targets  # noqa: E402
from flagset import program_flags  # noqa: E402

LEVELS = ["O1", "O2", "O3", "Os"]
COMPILERS = ["cc65", "sdcc", "llvm-mos"]  # the bars of bars.rs (llvm-mos with -fno-lto)
LTO = "llvm-mos-lto"  # llvm-mos' default link (LTO): the sensitivity bar; the counted line is given with and without it
ALL = COMPILERS + [LTO]


def family(compiler):
    return "llvm-mos" if compiler == LTO else compiler
BARS = HERE / "prog_bars.tsv"
NO_BAR = HERE / "no_bar_layout.tsv"  # program, compiler, reason: a program whose structs a competitor lays out differently
CORPUS = Path(os.environ.get("PROG_BARS_CORPUS", ROOT / "tests" / "run" / "c"))
MOS = Path(os.environ.get("LLVM_MOS", os.path.expanduser("~/scratch/fetch-llvm-mos-sdk/x/llvm-mos")))
INSNS = os.environ.get("SIM65_INSNS", os.path.expanduser("~/scratch/cc65-v2.19-src/sim65-insns"))


# what clang predefines and the programs read (cc65 and sdcc predefine none of it); a prelude line, not -D: cc65 takes no function-like -D
PRELUDE = "".join(f"#define {k} {v}\n" for k, v in [
    ("__INT8_TYPE__", "signed char"), ("__UINT8_TYPE__", "unsigned char"), ("__INT16_TYPE__", "short"), ("__UINT16_TYPE__", "unsigned short"),
    ("__INT32_TYPE__", "long"), ("__UINT32_TYPE__", "unsigned long"), ("__INT64_TYPE__", "long long"), ("__UINT64_TYPE__", "unsigned long long"),
    ("__SIZE_TYPE__", "unsigned"), ("__BYTE_ORDER__", "1234"), ("__ORDER_LITTLE_ENDIAN__", "1234"), ("__ORDER_BIG_ENDIAN__", "4321"),
    ("__SIZEOF_SHORT__", "2"), ("__SIZEOF_INT__", "2"), ("__SIZEOF_LONG__", "4"), ("__SIZEOF_LONG_LONG__", "8"), ("__SIZEOF_POINTER__", "2"),
    ("__SIZEOF_DOUBLE__", "4"), ("__CHAR_BIT__", "8"), ("__SCHAR_MAX__", "127"), ("__INT_MAX__", "32767"), ("__LONG_MAX__", "2147483647L")]
) + "#define __attribute__(x)\n"  # (no __attribute__ either: noinline only keeps ours' helpers apart; neither compiler inlines functions)


def flags(compiler, level):
    """The optimizer flags per level: bars.rs `flags`."""
    return {
        "cc65": {"O1": [], "O2": ["-O"], "O3": ["-Oirs"], "Os": ["-O"]},
        "sdcc": {"O1": [], "O2": [], "O3": ["--opt-code-speed"], "Os": ["--opt-code-size"]},
        "llvm-mos": {"O1": ["-O1"], "O2": ["-O2"], "O3": ["-O3"], "Os": ["-Os"]},
    }[family(compiler)][level] + (["-flto"] if compiler == LTO else [])


def expected_of(path):
    text = corpus_targets.expected_output(path, "6502-sim65").read_text().split()
    return [int(x) for x in text] if all(re.fullmatch(r"-?\d+", x) for x in text) else None


def na_class(source, why):
    """A short reason for a program a bar compiler cannot build or run: what the source asks for, then the compiler's own words."""
    kinds = [(r"\blong\s+long\b|\bint64_t\b|\buint64_t\b", "i64"), (r"\b(float|double)\b", "float/double"),
             (r"__builtin_|__atomic", "builtin"), (r"#\s*include\s*<(stdio|stdlib|string|math)\.h>|\bprintf\b", "stdlib/printf")]
    tags = [name for rx, name in kinds if re.search(rx, source)]
    return ("+".join(tags) + ": " if tags else "other: ") + why


def image_bytes_ihx(text):
    """Data bytes of an Intel hex image (sdcc)."""
    total = 0
    for line in text.splitlines():
        line = line.strip().lstrip(":")
        if len(line) >= 8 and line[6:8] == "00":
            total += int(line[0:2], 16)
    return total


def image_bytes_mos(data):
    """Bytes of a mos-sim image: every segment but the vectors at $FFFA."""
    at = total = 0
    while at + 4 <= len(data):
        address = data[at] | data[at + 1] << 8
        length = data[at + 2] | data[at + 3] << 8
        if address != 0xFFFA:
            total += length
        at += 4 + length
    return total


def sim65_from_ihx(text):
    """The sdcc image as a sim65 program (bench/sdcc.rs `sim65_program`): header, load and reset address, bytes from the lowest address up."""
    image = {}
    for line in text.splitlines():
        line = line.strip().lstrip(":")
        if len(line) < 10 or line[6:8] != "00":
            continue
        count, address = int(line[0:2], 16), int(line[2:6], 16)
        for n in range(count):
            image[address + n] = int(line[8 + 2 * n:10 + 2 * n], 16)
    word = lambda at: image.get(at, 0) | image.get(at + 1, 0) << 8
    reset = word(0xFFFC)
    body = sorted((a, v) for a, v in image.items() if a < 0xFFF1)  # sim65 keeps $FFF1 and up for its hooks
    low, high = body[0][0], body[-1][0]
    out = bytearray(b"sim65") + bytes([2, 0, 0, low & 255, low >> 8, reset & 255, reset >> 8])
    mem = bytearray(high - low + 1)
    for a, v in body:
        mem[a - low] = v
    return bytes(out + mem)


# ---- the driver every compiler links (C), and `report` in assembly with ours' body -------------------------------------------------

def driver_c(compiler, want, check):
    """main: log_init, prog_main, and in a CHECK build the exit status: 0 when the logged longs equal `want`, else 1 + index of the first
    difference (251: the program reported a different number of values)."""
    n = len(want) if want is not None else 0
    s = "#include <stdlib.h>\n" if compiler == "llvm-mos" else ""
    if compiler == "sdcc":  # sim65's exit ($FFF9) with A = status
        s += "unsigned char code;\nvoid done(void) { __asm lda _code \n jmp 0xFFF9 __endasm; }\n"
        finish = lambda c: f"{{ code = {c}; done(); }}"
    else:
        finish = lambda c: f"return {c};"
    s += f"extern void report(long v);\nextern void log_init(void);\nextern void prog_main(void);\nlong logbuf[{n + 2}];\n"
    if check:
        s += "static const long expect[%d] = {%s};\n" % (max(n, 1), ",".join(f"{v}L" for v in want) if n else "0")
    s += "int main(void)\n{\n    log_init();\n    prog_main();\n"
    if check:
        s += (f"    report(0x5A5A5A5AL);\n    {{ unsigned i; for (i = 0; i < {n}; i++) if (logbuf[i] != expect[i]) {finish('1 + (i < 250 ? i : 249)')}\n"
              f"      if (logbuf[{n}] != 0x5A5A5A5AL) {finish(251)} }}\n")
    s += "    " + finish(0) + "\n}\n"
    return s


def rep_asm(compiler):
    body = """  ldy #0
  sta (lp),y
  iny
  txa
  sta (lp),y
  iny
  lda {b2}
  sta (lp),y
  iny
  lda {b3}
  sta (lp),y
  lda lp
  clc
  adc #4
  sta lp
  bcc r1
  inc lp+1
r1:
  rts
"""
    if compiler == "cc65":
        return (".export _report, _log_init\n.import _logbuf\n.importzp sreg\n.segment \"ZEROPAGE\"\nlp: .res 2\n.segment \"CODE\"\n"
                "_log_init:\n  lda #<_logbuf\n  sta lp\n  lda #>_logbuf\n  sta lp+1\n  rts\n_report:\n" + body.format(b2="sreg", b3="sreg+1"))
    if compiler == "llvm-mos":
        return (".global report, log_init\n.section .zp.bss,\"aw\",@nobits\nlp: .space 2\n.text\n"
                "log_init:\n  lda #mos16lo(logbuf)\n  sta lp\n  lda #mos16hi(logbuf)\n  sta lp+1\n  rts\nreport:\n" + body.format(b2="__rc2", b3="__rc3"))
    # sdcc mos6502: the long argument is in _report_PARM_1..+3 (a global the caller stores), none in A
    return """\t.module rep
\t.area ZP (PAG)
lp:\t.ds 2
\t.area BSS
_report_PARM_1:\t.ds 4
\t.area CODE
\t.globl _report, _report_PARM_1, _log_init, _logbuf
_log_init:
\tlda #_logbuf
\tsta lp
\tlda #>_logbuf
\tsta *lp+1
\trts
_report:
\tldy #0
\tlda _report_PARM_1
\tsta [lp],y
\tiny
\tlda _report_PARM_1+1
\tsta [lp],y
\tiny
\tlda _report_PARM_1+2
\tsta [lp],y
\tiny
\tlda _report_PARM_1+3
\tsta [lp],y
\tlda lp
\tclc
\tadc #4
\tsta lp
\tbcc 1$
\tinc *lp+1
1$:
\trts
"""


# ---- one build -------------------------------------------------------------------------------------------------------------------

RUN_TIMEOUT = 120  # ours_cr halts a run at 120 s too
TIMEOUT = -1000  # a status no program exits with (the driver's own are 0..251)


def sh(cmd, cwd, timeout=300):
    try:
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess(cmd, TIMEOUT, "", "timeout")


def clean(text):
    """A compiler message without the temporary directory and the SDK path."""
    text = re.sub(r"\S*/(prog|floor)-src\.c", "prog.c", text)
    text = re.sub(r"\S*/bin/mos-sim-clang: ", "", text)
    return re.sub(r"\s+", " ", text).strip()


def first_error(r):
    said = (r.stdout + r.stderr).splitlines()
    pick = next((l for l in said if re.search(r"rror|undefined|Unresolved|ld.lld", l, re.I)), said[0] if said else "failed")
    return clean(pick)[:150]


def run_cycles(cmd, cwd):
    r = sh(cmd, cwd, timeout=RUN_TIMEOUT)
    text = r.stdout + r.stderr
    if "Maximum number of cycles" in text:  # sim65 gives up with exit 126, which is also the driver's "report #125 differs"
        return TIMEOUT, None, None
    m = re.findall(r"(\d+) cycles", text)
    ins = re.search(r"(\d+) instructions", text)
    return r.returncode, (int(m[-1]) if m else None), (int(ins.group(1)) if ins else None)


def build(compiler, level, w, prog_c, extra_defs, fl_clang, which, want, check):
    """Compile prog_c (or the empty floor program) with `compiler` and link it with the driver; returns (image path, None) or (None, why)."""
    lto = compiler == LTO
    compiler = family(compiler)
    src = prog_c if which == "prog" else None
    if which == "floor":
        (w / "floor.c").write_text("void prog_main(void) {}\n")
        src = w / "floor.c"
    lvl = [f for f in flags(LTO if lto else compiler, level) if f != "-flto"]
    defs = ["-Dmain=prog_main", *extra_defs]
    if compiler != "llvm-mos" and src is not None:  # no __attribute__ in cc65 or sdcc (cc65 takes no function-like -D): a copy with the macro first
        (w / f"{which}-src.c").write_text(PRELUDE + Path(src).read_text())
        src = w / f"{which}-src.c"
    (w / "driver.c").write_text(driver_c(compiler, want, check))
    (w / "rep.s").write_text(rep_asm(compiler))
    tag = f"{which}{'c' if check else 't'}"
    if compiler == "cc65":
        steps = [["cc65", "-t", "sim6502", "-j", *lvl, "-o", "drv.s", "driver.c"], ["ca65", "-t", "sim6502", "drv.s"], ["ca65", "-t", "sim6502", "rep.s", "-o", "rep.o"],
                 ["cc65", "-t", "sim6502", "-j", *lvl, *defs, "-o", f"{tag}.s", str(src)], ["ca65", "-t", "sim6502", f"{tag}.s", "-o", f"{tag}.o"]]
        link = ["cl65", "-t", "sim6502", "-m", f"{tag}.map", "-o", f"{tag}.bin", "drv.o", "rep.o", f"{tag}.o"]
    elif compiler == "sdcc":
        steps = [["sdcc", "-mmos6502", "--fsigned-char", "-c", "driver.c", "-o", "drv.rel"], ["sdas6500", "-plosgff", "rep.rel", "rep.s"],
                 ["sdcc", "-mmos6502", "--fsigned-char", *lvl, *defs, "-c", str(src), "-o", f"{tag}.rel"]]
        link = ["sdcc", "-mmos6502", "drv.rel", "rep.rel", f"{tag}.rel", "-o", f"{tag}.ihx"]
    else:
        mode = "-flto" if lto else "-fno-lto"
        base = [str(MOS / "bin/mos-sim-clang"), *lvl, mode, "-fsigned-char"]
        steps = [[*base, "-c", "driver.c", "-o", "drv.o"], [*base, "-c", "rep.s", "-o", "rep.o"],
                 [*base, *fl_clang, *defs, "-c", str(src), "-o", f"{tag}.o"]]
        link = [str(MOS / "bin/mos-sim-clang"), *lvl, mode, "-Wl,-u,report", "drv.o", "rep.o", f"{tag}.o", "-o", f"{tag}.bin"]
    for step in steps:
        r = sh(step, w)
        if r.returncode:
            return None, f"{step[0]}: {first_error(r)}"
    r = sh(link, w)
    if r.returncode:
        return None, f"link: {first_error(r)}"
    if compiler == "sdcc":
        (w / f"{tag}.sim").write_bytes(sim65_from_ihx((w / f"{tag}.ihx").read_text()))
        return w / f"{tag}.sim", None
    return w / f"{tag}.bin", None


def image_bytes(compiler, image):
    if compiler == "llvm-mos":
        return image_bytes_mos(image.read_bytes())
    if compiler == "sdcc":
        return image_bytes_ihx(image.with_suffix(".ihx").read_text())
    return image.stat().st_size


def run(compiler, image, w):
    if compiler == "llvm-mos":
        return run_cycles([str(MOS / "bin/mos-sim"), "--cycles", str(image)], w)
    return run_cycles(["sim65", "-c", "-x", "4000000000", str(image)], w)


def instructions(compiler, image, w):
    if compiler == "llvm-mos" or not os.path.exists(INSNS):
        return None
    return run_cycles([INSNS, "-c", "-x", "4000000000", str(image)], w)[2]


def measure(compiler, level, name):
    """(bytes, clocks, insns, note) of one cell; bytes None for an n/a or wrong cell, the reason in note."""
    fam = family(compiler)
    path = CORPUS / f"{name}.c"
    src = path.read_text()
    want = expected_of(path)
    if want is None:
        return None, None, None, "n/a: no numeric .out"
    fl = program_flags(path)
    defs = [f for f in fl if f.startswith("-D")]
    fl_clang = [f for f in fl if f.startswith("-f") and "sanitize" not in f] + ["-Wno-unknown-warning-option", "-Qunused-arguments"]
    w = Path(tempfile.mkdtemp(prefix=f"pb-{compiler}-{name}-", dir=os.environ.get("TMPDIR")))
    try:
        sizes = {}
        built = {}
        for which, check in (("prog", True), ("prog", False), ("floor", False)):
            image, why = build(compiler, level, w, path, defs, fl_clang, which, want, check)
            if image is None:
                return None, None, None, "n/a: " + na_class(src, why)
            built[(which, check)] = image
        rc, _, _ = run(fam, built[("prog", True)], w)
        if rc == TIMEOUT:
            return None, None, None, f"n/a: no halt (sim65 cycle cap or {RUN_TIMEOUT} s)"
        if rc != 0:
            return None, None, None, "wrong: " + (f"report #{rc - 1} differs" if 1 <= rc <= 250 else "a different number of reports" if rc == 251 else f"no clean exit (exit {rc})")
        pimg, fimg = built[("prog", False)], built[("floor", False)]
        rc, pc, _ = run(fam, pimg, w)
        rf, fc, _ = run(fam, fimg, w)
        if pc is None or fc is None or rc != 0 or rf != 0:
            return None, None, None, f"n/a: no halt (exit {rc}/{rf})"
        nb = image_bytes(fam, pimg) - image_bytes(fam, fimg) + 1  # + the floor main's own rts, as ours counts its main
        pi, fi = instructions(fam, pimg, w), instructions(fam, fimg, w)
        if nb <= 0:
            return None, None, None, f"n/a: measured {nb} B (floor subtraction)"
        return nb, pc - fc, (pi - fi if pi is not None and fi is not None else None), ("32-bit double, same output" if re.search(r"\b(double|float)\b", src) and fam != "llvm-mos" else "")
    finally:
        shutil.rmtree(w, True)


# ---- the bars file ---------------------------------------------------------------------------------------------------------------

def load_no_bar(path=NO_BAR):
    """{(program, compiler family): reason} of the cells whose program is not ours' program for that compiler: its struct
    layout differs (volatile_bitfield: `struct S` is 4 B packed here, 5 B for llvm-mos and sdcc), so the bar measures another
    program. `layouts` writes the file."""
    found = {}
    for line in Path(path).read_text().splitlines() if Path(path).exists() else []:
        if line.startswith("#") or not line.strip():
            continue
        program, compiler, reason = line.split("\t")
        found[(program, compiler)] = reason
    return found


def load_bars(path, no_bar=None):
    """The bars of `path`; a cell `no_bar` names is `n/a: layout: ...` whatever the file measured (it is kept as measured)."""
    no_bar = load_no_bar() if no_bar is None else no_bar
    rows = {}
    for line in Path(path).read_text().splitlines() if Path(path).exists() else []:
        if line.startswith("#") or not line.strip():
            continue
        p = line.split("\t")
        reason = no_bar.get((p[0], family(p[1])))
        if reason and p[4] != "n/a":  # a cell that did not build keeps its own reason
            p = [*p[:4], "n/a", "n/a", "-", f"n/a: layout: {reason}"]
        rows[(p[0], p[1], p[2])] = p[3:]
    return rows


def write_bars(path, rows):
    rows = {k: [*v[:4], clean(v[4])] if len(v) > 4 else v for k, v in rows.items()}
    out = ["# prog\tcompiler\tlevel\tflags\tbytes\tcycles\tinsns\tnote (external results; see prog_bars.py; `bars --refresh` rebuilds; n/a rows: no number, the reason)"]
    for k in sorted(rows):
        out.append("\t".join([*k, *rows[k]]))
    Path(path).write_text("\n".join(out) + "\n")


def flag_text(compiler, level):
    return " ".join(flags(compiler, level)) or "-"


def programs():
    """The pass82.txt programs that are for the 6502 (no `// targets:` line that leaves it out)."""
    names = [x.strip() for x in (HERE / "pass82.txt").read_text().splitlines() if x.strip()]
    def applies(n):
        tags = corpus_targets.tags((CORPUS / f"{n}.c").read_text())
        return tags is None or corpus_targets.applies("6502-sim65", "// targets: " + " ".join(tags) + "\n")
    return [n for n in names if applies(n)]


def record_layouts(text):
    """{record: (sizeof, field lines)} of clang's `-fdump-record-layouts` output: the AST layout blocks, field offsets in bits."""
    found, name, lines = {}, None, []
    for line in text.splitlines():
        if line.startswith("*** Dumping AST Record Layout"):
            name, lines = None, []
        elif name is None and re.match(r"^\s+0 \| (struct|union) ", line):
            name = line.split("|", 1)[1].strip()
        elif name is not None and (m := re.search(r"\[sizeof=(\d+)", line)):
            found[name] = (int(m.group(1)), tuple(lines))
            name = None
        elif name is not None:
            lines.append(re.sub(r"\s+", " ", line.strip()))
    return found


def layout_reasons(ours, theirs):
    """Why two dumps are not the same structs, one `struct S 4 B against 5 B` per record that differs; none when they match."""
    return [f"{k} {ours[k][0]} B here against {theirs[k][0]} B" if ours[k][0] != theirs[k][0] else f"{k} fields at other offsets"
            for k in sorted(ours) if k in theirs and ours[k] != theirs[k]]


def cmd_layouts(a):
    """Write no_bar_layout.tsv: the programs a competitor lays out differently from llrm-cc (its clang front end: msp430 data layout).
    llvm-mos: the two `-fdump-record-layouts` dumps compared; sdcc: a `_Static_assert` of each of our record sizes on the program."""
    rows = []
    for n in programs():
        src = CORPUS / f"{n}.c"
        fl = [f for f in (program_flags(src)[0] if program_flags(src) else []) if f.startswith("-f") and "sanitize" not in f]
        dump = ["-w", "-fsyntax-only", "-Xclang", "-fdump-record-layouts", *fl, str(src)]
        run = subprocess.run([a.cc, "--target=6502-sim65", *dump], capture_output=True, text=True)
        ours = record_layouts(run.stdout + run.stderr)
        mos = subprocess.run([str(MOS / "bin/mos-sim-clang"), "-fno-lto", "-fsigned-char", *dump], capture_output=True, text=True)
        mos = record_layouts(mos.stdout + mos.stderr)
        why = layout_reasons(ours, mos)
        if why:
            rows.append((n, "llvm-mos", "; ".join(why)))
        named = [k for k in ours if re.match(r"(struct|union) \w+$", k)]
        if named and shutil.which("sdcc"):
            asserts = "".join(f'_Static_assert(sizeof({k}) == {ours[k][0]}, "{k}");\n' for k in named)
            work = Path(tempfile.mkdtemp())
            (work / "p.c").write_text(PRELUDE + src.read_text() + "\n" + asserts)
            r = subprocess.run(["sdcc", "-mmos6502", "--std-c11", "-c", "-o", str(work / "p.rel"), str(work / "p.c")], capture_output=True, text=True)
            shutil.rmtree(work, True)
            bad = [k for k in named if f'static assertion failed: "{k}"' in r.stderr]
            if bad:
                rows.append((n, "sdcc", "; ".join(f"{k} is not {ours[k][0]} B there" for k in bad)))
    Path(a.out).write_text("# program\tcompiler\treason (written by `prog_bars.py layouts`; a bar of this program is a bar of another struct layout)\n" +
                           "".join("\t".join(r) + "\n" for r in rows))
    for r in rows:
        print("\t".join(r))


def cmd_bars(a):
    names = a.progs.split(",") if a.progs else programs()
    levels = a.levels.split(",") if a.levels else LEVELS
    rows = load_bars(a.out, no_bar={})
    if not a.progs:
        rows = {k: v for k, v in rows.items() if k[0] in names}
    jobs = []
    for n in names:
        for lv in levels:
            for c in ALL:
                old = rows.get((n, c, lv))
                if a.refresh or old is None or old[0] != flag_text(c, lv):
                    jobs.append((n, c, lv))
    print(f"{len(jobs)} cells to build", flush=True)

    def one(j):
        n, c, lv = j
        b, cy, ins, note = measure(c, lv, n)
        return j, [flag_text(c, lv), "n/a" if b is None else str(b), "n/a" if b is None else str(cy), "-" if ins is None else str(ins), note]
    with cf.ThreadPoolExecutor(a.jobs) as pool:
        for i, (j, row) in enumerate(pool.map(one, jobs)):
            rows[(j[0], j[1], j[2])] = row
            if i % 25 == 0:
                write_bars(a.out, rows)
                print(i, "of", len(jobs), flush=True)
    write_bars(a.out, rows)


# ---- ours and the matrix ---------------------------------------------------------------------------------------------------------

def cmd_ours(a):
    levels = a.levels.split(",") if a.levels else LEVELS
    names = a.progs.split(",") if a.progs else [x.strip() for x in (HERE / "pass82.txt").read_text().splitlines() if x.strip()]
    Path(a.out).mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "OURS_RT": a.rt, "OURS_PADS": "0", "SIM65_INSNS": INSNS}
    if a.rtlib:
        env["OURS_RTLIB"] = a.rtlib
    for lv in levels:
        subprocess.run([sys.executable, str(HERE / "ours_cr.py"), a.cc, lv, str(Path(a.out) / f"ours-{lv}.tsv"), "__void__", *names], env=env, check=True)


def counted(ours_bytes, bars):
    """Counted: ours <= the smallest bar (a tie counts); None when no compiler built the program."""
    have = [b for b in bars if b is not None]
    return None if not have else ours_bytes is not None and ours_bytes <= min(have)


CAVEATS = [
    "ours: ours_cr.py, llrm-cc 6502-sim65 with -fwhole-program, product runtime archive (mkrt.py), one code layout (pad 0); bars: cc65 2.19-2 (Ubuntu package; self-reports V2.18), sdcc 4.5.0, llvm-mos 24.0.0git",
    "bytes: linked image minus the same image with an empty main (+ its rts); BSS not counted. A routine the empty image already links (cc65's pushax) is free to the program, so a bar is never larger than the whole",
    "clocks: run minus the empty-main run on both sides, ours net of crt0's zerobss for its BSS (sim65 zeroes it at 12-15 clk a byte, mos-sim does not); ours, cc65 and sdcc in sim65 -c, llvm-mos in mos-sim --cycles (two simulators); ours' floor is overlay/__void__.c",
    "flags: per-compiler level tables, not tools/external_flags.py (cc65, sdcc and llvm-mos on these targets have no distro frame-pointer, PIE or canary defaults to pin)",
    "levels: cc65 O1 no optimizer, O2 = Os (-O), O3 -Oirs; sdcc O1 = O2 (default), O3 --opt-code-speed, Os --opt-code-size (changes few cells); llvm-mos -O1/-O2/-O3/-Os",
    "llvm-mos bar = -fno-lto as bars.rs; llvm-mos-lto (its default LTO link, prog_main inlined into the driver's main) is a fourth bar, `counted incl. LTO` below uses it too; ours gets -fwhole-program",
    "a cell whose reports differ from the .out is `wrong`, not a bar: cc65 miscompiles by-value structs and mixed-width compound assignment; sdcc has static (non-reentrant) parameters, so recursion is wrong (idiv_loop_os passes with __reentrant), other sdcc wrong cells not isolated",
    "n/a: the compiler cannot build the program (i64, float/double, builtins, bit-field and call-argument limits; sdcc's zero page overflows at 438 B > 256 on mul/div constants); cc65 and sdcc have no __attribute__ and none of clang's predefined macros (a prelude defines them); `-f` flags of the program reach llvm-mos only",
    "programs whose best bar is under ~100 B were folded to constants by llvm-mos (int64_division 62 B): a real gap, not an instrument fault",
]


def cmd_matrix(a):
    import json
    names = programs()
    bars = load_bars(a.bars)
    rows = []
    # per level and bar set: counted, clocks, over-bar list, programs without a bar
    summary = {lv: {k: dict(counted=0, total=0, clk=0, clk_total=0, over=[], clk_over=[], nobar=[]) for k in ("main", "lto")} for lv in LEVELS}
    status = {lv: {} for lv in LEVELS}
    for lv in LEVELS:
        res = {n: (v, d) for n, v, d in json.load(open(Path(a.ours) / f"ours-{lv}.tsv.json"))}
        floor = res["__void__"][1]
        for n in names:
            v, d = res.get(n, ("MISSING", ""))
            if v in ("N/A", "NO-OUT"):  # not a 6502 program (`// targets:`) or a helper: out of the corpus
                continue
            ob = d["bytes"] if v == "PASS" else None
            oc = d["clks"] - floor["clks"] if v == "PASS" else None
            oi = d["insns"] - floor["insns"] if v == "PASS" and d.get("insns") and floor.get("insns") else None
            ext = {c: bars.get((n, c, lv)) for c in ALL}
            best = lambda cs: min((c for c in cs if ext[c] and ext[c][1] != "n/a"), key=lambda c: int(ext[c][1]), default=None)
            rows.append((n, lv, "ours", ob, oc, oi, "" if v == "PASS" else f"{v}: {d}"))
            for c in ALL:
                r = ext[c]
                if r is None:
                    continue
                ok = r[1] != "n/a"
                tag = [t for t, cs in (("best bar by bytes", COMPILERS), ("best bar by bytes incl. LTO", ALL)) if best(cs) == c]
                note = "; ".join(tag[:1] + ([r[4]] if r[4] else [])) if ok else clean(r[4])
                rows.append((n, lv, c, int(r[1]) if ok else None, int(r[2]) if ok else None, int(r[3]) if ok and r[3] != "-" else None, note))
                kind = "ok" if ok else r[4].split(": ")[0] if r[4].startswith("wrong") else "n/a " + (r[4].split(": ")[1] if r[4].startswith("n/a: ") else "")
                status[lv][(c, kind)] = status[lv].get((c, kind), 0) + 1
            for key, cs in (("main", COMPILERS), ("lto", ALL)):
                s = summary[lv][key]
                have = {c: int(ext[c][1]) for c in cs if ext[c] and ext[c][1] != "n/a"}
                clk = {c: int(ext[c][2]) for c in have}
                if not have:
                    s["nobar"].append(n)
                    continue
                s["total"] += 1
                if counted(ob, have.values()):
                    s["counted"] += 1
                else:
                    cb = min(have, key=have.get)
                    s["over"].append((ob - have[cb] if ob is not None else None, n, ob, have[cb], cb))
                if oc is not None:
                    s["clk_total"] += 1
                    if oc <= min(clk.values()):
                        s["clk"] += 1
                    else:
                        cb = min(clk, key=clk.get)
                        s["clk_over"].append((oc / clk[cb], n, oc, clk[cb], cb))
    line = lambda key, what: ", ".join(f"{lv} {summary[lv][key][what]}/{summary[lv][key]['clk_total' if what == 'clk' else 'total']}" for lv in LEVELS)
    head = [f"# 6502 program-corpus counted line, generated {a.date}; release/next {a.tip}",
            f"# counted: {line('main', 'counted')} (programs with at least one bar; ours <= min bar bytes, tie counts)",
            f"# counted incl. llvm-mos LTO bar: {line('lto', 'counted')}",
            f"# clocks (second goal): {line('main', 'clk')} (ours net clocks <= min bar clocks); incl. LTO: {line('lto', 'clk')}",
            *[f"# {c}" for c in CAVEATS]]
    out = head + ["target\tprog\tlevel\tcompiler\tcode_bytes\tclocks\tinstrs\tnote"]
    order = {"ours": 0, **{c: i + 1 for i, c in enumerate(ALL)}}
    rows.sort(key=lambda r: (r[0], LEVELS.index(r[1]), order[r[2]]))
    f = lambda x: "-" if x is None else str(x)
    for n, lv, c, b, cy, ins, note in rows:
        out.append("\t".join(["6502", n, lv, c, f(b), f(cy), f(ins), note]))
    Path(a.out).write_text("\n".join(out) + "\n")
    for lv in LEVELS:
        for key in ("main", "lto"):
            s = summary[lv][key]
            print(f"{lv} {key}: counted {s['counted']}/{s['total']}; clocks {s['clk']}/{s['clk_total']}; no bar {s['nobar']}")
        for (c, kind), k in sorted(status[lv].items()):
            print(f"   {c:13} {kind[:70]:70} {k}")
        s = summary[lv]["main"]
        for ratio, n, oc, bc, c in sorted(s["clk_over"], reverse=True)[:5]:
            print(f"   clocks over: {n} ours {oc} vs {c} {bc} (x{ratio:.2f})")
        for d, n, ob, bb, c in sorted(s["over"], key=lambda x: -(x[0] or 0))[:15]:
            print(f"   over: {n}: ours {ob} vs {c} {bb} (+{d})")


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("bars"); b.add_argument("--refresh", action="store_true"); b.add_argument("--jobs", type=int, default=6)
    b.add_argument("--levels"); b.add_argument("--progs"); b.add_argument("--out", default=str(BARS))
    o = sub.add_parser("ours"); o.add_argument("--cc", required=True); o.add_argument("--rt", required=True); o.add_argument("--rtlib")
    o.add_argument("--out", required=True); o.add_argument("--levels"); o.add_argument("--progs")
    l = sub.add_parser("layouts"); l.add_argument("--cc", required=True); l.add_argument("--out", default=str(NO_BAR))
    m = sub.add_parser("matrix"); m.add_argument("--ours", required=True); m.add_argument("--tip", required=True); m.add_argument("--out", required=True)
    m.add_argument("--bars", default=str(BARS)); m.add_argument("--date", default="2026-11-15")
    a = p.parse_args()
    {"bars": cmd_bars, "ours": cmd_ours, "layouts": cmd_layouts, "matrix": cmd_matrix}[a.cmd](a)
