#!/usr/bin/env python3
"""Loop unswitching against the pinned `opt -passes='loop-mssa(simple-loop-unswitch<no-nontrivial;trivial>)'`, or with
`--non-trivial` `simple-loop-unswitch<nontrivial;trivial>`.

    tools/simpleloopunswitch_oracle.py [--rotated] [--non-trivial] [--keep DIR] [FILE.ll ...]    (default: tests/fixtures/slu and the analysis corpus)

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
import targetdesc
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

REQUIRED_NON_TRIVIAL = {"nt_branch", "nt_switch", "nt_and", "nt_or", "nt_select", "nt_exit_phi", "nt_nested"}
REQUIRED = {"header_exit", "variant_stays", "partial_or", "partial_and", "switch_exit", "switch_default_exit", "latch_exit", "exit_phi_variant", "stores_first", "nested_hoist", "shared_exit", "second_branch", "constant_cond"}
PINNED = Path.home() / "scratch/llvm-build/bin/opt"
TARGET = targetdesc.triple_cpu()  # llrm's default target (crates/target/*/datalayout.toml): a dump has no `target triple`, and opt costs for x86-64 without one
COMMIT = "24.0.0git"
MODE = targetdesc.default()[1:]  # opt prices for this target (TARGET), so llrm-mir does too
ROOT = Path(__file__).resolve().parent.parent
LLRM = ROOT / "target/debug/llrm-mir"
ROTATED = "--rotated" in sys.argv
NON_TRIVIAL = "--non-trivial" in sys.argv
KEEP = Path(sys.argv[sys.argv.index("--keep") + 1]) if "--keep" in sys.argv else None  # writes each file's input, opt's and llrm's output there
ARGS = [a for a in sys.argv[1:] if a not in ("--rotated", "--non-trivial", "--keep") and (KEEP is None or a != str(KEEP))]
# The control is the same run without the half under test: it must differ from llrm where opt changes the loop.
PASS = "loop-mssa(simple-loop-unswitch<nontrivial;trivial>)" if NON_TRIVIAL else "loop-mssa(simple-loop-unswitch<no-nontrivial;trivial>)"
CONTROL = "loop-mssa(simple-loop-unswitch<no-nontrivial;trivial>)" if NON_TRIVIAL else "loop-mssa(simple-loop-unswitch<no-nontrivial;no-trivial>)"


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

    return "\n".join(exact(line) for line in text.splitlines() if not line.startswith("target triple")) + "\n"


def node_text(nodes, number, depth=0):
    """Metadata node `number` with the nodes it names written out."""
    text = nodes.get(number, "?")
    if depth > 6:
        return text
    return re.sub(r"!(\d+)", lambda m: "{" + node_text(nodes, m.group(1), depth + 1) + "}", text)


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
            # A node's number depends on what else the module holds (opt adds the loop id of an unswitched loop), so compare its content.
            s = re.sub(r"!tbaa !(\d+)", lambda m: "!tbaa " + node_text(nodes, m.group(1)), s)
            s = rename(s)
            m = re.match(r"^(%\w+ = phi \S+ )(.*)$", s)
            if m:
                arms = sorted(re.findall(r"\[ [^\]]* \]", m.group(2)))
                s = m.group(1) + ", ".join(arms)
            out.append("  " + s)
    return "\n".join(out)


def marked_partial(body, nodes):
    """Whether a loop of `body` carries the mark opt leaves on a partially invariant condition it unswitched (nodes follow `!N` references)."""
    pending, seen = re.findall(r"!llvm\.loop !(\d+)", body), set()
    while pending:
        node = pending.pop()
        if node in seen:
            continue
        seen.add(node)
        text = nodes.get(node, "")
        if "llvm.loop.unswitch.partial.disable" in text:
            return True
        pending += re.findall(r"!(\d+)", text)
    return False


def llrm_command(path):
    """llrm's run of `path`: at the target opt priced for, or the loops differ in cost and in decision."""
    return [str(LLRM), "--simple-loop-unswitch", *(["--non-trivial"] if NON_TRIVIAL else []), "--mode", MODE, str(path)]


def opt_unswitched(ir, passes=PASS):
    r = run([str(PINNED), *TARGET, f"-passes={passes}", "-S"], stdin=ir)
    return r.stdout if r.returncode == 0 else None


def main():
    version = run([str(PINNED), "--version"]).stdout
    if COMMIT not in version:
        raise SystemExit(f"simpleloopunswitch_oracle: {PINNED} is not LLVM {COMMIT}")
    print([l.strip() for l in version.splitlines() if "version" in l][0], f"(pinned {PINNED})")
    files = ARGS or sorted(
        glob.glob(str(ROOT / "tests/fixtures/slu/*.ll")) + glob.glob(str(ROOT / "crates/opt/llrm-analysis/corpus/*/*.ll"))
    )
    compared = failed = missing = classified = 0
    bad = []
    if not ARGS:
        absent = (REQUIRED_NON_TRIVIAL if NON_TRIVIAL else REQUIRED) - {Path(f).stem for f in files if "fixtures/slu" in f}
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
        prepared = run([str(PINNED), *TARGET, "-passes=loop-simplify,lcssa" + (",loop-rotate" if ROTATED else ""), "-S"], stdin=ir).stdout
        tmp = Path(tempfile.mkdtemp()) / "prepared.ll"
        tmp.write_text(to_mir(prepared))
        want = opt_unswitched(prepared)
        got = run(llrm_command(tmp))
        if want is None or got.returncode != 0:
            missing += 1
            bad.append(f"{name}: " + ("opt failed" if want is None else "llrm-mir failed: " + got.stderr.strip()[:300]))
            continue
        compared += 1
        if KEEP:
            KEEP.mkdir(parents=True, exist_ok=True)
            (KEEP / f"{name}.in.ll").write_text(to_mir(prepared))
            (KEEP / f"{name}.opt.ll").write_text(want)
            (KEEP / f"{name}.llrm.ll").write_text(got.stdout)
        nodes = [dict(re.findall(r"^!(\d+) = (.*)$", text, re.M)) for text in (prepared, want, got.stdout)]
        control = None
        before_fns, want_fns, got_fns = functions(prepared), functions(want), functions(got.stdout)
        for fn, text in before_fns.items():
            total += 1
            # SU9: opt unswitches a condition MemorySSA shows invariant (`hasPartialIVCondition`) and marks the loop; llrm has no MemorySSA here.
            if NON_TRIVIAL and marked_partial(want_fns.get(fn, ""), nodes[1]):
                classified += 1
                continue
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
          f"  ({total} functions, opt unswitched in {unswitched_by_opt}, control differed in {controlled} files, {classified} classified SU9)")
    for line in bad:
        print(line)
    return 1 if failed or missing else 0


if __name__ == "__main__":
    sys.exit(main())
