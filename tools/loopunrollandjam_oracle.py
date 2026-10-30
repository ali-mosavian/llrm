#!/usr/bin/env python3
"""Loop unroll and jam against the pinned `opt -passes=loop-unroll-and-jam`, on the pin's own tests.

    tools/loopunrollandjam_oracle.py [--bin PATH] [--list] [FILE.ll ...]
        (default: llvm/test/Transforms/LoopUnrollAndJam)

Each RUN line of a file that calls `opt` with `-passes=loop-unroll-and-jam` is one
run; its `-allow-unroll-and-jam`, `-unroll-*` and `-da-*` flags go to both sides. Both get the
file through `loop-simplify,lcssa` (the loop pass manager's form). For each
function: the decision first (whether the loops were jammed), then the
whole output, both put in one form (blocks in depth-first order, blocks and
values renamed by place, phi arms sorted, metadata inlined) and compared. A
control, the same run without `-allow-unroll-and-jam` must differ from llrm on some file. Prints
`files total / compared / failed / missing`, then each mismatch. Exit 1 on any
failure or a missing side; `tools/loopunrollandjam_expected.toml` classifies
files whose difference has a stated deviation or reason.

    tools/loopunrollandjam_oracle.py --refresh
        rebuilds crates/opt/llrm-transforms/tests/loopunrollandjam: for each file llrm agrees with the pin on, the
        loop-simplified, LCSSA text llrm reads (FILE.ll) and for each run `== FLAGS` and, per function, the pin's
        output in the one form above (FILE.expected); loopunrollandjam_tests.rs reads them.
"""
import os
import re
import shlex
import struct
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

PINNED = Path.home() / "scratch/llvm-build/bin/opt"
TESTS = Path.home() / "scratch/llvm-src/llvm/test/Transforms/LoopUnrollAndJam"
ROOT = Path(__file__).resolve().parent.parent
LLRM = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug/llrm-mir"
COMMIT = "24.0.0git"
CLASSIFIED = ROOT / "tools/loopunrollandjam_expected.toml"
CORPUS = ROOT / "crates/opt/llrm-transforms/tests/loopunrollandjam"


def run(cmd, stdin=None):
    return subprocess.run(cmd, input=stdin, capture_output=True, text=True)


def functions(ir):
    """The text of each function with a body of `ir`, by name."""
    return {m.group(1): m.group(0) for m in re.finditer(r"^define [^\n]*@\"?([\w.$-]+)\"?\(.*?^}$", ir, re.M | re.S)}


def fp_bits(literal, single=False):
    if literal.startswith("f0x"):
        literal = literal[1:]
    if literal.startswith("0x"):
        return int(literal, 16)
    value = float(literal)
    if single:
        value = struct.unpack("f", struct.pack("f", value))[0]
    return struct.unpack(">Q", struct.pack(">d", value))[0]


def to_mir(text):
    """`text` as printed by opt, in what llrm-mir reads."""
    text = re.sub(r"\bcaptures\(none\)", "nocapture", text)
    text = re.sub(r" captures\([^)]*\)", "", text)
    text = re.sub(r"\bf0x([0-9A-Fa-f]{16})\b", r"0x\1", text)
    text = re.sub(r" nocreateundeforpoison\b", "", text)
    text = re.sub(r"^target triple.*$", "", text, flags=re.M)
    text = re.sub(r"\b(hidden|dso_local|local_unnamed_addr) ", "", text)
    # Attributes MIR does not read.
    text = re.sub(r"\ballocptr\b ?", "", text)
    text = re.sub(r"\balign=\d+", "", text)
    # A group `align=` was all of is empty: it and its uses go.
    for n in re.findall(r"^attributes #(\d+) = \{\s*\}\s*$", text, re.M):
        text = re.sub(rf"^attributes #{n} = \{{\s*\}}\s*$", "", text, flags=re.M)
        text = re.sub(rf"\s#{n}\b", "", text)
    return text


def without_debug_info(text):
    """`text` without debug info: MIR keeps debug records, not DILocation or the DI* nodes."""
    text = re.sub(r"^\s*#dbg_\w+\(.*\)\s*$", "", text, flags=re.M)
    text = re.sub(r"^\s*call void @llvm\.dbg\.\w+\(.*$", "", text, flags=re.M)
    text = re.sub(r"^!\d+ = (distinct )?!DI.*$", "", text, flags=re.M)
    text = re.sub(r"^!llvm\.dbg\.cu = .*$", "", text, flags=re.M)
    text = re.sub(r",? ?!dbg !\d+", "", text)
    # The nodes that named the ones removed go too.
    while True:
        defined = set(re.findall(r"^!(\d+) = ", text, re.M))
        dead = {n for n, body in re.findall(r"^!(\d+) = (.*)$", text, re.M)
                if set(re.findall(r"!(\d+)\b", body)) - defined}
        if not dead:
            break
        text = re.sub(r"^!(\d+) = .*$", lambda m: "" if m.group(1) in dead else m.group(0), text, flags=re.M)
    text = re.sub(r"^declare void @llvm\.dbg\.\w+.*$", "", text, flags=re.M)
    return text


def narrowed(text):
    """The same IR with 32-bit pointers and indices (the Banerjee test of DA stops at 128 bits)."""
    text = re.sub(r"^target datalayout.*$", "", text, flags=re.M)
    text = text.replace("i64", "i32").replace("i128", "i64")
    return 'target datalayout = "e-p:32:32"\n' + text


def run_lines(path):
    """The commands of the RUN lines of `path`, continuations joined."""
    joined, current = [], ""
    for line in Path(path).read_text().splitlines():
        if not line.startswith("; RUN:"):
            continue
        body = line[len("; RUN:"):].strip()
        current += " " + body.rstrip("\\").strip()
        if not body.endswith("\\"):
            joined.append(current.strip())
            current = ""
    return joined


def runs(path):
    """[(opt flags, llrm flags)] for each RUN line that runs loop-unroll-and-jam through opt."""
    found = []
    for command in run_lines(path):
        if " opt " not in " " + command or "-passes=" not in command or "loop-unroll-and-jam" not in command:
            continue
        tokens = shlex.split(command.split("|")[0])
        flags = [t for t in tokens if re.match(r"^-(allow-unroll-and-jam|unroll-and-jam-|unroll-runtime|unroll-partial-threshold|unroll-remainder|da-|aa-pipeline)", t)]
        found.append((flags, [t for t in flags if not t.startswith("-aa-pipeline")]))
    return found


def canonical(ir, nodes=None, body=None):
    """The function `body` of module `ir` in one form, or None if it has none."""
    nodes = nodes if nodes is not None else dict(re.findall(r"^!(\d+) = (.*)$", ir, re.M))
    blocks, order, name = {}, [], None
    entry_open = False
    for raw in (body if body is not None else ir).splitlines():
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
            s = re.sub(r",? ?!llvm\.loop (distinct )?!(\d+|\{[^}]*\})", "", s)  # D2: no loop metadata is made
            s = re.sub(r", !(tbaa|dbg|range|nonnull|align|noundef) !\d+", "", s)
            s = re.sub(r", !llvm\.loop (!\d+|distinct !\{[^}]*\}|![^ ]*)", "", s)  # D2: no loop metadata is made
            s = rename(s)
            m = re.match(r"^(%\w+ = phi \S+ )(.*)$", s)
            if m:
                arms = sorted(re.findall(r"\[ [^\]]* \]", m.group(2)))
                s = m.group(1) + ", ".join(arms)
            out.append("  " + s)
    return "\n".join(out)


def prepared_form(text, binary):
    r = run([binary, "-passes=loop-simplify,lcssa", "-S"], stdin=text)
    return r.stdout if r.returncode == 0 else None


def main():
    args = sys.argv[1:]
    binary = str(PINNED)
    if "--bin" in args:
        at = args.index("--bin")
        binary = args[at + 1]
        del args[at : at + 2]
    listing = "--list" in args
    refresh = "--refresh" in args
    args = [a for a in args if a not in ("--list", "--refresh")]
    kept = {}
    version = run([binary, "--version"]).stdout
    if COMMIT not in version:
        raise SystemExit(f"loopunrollandjam_oracle: {binary} is not LLVM {COMMIT}")
    files = args or sorted(str(p) for p in TESTS.glob("*.ll"))
    classified = tomllib.loads(CLASSIFIED.read_text()) if CLASSIFIED.exists() else {}
    if listing:
        for f in files:
            print(Path(f).name, len(runs(f)))
        return 0
    compared = failed = missing = skipped = total = changed_by_opt = controlled = known = 0
    bad = []
    pinned = set()
    scratch = Path(tempfile.mkdtemp())
    for f in files:
        name = Path(f).name
        found = runs(f)
        if not found:
            skipped += 1
            continue
        entry = classified.get(name, {})
        text = to_mir(Path(f).read_text())
        text = "\n".join(l for l in text.splitlines() if not re.match(r"^;\s*(CHECK|REMARK|NOTE)", l))
        if entry.get("debug"):
            text = without_debug_info(text)
        if entry.get("narrow"):
            text = narrowed(text)
        prepared = prepared_form(text, binary)
        for index, (flags, mine) in enumerate(found):
            label = f"{name}[{index}]"
            if prepared is None:
                missing += 1
                bad.append(f"{label}: opt failed on the file")
                continue
            tmp = scratch / f"{Path(f).stem}.{index}.ll"
            tmp.write_text(to_mir(prepared))
            want = run([binary, "-passes=loop-unroll-and-jam", *flags, "-S"], stdin=prepared)
            got = run([str(LLRM), "--loop-unroll-and-jam", *mine, str(tmp)])
            plain = run([str(LLRM), str(tmp)])
            reason = None if entry.get("narrow") or entry.get("debug") else entry.get("reason")
            if want.returncode != 0 or got.returncode != 0:
                why = "opt failed" if want.returncode else "llrm-mir failed: " + got.stderr.strip()[:200]
                if reason:
                    known += 1
                else:
                    missing += 1
                    bad.append(f"{label}: {why}")
                continue
            compared += 1
            run_ok = True
            nodes = [dict(re.findall(r"^!(\d+) = (.*)$", t, re.M)) for t in (prepared, want.stdout, got.stdout)]
            before_fns, want_fns, got_fns = functions(prepared), functions(want.stdout), functions(got.stdout)
            plain_fns = functions(plain.stdout)
            for fn, body in before_fns.items():
                total += 1
                before = canonical(prepared, nodes[0], body)
                a = canonical(want.stdout, nodes[1], want_fns.get(fn, ""))
                b = canonical(got.stdout, nodes[2], got_fns.get(fn, ""))
                # llrm's own print of the unchanged function: it spells some things (call attributes) its own way.
                own = canonical(plain.stdout, nodes[2], plain_fns.get(fn, ""))
                moved = a != before
                changed_by_opt += moved
                fn_reason = reason or entry.get("functions", {}).get(fn)
                if fn_reason and (a != b or (b != own) != moved):
                    # Pinned to llrm's own output: a change to it shows in the corpus.
                    pinned.add((name, fn))
                if (b != own) != moved:
                    run_ok = False
                    if fn_reason:
                        known += 1
                    else:
                        failed += 1
                        bad.append(f"{label} @{fn}: opt {'jams' if moved else 'leaves'} the loops, llrm {'jams' if b != own else 'leaves'} them")
                elif a != b and (moved or b != own):
                    run_ok = False
                    if fn_reason:
                        known += 1
                    else:
                        failed += 1
                        bad.append(f"{label} @{fn}: output differs\n--- opt\n{a}\n--- llrm\n{b}")
            if (run_ok or any(pin[0] == name for pin in pinned)) and not reason:
                def form(fn):
                    """opt's output for a function it changed, llrm's own print for one it left (call attributes)."""
                    if (name, fn) in pinned:
                        return canonical(got.stdout, nodes[2], got_fns.get(fn, ""))
                    if canonical(want.stdout, nodes[1], want_fns.get(fn, "")) == canonical(prepared, nodes[0], before_fns[fn]):
                        return canonical(plain.stdout, nodes[2], plain_fns.get(fn, ""))
                    return canonical(want.stdout, nodes[1], want_fns.get(fn, ""))
                body = "".join(f"@{fn}\n{form(fn)}\n" for fn in before_fns)
                kept.setdefault(name, (to_mir(prepared), []))[1].append(("== " + " ".join(mine) + "\n", body))
            if True:
                ctl = run([binary, "-passes=loop-unroll-and-jam", *[f for f in flags if f != "-allow-unroll-and-jam"], "-S"], stdin=prepared)
                if ctl.returncode == 0:
                    cfns = functions(ctl.stdout)
                    cn = dict(re.findall(r"^!(\d+) = (.*)$", ctl.stdout, re.M))
                    controlled += any(canonical(ctl.stdout, cn, cfns.get(fn, "")) != canonical(got.stdout, nodes[2], got_fns.get(fn, "")) for fn in before_fns)
    if refresh and not bad:
        CORPUS.mkdir(parents=True, exist_ok=True)
        for old in CORPUS.glob("*"):
            if old.name != "README.md":
                old.unlink()
        for name, (text, outputs) in kept.items():
            (CORPUS / name).write_text(text)
            (CORPUS / (Path(name).stem + ".expected")).write_text("".join(head + body for head, body in outputs))
        print(f"wrote {len(kept)} files to {CORPUS}")
    if changed_by_opt and not controlled and not args:
        failed += 1
        bad.append("the control (profitabilities=ignore) never differed from llrm: the oracle cannot fail")
    print(f"files {len(files)} / compared {compared} / failed {failed} / missing {missing}"
          f"  ({total} functions, opt jammed in {changed_by_opt}, control differed in {controlled}, "
          f"classified {known}, no loop-unroll-and-jam run {skipped})")
    for line in bad:
        print(line)
    return 1 if failed or missing else 0


sys.exit(main())
