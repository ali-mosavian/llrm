#!/usr/bin/env python3
"""Trivial loop unswitching against the pinned `opt -passes='loop-mssa(simple-loop-unswitch<no-nontrivial;trivial>)'`.

    tools/simpleloopunswitch_oracle.py [--rotated] [FILE.ll ...]    (default: tests/fixtures/slu and the analysis corpus)

A file holds functions; both sides get each through `loop-simplify,lcssa`, as the loop pass manager does. Both outputs are put in one form (blocks in
depth-first order from the entry, blocks and values renamed by place, phi inputs
sorted, metadata inlined) and must be equal. The first check is the decision: a
file whose loop opt unswitches must be unswitched by llrm-mir --simple-loop-unswitch, and the other
way round. A control runs opt without trivial unswitching, which must differ
from llrm on a file opt unswitches: an oracle that cannot fail says nothing. The
corpus must hold each shape the translation claims (REQUIRED). Prints
`files total / compared / failed / missing` first, then each mismatch. Exit 1
on any failure or a missing side.
"""
import glob
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

REQUIRED = {"header_exit", "variant_stays", "partial_or", "partial_and", "switch_exit", "switch_default_exit", "latch_exit", "exit_phi_variant", "stores_first", "nested_hoist", "shared_exit", "second_branch", "constant_cond"}
PINNED = Path.home() / "scratch/llvm-build/bin/opt"
COMMIT = "24.0.0git"
ROOT = Path(__file__).resolve().parent.parent
LLRM = ROOT / "target/debug/llrm-mir"
PASS = "loop-mssa(simple-loop-unswitch<no-nontrivial;trivial>)"
CONTROL = "loop-mssa(simple-loop-unswitch<no-nontrivial;no-trivial>)"

ROTATED = "--rotated" in sys.argv
ARGS = [a for a in sys.argv[1:] if a != "--rotated"]


def run(cmd, stdin=None):
    return subprocess.run(cmd, input=stdin, capture_output=True, text=True)


def functions(ir):
    """The text of each function with a body of `ir`, by name."""
    return {m.group(1): m.group(0) for m in re.finditer(r"^define [^\n]*@\"?([\w.$-]+)\"?\(.*?^}$", ir, re.M | re.S)}


def fp_bits(literal, single=False):
    """The 64 bits of a double literal as opt or llrm prints it; of a float's value when `single`."""
    if literal.startswith("f0x"):
        literal = literal[1:]
    if literal.startswith("0x"):
        return int(literal, 16)
    value = float(literal)
    if single:
        value = struct.unpack("f", struct.pack("f", value))[0]
    return struct.unpack(">Q", struct.pack(">d", value))[0]


def to_mir(text):
    """`text` as printed by opt, in what llrm-mir reads: attributes opt adds or spells anew."""
    text = re.sub(r"\bcaptures\(none\)", "nocapture", text)
    text = re.sub(r" captures\([^)]*\)", "", text)
    text = re.sub(r"\bf0x([0-9A-Fa-f]{16})\b", r"0x\1", text)
    text = re.sub(r" nocreateundeforpoison\b", "", text)

    def exact(line):
        # A decimal `float` literal opt prints rounds to the float; llrm reads only exact ones.
        if " float " not in line or " double " in line:
            return line
        return re.sub(
            r"(?<![\w.%@])-?\d+\.\d+e[+-]\d+\b",
            lambda m: "0x%016X" % fp_bits("0x%016X" % struct.unpack(">Q", struct.pack(">d", struct.unpack("f", struct.pack("f", float(m.group(0))))[0]))[0]),
            line,
        )

    return "\n".join(exact(line) for line in text.splitlines()) + "\n"


def canonical(ir, nodes=None, body=None):
    """The function `body` of module `ir` in one form, or None if it has none."""
    nodes = nodes if nodes is not None else dict(re.findall(r"^!(\d+) = (.*)$", ir, re.M))
    blocks, order, name = {}, [], None
    entry_open = False
    # A switch's cases follow it on lines of their own: one statement.
    text = re.sub(r"\[\s*\n((?:[^\]]*\n)*?)\s*\]", lambda m: "[ " + " ".join(l.strip() for l in m.group(1).splitlines()) + " ]", body if body is not None else ir)
    for raw in text.splitlines():
        line = raw.split(";")[0].rstrip()
        s = line.strip()
        if not s or s.startswith(("define", "declare", "target", "source_filename", "!", "attributes")):
            if s.startswith("define"):
                entry_open = True
            continue
        if s == "}":
            break
        m = re.match(r"^([\w.$-]+):$", s)
        if m:
            name = m.group(1)
            blocks[name] = []
            order.append(name)
            entry_open = False
            continue
        if entry_open and name is None:
            name = "%entry"
            blocks[name] = []
            order.append(name)
            entry_open = False
        blocks[name].append(s)
    if not order:
        return None

    def successors(label):
        end = blocks[label][-1]
        return list(dict.fromkeys(re.findall(r"label %([\w.$-]+)", end)))

    seen, dfs, stack = set(), [], [order[0]]
    while stack:
        label = stack.pop()
        if label in seen:
            continue
        seen.add(label)
        dfs.append(label)
        stack.extend(reversed(successors(label)))
    names = {label: f"B{n}" for n, label in enumerate(dfs)}
    values = {}
    for label in dfs:
        for s in blocks[label]:
            m = re.match(r"^%([\w.$-]+) = ", s)
            if m:
                values[m.group(1)] = f"v{len(values)}"

    def rename(text):
        text = re.sub(r"label %([\w.$-]+)", lambda m: "label %" + names.get(m.group(1), m.group(1)), text)
        text = re.sub(r"\[ ([^,\]]+), %([\w.$-]+) \]", lambda m: f"[ {m.group(1)}, %{names.get(m.group(2), m.group(2))} ]", text)
        text = re.sub(r"%([\w.$-]+)", lambda m: "%" + (values.get(m.group(1)) or names.get(m.group(1)) or m.group(1)), text)
        return text

    out = []
    for label in dfs:
        out.append(names[label] + ":")
        for s in blocks[label]:
            s = re.sub(r", align \d+", "", s)
            s = re.sub(r'@"([^"]*)"', r"@\1", s)
            s = re.sub(r"\bcc (\d+)", r"cc\1", s)
            s = re.sub(r"(?<![\w.%@])(f?0x[0-9A-Fa-f]{16}|-?\d+\.\d+e[+-]\d+)\b", lambda m: "fp(%016X)" % fp_bits(m.group(1), " float " in s and " double " not in s), s)
            s = re.sub(r"!prof !(\d+)", lambda m: "!prof " + nodes.get(m.group(1), "?"), s)
            s = re.sub(r"!llvm.loop !(\d+)", lambda m: "!llvm.loop " + nodes.get(m.group(1), "?"), s)
            s = rename(s)
            m = re.match(r"^(%\w+ = phi \S+ )(.*)$", s)
            if m:
                arms = sorted(re.findall(r"\[ [^\]]* \]", m.group(2)))
                s = m.group(1) + ", ".join(arms)
            out.append("  " + s)
    return "\n".join(out)


def opt_unswitched(ir, passes=PASS):
    r = run([str(PINNED), f"-passes={passes}", "-S"], stdin=ir)
    return r.stdout if r.returncode == 0 else None


def main():
    version = run([str(PINNED), "--version"]).stdout
    if COMMIT not in version:
        raise SystemExit(f"simpleloopunswitch_oracle: {PINNED} is not LLVM {COMMIT}")
    print([l.strip() for l in version.splitlines() if "version" in l][0], f"(pinned {PINNED})")
    files = ARGS or sorted(
        glob.glob(str(ROOT / "tests/fixtures/slu/*.ll")) + glob.glob(str(ROOT / "crates/opt/llrm-analysis/corpus/*/*.ll"))
    )
    compared = failed = missing = 0
    bad = []
    if not ARGS:
        absent = REQUIRED - {Path(f).stem for f in files if "fixtures/slu" in f}
        if absent:
            failed += len(absent)
            bad.append(f"the corpus misses shapes: {sorted(absent)}")
    unswitched_by_opt = controlled = 0
    total = 0
    for f in files:
        name = Path(f).stem
        ir = Path(f).read_text()
        # The loop pass manager hands its passes loops in simplified, LCSSA form;
        # both sides start from the same one.
        prepared = run([str(PINNED), "-passes=loop-simplify,lcssa" + (",loop-rotate" if ROTATED else ""), "-S"], stdin=ir).stdout
        tmp = Path(tempfile.mkdtemp()) / "prepared.ll"
        tmp.write_text(to_mir(prepared))
        want = opt_unswitched(prepared)
        got = run([str(LLRM), "--simple-loop-unswitch", str(tmp)])
        if want is None or got.returncode != 0:
            missing += 1
            bad.append(f"{name}: " + ("opt failed" if want is None else "llrm-mir failed: " + got.stderr.strip()[:300]))
            continue
        compared += 1
        nodes = [dict(re.findall(r"^!(\d+) = (.*)$", text, re.M)) for text in (prepared, want, got.stdout)]
        control = None
        before_fns, want_fns, got_fns = functions(prepared), functions(want), functions(got.stdout)
        for fn, text in before_fns.items():
            total += 1
            before = canonical(prepared, nodes[0], text)
            a = canonical(want, nodes[1], want_fns.get(fn, ""))
            b = canonical(got.stdout, nodes[2], got_fns.get(fn, ""))
            changed = a != before
            unswitched_by_opt += changed
            if (b != before) != changed:
                failed += 1
                bad.append(f"{name} @{fn}: opt {'unswitches' if changed else 'leaves'} the loop, llrm {'unswitches' if b != before else 'leaves'} it")
            elif a != b:
                failed += 1
                bad.append(f"{name} @{fn}: output differs\n--- opt\n{a}\n--- llrm\n{b}")
        if any(canonical(want, nodes[1], want_fns.get(fn, "")) != canonical(prepared, nodes[0], t) for fn, t in before_fns.items()):
            control = opt_unswitched(prepared, CONTROL)
            if control is not None:
                cfns = functions(control)
                cn = dict(re.findall(r"^!(\d+) = (.*)$", control, re.M))
                controlled += any(canonical(control, cn, cfns.get(fn, "")) != canonical(got.stdout, nodes[2], got_fns.get(fn, "")) for fn in before_fns)
    if unswitched_by_opt and not controlled and not ARGS:
        failed += 1
        bad.append("the control (no trivial unswitching) never differed from llrm: the oracle cannot fail")
    print(f"files {len(files)} / compared {compared} / failed {failed} / missing {missing}"
          f"  ({total} functions, opt unswitched in {unswitched_by_opt}, control differed in {controlled} files)")
    for line in bad:
        print(line)
    return 1 if failed or missing else 0


sys.exit(main())
