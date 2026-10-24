"""Which in-loop instructions the pin's early MachineLICM removes at m16, by kind (docs/working/early-machinelicm.md).

    mlicm_kinds.py LLRM_C LEVEL MODULE...      (QCport module names, e.g. mdl_ai cmd sc ent weapons)

Per function, the pin's `llc -O2` with and without `-disable-machine-licm` on our IR; an instruction line (AT&T, operands as printed) is
in a loop when it lies between a label and a later branch back to it. Prints, per kind (the mnemonic and the shape of the operands), the
in-loop instructions without the pass less those with it. A text diff of the two listings was tried first: register and label numbers
differ throughout, so it reports a gross 1765 where the net is 754.
"""
import collections, glob, re, shutil, subprocess, sys, tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pin

OURS, LV, MODULES = sys.argv[1], sys.argv[2], sys.argv[3:]
Q = Path.home() / "scratch/qcport/src"
DIRS = ("host", "render", "model", "game", "sound", "ui")
INC = [x for d in (*DIRS, "qgl") for x in ("-I", str(Q / d))] + ["-I", str(Path.home() / "scratch/qctc/inc")]
of = pin.target("m16", optsize=(LV == "-Os"))


def functions(asm):
    """name -> instruction lines (labels kept as 'label:')."""
    out, cur = {}, None
    for line in asm.splitlines():
        s = line.split("#")[0].strip()
        m = re.match(r'"?([A-Za-z_][\w@$.]*)"?:$', s)
        if m and not s.startswith(".L"):
            cur = m.group(1)
            out[cur] = []
            continue
        if cur is None or not s or s.startswith(";"):
            continue
        if s.startswith(".") and not re.match(r"\.[\w$@.]+:$", s):
            continue
        out[cur].append(s)
    return out


def loop_lines(lines):
    pos = {}
    for i, l in enumerate(lines):
        m = re.match(r"([\w.$@?]+):$", l)
        if m:
            pos[m.group(1)] = i
    inside = set()
    for i, l in enumerate(lines):
        m = re.match(r"j\w+\s+([\w.$@?]+)", l)
        if m and m.group(1) in pos and pos[m.group(1)] <= i:
            inside.update(range(pos[m.group(1)], i + 1))
    return [lines[i] for i in sorted(inside) if not re.match(r"[\w.$@?]+:$", lines[i])]


def kind(l):
    op, _, rest = l.replace("\t", " ").partition(" ")
    rest = rest.strip()
    if op.startswith("lea"):
        return "address (lea)"
    if re.match(r"mov[wlb]?$", op) and re.match(r"\$[A-Za-z_]", rest):
        return "address (mov $symbol)"
    if re.match(r"mov[wlb]?$", op) and re.match(r"\$-?\d|\$0x", rest):
        return "constant (mov $imm)"
    if op.startswith("xor") and len(set(re.findall(r"%\w+", rest))) == 1:
        return "constant (xor r,r)"
    if re.match(r"mov", op) and re.search(r"\(|^[A-Za-z_]\w*[,]|[\w]\(%", rest.split(",")[0]) and not rest.split(",")[-1].strip().startswith(("%es:", "(")):
        return "load"
    if re.match(r"mov", op) and ("(" in rest.split(",")[0] or "%es:" in rest.split(",")[0] or "%ds:" in rest.split(",")[0]):
        return "load"
    if re.match(r"mov", op):
        return "move (reg/other)"
    return "other (" + op + ")"


def one(module):
    src = next(Q.glob(f"*/{module}.c"))
    w = tempfile.mkdtemp(dir=str(Path.home() / "scratch/w3-mlicm"))
    try:
        r = subprocess.run([OURS, "-m16", LV, "-S", "--dump", w, *INC, "-o", w + "/o.s", str(src)], capture_output=True, text=True)
        st = glob.glob(w + "/*-switchconv.ll")
        ir = re.sub(r" tre-deferred| threeway", "", open(st[0]).read())
        a = functions(pin.llc(["-O2", "-filetype=asm", "-o", "-"], ir, of).stdout)
        b = functions(pin.llc(["-O2", "-filetype=asm", "-o", "-", "-disable-machine-licm"], ir, of).stdout)
        net = collections.Counter()
        for name, lines in b.items():
            for text in loop_lines(lines):
                net[kind(text)] += 1
            for text in loop_lines(a.get(name, [])):
                net[kind(text)] -= 1
        return net
    finally:
        shutil.rmtree(w, ignore_errors=True)


total = collections.Counter()
for module in MODULES:
    net = one(module)
    print(f"{module}: {sum(net.values())} fewer in loops with the pass:", dict(net.most_common(5)))
    total.update(net)
print("all:", sum(total.values()), dict(total.most_common(8)))
