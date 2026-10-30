#!/usr/bin/env python3
"""DependenceAnalysis against the pinned `opt -passes='print<da>'`.

    tools/da_oracle.py [--bin PATH] [--list] [FILE.ll ...]
        (default: the pin's own tests, llvm/test/Analysis/DependenceAnalysis)
    tools/da_oracle.py --refresh
        rebuilds crates/opt/llrm-analysis/tests/da: the pin's tests without their CHECK
        lines, and 32-bit-pointer copies of those whose Banerjee runs need more than
        128 bits at 64; a file is kept only where every run agrees, with the pin's
        answers in FILE.expected (what depanalysis_tests.rs reads)

The options of each file's first `opt` RUN line (`-passes=print<da<...>>`,
`-da-*`) are given to both sides. Compared, per function and per pair in
order: the answer line (`da analyze - flow [<]!`) and the runtime assumptions
under it; the `Src:`/`Dst:` text is each side's own printing of the
instruction. Prints `files total / compared / failed / missing`, then each
mismatch. Exit 1 on any failure or a missing side.
"""
import os
import re
import shlex
import subprocess
import sys
from pathlib import Path

PINNED = Path.home() / "scratch/llvm-build/bin/opt"
TESTS = Path.home() / "scratch/llvm-src/llvm/test/Analysis/DependenceAnalysis"
ROOT = Path(__file__).resolve().parent.parent
LLRM = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug/llrm-mir"
HEADER = "Printing analysis 'Dependence Analysis' for function '"


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


def opt_flags(path):
    """The flags of each RUN line that calls opt with print<da>, as [(opt flags, llrm flags)]."""
    text, lines = Path(path).read_text(), []
    for line in text.splitlines():
        if line.startswith("; RUN:"):
            lines.append(line[len("; RUN:"):].strip())
    joined, current = [], ""
    for line in lines:
        current += " " + line.rstrip("\\").strip()
        if not line.endswith("\\"):
            joined.append(current.strip())
            current = ""
    found = []
    for command in joined:
        if " opt " not in " " + command or "print<da" not in command:
            continue
        tokens = shlex.split(command.split("|")[0].replace("2>&1", ""))
        flags, mine, normalize, tbaa = [], [], False, True
        for token in tokens:
            if token.startswith("-passes=") and "print<da" in token:
                flags.append(token)
                normalize = "normalized-results" in token
            elif token.startswith("-da-") or token.startswith("-aa-pipeline="):
                flags.append(token)
                if token.startswith("-aa-pipeline="):
                    tbaa = "tbaa" in token or token.endswith("=default")
                if token == "-da-disable-delinearization-checks":
                    mine.append("--disable-delinearization-checks")
                elif token in ("-da-delinearize=false", "-da-delinearize=0"):
                    mine.append("--no-delinearize")
                elif token.startswith("-da-miv-max-level-threshold="):
                    mine += ["--miv-max-level", token.split("=")[1]]
                elif token.startswith("-da-enable-dependence-test="):
                    mine += ["--test", token.split("=")[1]]
        if normalize:
            mine.append("--normalize")
        if tbaa:
            mine.append("--tbaa")
        found.append((flags, mine))
    return found


def sanitized(path, scratch):
    """The file for llrm: MIR's subset of LLVM takes no `target triple` and no `dso_local`."""
    text = Path(path).read_text()
    text = re.sub(r"^target triple.*$", "", text, flags=re.M)
    text = text.replace("dso_local ", "")
    text = "\n".join(line if line.lstrip().startswith(";") else re.sub(r"\s#\d+\b", "", line) for line in text.splitlines())
    out = scratch / Path(path).name
    out.write_text(text)
    return out


def parse(text):
    """{function: [answer, ...]}: the `da analyze` lines, with the runtime assumptions that follow."""
    out, function, current = {}, None, None
    for line in text.splitlines():
        if line.startswith(HEADER):
            function = line[len(HEADER):].split("'")[0].lstrip("@")
            out[function] = []
            current = None
        elif function is None:
            continue
        elif line.startswith("  da analyze - "):
            current = [line[len("  da analyze - "):]]
            out[function].append(current)
        elif current is not None and line.startswith("  "):
            current.append(line)
    return out


CORPUS = ROOT / "crates/opt/llrm-analysis/tests/da"
# Files that carry Banerjee runs at 64-bit pointers (D1): also kept with 32-bit pointers.
NARROW = ["Banerjee", "PR51512", "banerjee-single-iteration", "run-specific-dependence-test"]


def stripped(text):
    """The file without the lines the pin checks its output with."""
    return "\n".join(
        line for line in text.splitlines()
        if not re.match(r"^;\s*(CHECK|NORMALIZE|DELIN|LIN)[A-Z0-9_-]*(-NEXT|-LABEL|-DAG|-NOT)?:", line)
        and not line.startswith("; NOTE:")
    ) + "\n"


def narrowed(text):
    """The same IR with 32-bit pointers and indices."""
    text = re.sub(r"^target datalayout.*$", "", text, flags=re.M)
    text = text.replace("i64", "i32").replace("i128", "i64")
    return 'target datalayout = "e-p:32:32"\n' + text


def expected(path, runs, binary):
    """The pin's answers for each run, or None where llrm differs from it on one."""
    out = ""
    for flags, mine in runs:
        want = run([binary, "-disable-output", *flags, str(path)])
        got = run([str(LLRM), "--da", *mine, str(path)])
        if want.returncode or got.returncode or parse(want.stderr) != parse(got.stdout):
            return None
        out += "== " + " ".join(mine) + "\n"
        for function, answers in parse(want.stderr).items():
            out += f"@{function}\n" + "".join("".join(f"  {line}\n" for line in answer) for answer in answers)
    return out


def refresh(binary):
    CORPUS.mkdir(parents=True, exist_ok=True)
    for old in CORPUS.glob("*"):
        old.unlink()
    scratch = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "da-oracle"
    scratch.mkdir(parents=True, exist_ok=True)
    kept, dropped = [], []
    for source in sorted(TESTS.glob("*.ll")):
        for name, text in [(source.name, source.read_text())] + (
            [("narrow-" + source.name, narrowed(source.read_text()))] if source.stem in NARROW else []
        ):
            target = CORPUS / name
            target.write_text(stripped(text))
            runs = opt_flags(target)
            clean = sanitized(target, scratch)
            target.write_text(clean.read_text())
            answers = expected(target, runs, binary) if runs else None
            if answers is None:
                target.unlink()
                dropped.append(name)
                continue
            target.with_suffix(".expected").write_text(answers)
            kept.append(name)
    (CORPUS / "README.md").write_text(
        "The pin's DependenceAnalysis tests (llvm/test/Analysis/DependenceAnalysis) without their CHECK lines, and\n"
        "their `.expected` answers from `opt -passes=print<da>` for each RUN line, in order (`== OPTIONS`, then per function\n"
        "`@name` and each pair's answer). `tools/da_oracle.py --refresh` writes them; a file is kept only where every run\n"
        "agrees with llrm.\n\nNot kept (a Banerjee run at 64-bit pointers needs more than 128 bits, DEVIATE D1; a range\n"
        "attribute llrm drops; numbered types): " + ", ".join(dropped) + "\n"
    )
    print(f"{len(kept)} kept, {len(dropped)} dropped: {' '.join(dropped)}")
    return 0


def main():
    args, binary, listing = sys.argv[1:], str(PINNED), False
    if args[:1] == ["--refresh"]:
        return refresh(binary)
    if args[:1] == ["--bin"]:
        binary, args = args[1], args[2:]
    if args[:1] == ["--list"]:
        listing, args = True, args[1:]
    files = args or sorted(str(one) for one in TESTS.glob("*.ll"))
    failed = missing = compared = skipped = 0
    scratch = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "da-oracle"
    scratch.mkdir(parents=True, exist_ok=True)
    runs = []
    for path in files:
        found = opt_flags(path)
        if not found:
            skipped += 1
        runs += [(path, flags) for flags in found]
    for path, flags in runs:
        want = run([binary, "-disable-output", *flags[0], path])
        got = run([str(LLRM), "--da", *flags[1], str(sanitized(path, scratch))])
        if want.returncode or got.returncode:
            missing += 1
            print(f"MISSING {Path(path).name}: opt {want.returncode} {want.stderr[:100]!r} llrm {got.returncode} {got.stderr[:160]!r}")
            continue
        compared += 1
        a, b = parse(want.stderr), parse(got.stdout)
        if a != b:
            failed += 1
            print(f"DIFF {Path(path).name} {' '.join(flags[0])}")
            if listing:
                continue
            for function in sorted(set(a) | set(b)):
                x, y = a.get(function, []), b.get(function, [])
                for n in range(max(len(x), len(y))):
                    one, two = (x[n] if n < len(x) else ["<none>"]), (y[n] if n < len(y) else ["<none>"])
                    if one != two:
                        print(f"  @{function} pair {n}: opt {one} llrm {two}")
    print(f"{len(files)} files / {len(runs)} runs / {compared} compared / {failed} failed / {missing} missing / {skipped} skipped")
    return 1 if failed or missing else 0


if __name__ == "__main__":
    sys.exit(main())
