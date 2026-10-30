#!/usr/bin/env python3
"""Today's alias answers against LLVM's BasicAA, pair by pair, on dumped pass inputs.

    tools/aliascompare.py [--llrm-mir BIN] FILE.ll...

For each input: `llrm-mir --aliases` (the answer of the points-to facts for every pair of pointer operands of loads, stores and calls) and
`opt -passes=aa-eval -print-all-alias-modref-info` (the pinned LLVM, 24.0.0git d1106deb71cc, via tools/difftest/difftest.py's
normaliser and its LLVM_BIN) on the same text. Only pairs both print are compared. A table of (ours, LLVM's) counts follows, with the
opt version in its header. Ours NoAlias where LLVM says MayAlias is a candidate miscompile of today's code; ours MayAlias where LLVM
says NoAlias is a lost precision.
"""

import argparse
import collections
import importlib.util
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent


def difftest():
    """tools/difftest/difftest.py, or the directory named by DIFFTEST_DIR until it is on main."""
    for base in filter(None, [os.environ.get("DIFFTEST_DIR"), str(HERE / "difftest")]):
        path = Path(base) / "difftest.py"
        if path.is_file():
            spec = importlib.util.spec_from_file_location("difftest", path)
            module = importlib.util.module_from_spec(spec)
            sys.modules["difftest"] = module
            spec.loader.exec_module(module)
            return module
    sys.exit("aliascompare: tools/difftest/difftest.py is not here; set DIFFTEST_DIR to the directory that has it")


PAIR = re.compile(r"^\s+(NoAlias|MayAlias|PartialAlias|MustAlias):\s+(.*)$")
NAME = re.compile(r'[%@](?:"[^"]*"|[\w.$-]+)')
OPERAND = re.compile(r'(\S+)\*\s+([%@](?:"[^"]*"|[\w.$-]+))')
FUNCTION = re.compile(r"^Function: ([^:]+):")
SIZES = {"i1": 1, "i8": 1, "i16": 2, "i32": 4, "i64": 8, "float": 4, "double": 8}

Key = tuple[str, frozenset[tuple[str, int | None]]]


def entry(name: str, kind: str) -> tuple[str, int | None]:
    """A pointer and the bytes aa-eval accesses through it, from the type it prints (none where it prints one llrm sizes by its datalayout)."""
    return name.replace('"', ""), SIZES.get(kind)


# The data layout of the machine the replay inputs were made for (`-m32`, crates/target/llrm-x86-m32/src/machines/datalayout.toml): the dumps carry none, and
# both readings then use LLVM's default of 64-bit pointers, whose 8-byte `load ptr` is larger than a 4-byte object that the real 4-byte pointer fits.
M32_DATALAYOUT = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

AA_PIPELINES = {"default": "tbaa,scoped-noalias-aa,basic-aa", "basic": "basic-aa"}


def llvm_answers(dt, text: str, aa: str = "default") -> dict[Key, str]:
    done = subprocess.run(
        [str(dt.LLVM_BIN / "opt"), *dt.target_args(), f"-aa-pipeline={AA_PIPELINES[aa]}", "-passes=aa-eval", "-print-all-alias-modref-info", "-disable-output", "-"],
        input=dt.normalise(text),
        capture_output=True,
        text=True,
    )
    found: dict[Key, str] = {}
    current = ""
    for line in done.stderr.splitlines():
        if (head := FUNCTION.match(line)) is not None:
            current = head[1].strip('"')
        elif (pair := PAIR.match(line)) is not None and len(operands := OPERAND.findall(pair[2])) == 2:
            found[(current, frozenset(entry(name, kind) for kind, name in operands))] = pair[1]
    return found


def llrm_answers(binary: str, path: Path) -> dict[Key, str]:
    done = subprocess.run([binary, "--aliases", str(path)], capture_output=True, text=True)
    found = {}
    for line in done.stdout.splitlines():
        function, answer, first, first_bytes, second, second_bytes = line.split("\t")
        found[(function, frozenset(((first.replace('"', ""), int(first_bytes)), (second.replace('"', ""), int(second_bytes)))))] = answer
    return found


def theirs_for(theirs: dict[Key, str], key: Key) -> str | None:
    """LLVM's answer for one of llrm's entries: the same pointers and sizes, a size llrm cannot read from the type standing for any."""
    function, pair = key
    if (found := theirs.get(key)) is not None:
        return found
    (a, a_bytes), (b, b_bytes) = sorted(pair, key=str)
    for one in (a_bytes, None):
        for two in (b_bytes, None):
            if (found := theirs.get((function, frozenset(((a, one), (b, two)))))) is not None:
                return found
    return None


MEMOP_PAIR = re.compile(r"^\s+(NoAlias|MayAlias|PartialAlias|MustAlias):\s+.* <-> .*$")


def memop_answers(dt, binary: str, path: Path, text: str, aa: str) -> tuple[dict, dict, int]:
    """(ours, LLVM's) answers by (function, n) for the n-th load/store pair aa-eval lists in a function. Both sides list the pairs in aa-eval's
    order (each load against each store, then each store against the stores before it), so the n-th line of each is the same pair; a function
    where the counts differ is not compared (the third value)."""
    done = subprocess.run(
        [str(dt.LLVM_BIN / "opt"), *dt.target_args(), f"-aa-pipeline={AA_PIPELINES[aa]}", "-evaluate-aa-metadata", "-passes=aa-eval", "-print-all-alias-modref-info", "-disable-output", "-"],
        input=dt.normalise(text), capture_output=True, text=True,
    )
    theirs_lists: dict[str, list[str]] = collections.defaultdict(list)
    current = ""
    for line in done.stderr.splitlines():
        if (head := FUNCTION.match(line)) is not None:
            current = head[1].strip('"')
            theirs_lists[current] = []
        elif (pair := MEMOP_PAIR.match(line)) is not None:
            theirs_lists[current].append(pair[1])
    done = subprocess.run([binary, "--memops", str(path)], capture_output=True, text=True)
    ours_lists: dict[str, list[str]] = collections.defaultdict(list)
    for line in done.stdout.splitlines():
        function, answer = line.split("\t")
        ours_lists[function].append(answer)
    ours: dict = {}
    theirs: dict = {}
    skipped = 0
    for function, answers in ours_lists.items():
        if len(answers) != len(theirs_lists.get(function, [])):
            skipped += len(answers)
            continue
        for n, (mine, other) in enumerate(zip(answers, theirs_lists[function])):
            if mine != "?":
                ours[(function, n)] = mine
                theirs[(function, n)] = other
    return ours, theirs, skipped


MODREF_WORDS = {"NoModRef": "NoModRef", "Just Ref": "Ref", "Just Mod": "Mod", "Both ModRef": "ModRef"}
MODREF_LINE = re.compile(r"^\s+(NoModRef|Just Ref|Just Mod|Both ModRef):\s")


def call_answers(dt, binary: str, path: Path, text: str, aa: str) -> tuple[dict, dict, int]:
    """(ours, LLVM's) mod/ref answers by (function, n) for the n-th call-against-pointer or call-against-call pair aa-eval lists in a function
    (every call against every pointer, then every call against every other call); a function whose counts differ is not compared."""
    done = subprocess.run(
        [str(dt.LLVM_BIN / "opt"), *dt.target_args(), f"-aa-pipeline={AA_PIPELINES[aa]}", "-passes=aa-eval", "-print-all-alias-modref-info", "-disable-output", "-"],
        input=dt.normalise(text), capture_output=True, text=True,
    )
    theirs_lists: dict[str, list[str]] = collections.defaultdict(list)
    current = ""
    for line in done.stderr.splitlines():
        if (head := FUNCTION.match(line)) is not None:
            current = head[1].strip('"')
            theirs_lists[current] = []
        elif (found := MODREF_LINE.match(line)) is not None:
            theirs_lists[current].append(MODREF_WORDS[found[1]])
    done = subprocess.run([binary, "--calls", str(path)], capture_output=True, text=True)
    ours_lists: dict[str, list[str]] = collections.defaultdict(list)
    for line in done.stdout.splitlines():
        function, answer = line.split("\t")
        ours_lists[function].append(answer)
    ours: dict = {}
    theirs: dict = {}
    skipped = 0
    for function, answers in ours_lists.items():
        if len(answers) != len(theirs_lists.get(function, [])):
            skipped += len(answers)
            continue
        for n, (mine, other) in enumerate(zip(answers, theirs_lists[function])):
            if mine != "?":
                ours[(function, n)] = mine
                theirs[(function, n)] = other
    return ours, theirs, skipped


def premise(table: collections.Counter, calls: bool = False) -> str | None:
    """Why the reference cannot be trusted on this set: a comparison against an analysis that answered MayAlias alone, or never MustAlias or NoAlias,
    says nothing of the pairs it was asked (an instrument that cannot answer in more than one way agrees with anything)."""
    seen = {theirs for (_, theirs) in table}
    missing = ({"NoModRef"} - seen) | (set() if seen & {"Ref", "Mod", "ModRef"} or not calls else {"Ref, Mod or ModRef"}) if calls else {"NoAlias", "MustAlias"} - seen
    if missing:
        return f"the reference never answered {', '.join(sorted(missing))} on this set (it answered {', '.join(sorted(seen)) or 'nothing'})"
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("files", nargs="+", type=Path)
    parser.add_argument("--llrm-mir", default=str(ROOT / "target/release/llrm-mir"))
    parser.add_argument("--calls", action="store_true", help="the call-against-pointer and call-against-call mod/ref pairs instead of the pointer pairs")
    parser.add_argument("--datalayout", default=M32_DATALAYOUT, help="the data layout both sides read the dumps with ('' = LLVM's default, as the dumps carry none)")
    parser.add_argument("--memops", action="store_true", help="the load and store pairs of `-evaluate-aa-metadata` (type tags included unless --basic) instead of the pointer pairs")
    parser.add_argument(
        "--basic",
        action="store_true",
        help="LLVM's BasicAA alone against llrm's answers without type tags (LLRM_ALIASES_NO_TBAA); by default LLVM's default pipeline (TBAA, scoped noalias, BasicAA) against llrm's whole",
    )
    args = parser.parse_args()
    dt = difftest()
    version = subprocess.run([str(dt.LLVM_BIN / "opt"), "--version"], capture_output=True, text=True).stdout.split("\n")[1].strip()
    print(f"opt: {dt.LLVM_BIN / 'opt'} ({version}) -aa-pipeline={AA_PIPELINES['basic' if args.basic else 'default']}")
    if args.basic:
        os.environ["LLRM_ALIASES_NO_TBAA"] = "1"
    table: collections.Counter[tuple[str, str]] = collections.Counter()
    unmatched = 0
    theirs_only = 0
    ambiguous = 0
    for path in args.files:
        text = path.read_text()
        if args.datalayout and "target datalayout" not in text:
            text = f'target datalayout = "{args.datalayout}"\n' + text
        with tempfile.NamedTemporaryFile("w", suffix=".ll", delete=False) as handle:
            handle.write(text)
        real = Path(handle.name)
        if args.calls:
            ours, theirs, skipped = call_answers(dt, args.llrm_mir, real, text, "basic" if args.basic else "default")
            ambiguous += skipped
        elif args.memops:
            ours, theirs, skipped = memop_answers(dt, args.llrm_mir, real, text, "basic" if args.basic else "default")
            ambiguous += skipped
            llrm_side = {k: v for k, v in ours.items()}
        else:
            ours, theirs = llrm_answers(args.llrm_mir, real), llvm_answers(dt, text, "basic" if args.basic else "default")
        real.unlink(missing_ok=True)
        if not ours and not theirs:
            continue  # no two pointers to compare
        if not ours or not theirs:
            print(f"FAILED ROW (no answers from {'llrm' if not ours else 'opt'}): {path}")
            continue
        matched = 0
        for key, answer in ours.items():
            if (found := (theirs.get(key) if args.memops or args.calls else theirs_for(theirs, key))) is not None:
                table[(answer, found)] += 1
                matched += 1
            else:
                unmatched += 1
        theirs_only += max(0, len(theirs) - matched)
    print("ours -> LLVM: pairs")
    for (ours, theirs), count in sorted(table.items()):
        flag = "  <- candidate miscompile" if ours in ("NoAlias", "NoModRef") and theirs not in ("NoAlias", "NoModRef") else ""
        print(f"  {ours:9} {theirs:12} {count:8}{flag}")
    print(f"pairs llrm printed that aa-eval did not: {unmatched}")
    print(f"pairs aa-eval printed that llrm did not: {theirs_only}")
    if args.memops or args.calls:
        print(f"pairs of functions whose pair counts differ (not compared): {ambiguous}")
    if (why := premise(table, args.calls)) is not None:
        print(f"FAILED PREMISE: {why}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
