#!/usr/bin/env python3
"""Compares ConstantFoldInstruction's answers, LLVM's against llrm's, on .ll files.

    tools/llvmport/cfcompare.py PATH... [--show N] [--verbose] [--flat]
    tools/llvmport/cfcompare.py --write-expected FILE.ll...

PATH is a .ll file or a directory (every `.ll` under it, or with --flat only its own); the totals are per PATH. A file either side cannot read is
skipped and counted. LLVM's side is `constfold_dump.sh`, llrm's the `cfdump` example of llrm-mir (CARGO_TARGET_DIR is
honoured). Each differing line is one of: MORE (llrm folds, LLVM does not), DIFF (both fold, to different constants), or a
LESS class (LLVM folds, llrm does not), named by opcode and the kind of constant LLVM made. A difference only in the
`nusw`/`nuw` flags of a constant GEP is class `gep-flags` (MIR keeps `inbounds` only). Exit 1 on a MORE or a DIFF.
"""
import os
import re
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def run(cmd):
    done = subprocess.run(cmd, capture_output=True, text=True)
    return done.returncode, done.stdout.splitlines(), done.stderr


def normalized(file, scratch):
    """`file` without its `target triple` line and with `undef` as `poison` (MIR has neither), both sides read this copy."""
    text = Path(file).read_text(errors="replace")
    text = "\n".join(line for line in text.split("\n") if not line.startswith("target triple"))
    copy = Path(scratch) / (str(abs(hash(file))) + ".ll")
    copy.write_text(re.sub(r"\bundef\b", "poison", text))
    return str(copy)


def kind(answer):
    """The kind of constant: its type class and shape, for naming a LESS class."""
    ty, _, rest = answer.partition(" ")
    if answer.startswith("<") or "<" in ty:
        return "vector"
    if re.match(r"(float|double|half|bfloat|x86_fp80|fp128)\b", answer):
        return "fp"
    shape = rest.split(" ")[0].split("[")[0].split("(")[0]
    if shape.startswith("gep"):
        return "gep"
    return f"{ty}:{shape}" if shape in ("poison", "null", "zeroinitializer") else (
        shape if shape.startswith(("inttoptr", "ptrtoint", "bitcast", "trunc", "addrspacecast", "@", "expr:")) else ty
    )


def gep_flags_only(llvm, ours):
    return re.sub(r"gep\[[a-z,]*\]", lambda m: "gep[" + ("inbounds" if "inbounds" in m.group(0) else "") + "]", llvm) == ours


def write_expected(files):
    """Beside each FILE.ll: FILE.expected (LLVM's lines) and FILE.deviates (`<function> <n> <class>` of each line llrm
    answers differently), for `constantfolding_tests`. A MORE or a DIFF is refused."""
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    for file in files:
        code, theirs, err = run([str(ROOT / "tools/llvmport/constfold_dump.sh"), file])
        code2, ours, err2 = run([str(target / "release/examples/cfdump"), file])
        if code or code2 or len(ours) != len(theirs):
            sys.exit(f"{file}: {err or err2}")
        deviates = []
        for a, b in zip(ours, theirs):
            if a == b:
                continue
            fn, n, opcode, answer = b.split(" ", 3)
            if a.split(" ", 3)[3] == "-":
                label = f"{opcode} {kind(answer)}"
            elif gep_flags_only(answer, a.split(" ", 3)[3]):
                label = "gep-flags"
            else:
                sys.exit(f"{file}: {a} against {b}")
            deviates.append(f"{fn} {n} {label}")
        Path(file).with_suffix(".expected").write_text("\n".join(theirs) + "\n")
        Path(file).with_suffix(".deviates").write_text("".join(d + "\n" for d in deviates))
    return 0


def main():

    args = sys.argv[1:]
    show = 20
    if "--show" in args:
        at = args.index("--show")
        show = int(args[at + 1])
        del args[at : at + 2]
    verbose = "--verbose" in args
    flat = "--flat" in args
    paths = [a for a in args if not a.startswith("--")]
    if "--write-expected" in args:
        return write_expected(paths)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    ours_bin = target / "release/examples/cfdump"
    scratch = tempfile.mkdtemp()
    classes = Counter()
    examples = {}
    failed = False
    for path in paths:
        files = sorted(str(f) for f in (Path(path).glob("*.ll") if flat else Path(path).rglob("*.ll"))) if Path(path).is_dir() else [path]
        total = equal = skipped = 0
        counts = Counter()
        for original in files:
            file = normalized(original, scratch)
            code, theirs, _ = run([str(ROOT / "tools/llvmport/constfold_dump.sh"), file])
            if code:
                skipped += 1
                continue
            code, ours, err = run([str(ours_bin), file])
            if code:
                skipped += 1
                if verbose:
                    print(f"{original}: llrm failed: {(err.strip().splitlines() + [""])[1] if err.strip() else code}")
                continue
            if len(ours) != len(theirs):
                print(f"{original}: {len(ours)} lines here, {len(theirs)} in LLVM")
                counts["count"] += 1
                failed = True
                continue
            for a, b in zip(ours, theirs):
                total += 1
                if a == b:
                    equal += 1
                    continue
                head_a, _, ans_a = a.split(" ", 3)[:3], None, a.split(" ", 3)[3]
                head_b, ans_b = b.split(" ", 3)[:3], b.split(" ", 3)[3]
                opcode = head_b[2]
                if ans_b == "-":
                    label = "MORE"
                elif ans_a == "-":
                    label = f"LESS {opcode} {kind(ans_b)}"
                elif gep_flags_only(ans_b, ans_a):
                    label = "LESS gep-flags"
                else:
                    label = f"DIFF {opcode}"
                counts[label.split(" ")[0]] += 1
                classes[(path, label)] += 1
                examples.setdefault((path, label), (original, a, b))
                if label.split(" ")[0] in ("MORE", "DIFF"):
                    failed = True
                    if counts[label.split(" ")[0]] <= show:
                        print(f"{original}\n  llrm: {a}\n  llvm: {b}")
        print(
            f"{path}: {total} compared, {equal} equal, {counts['LESS']} less, {counts['MORE']} more, "
            f"{counts['DIFF']} diff, {skipped} of {len(files)} files skipped"
        )
    for (path, label), count in sorted(classes.items(), key=lambda kv: -kv[1]):
        if label.startswith(("LESS",)):
            file, a, b = examples[(path, label)]
            print(f"  {count:5} {Path(path).name} {label}\n        e.g. {b}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
