#!/usr/bin/env python3
"""pin_ablate.py PASS... : what the pin's -O2 pipeline does without each PASS, over the IRs of `IR_DIR`/*/01-globalopt.ll.

    LLRM_ABLATE_IRS=~/scratch/fa/pinir tools/pairs/pin_ablate.py tailcallelim div-rem-pairs ...

The pipeline is the fixture's (tests/fixtures/pin-pipeline/O2.txt) with one instance of PASS removed at a time; each IR runs through the pin's `opt`
(tools/pairs/pin.py, the m32 target) with it and with the whole pipeline. Prints, per instance of each pass, the IRs whose output differs and the change in
instructions (lines of `-S` that are instructions) summed over them. A pass the pin runs and whose removal changes nothing here is no
evidence it never matters: only that these IRs do not need it.
"""
from __future__ import annotations

import os
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pin  # noqa: E402

FIXTURE = HERE.parents[1] / "tests" / "fixtures" / "pin-pipeline" / "O2.txt"


def tree(text: str):
    """The printed pipeline as nested [name, [children]] lists; a leaf has children None."""
    pos = 0

    def items(close: bool):
        nonlocal pos
        out = []
        buf = ""
        angle = 0
        while pos < len(text):
            ch = text[pos]
            pos += 1
            if ch == "<":
                angle += 1
            elif ch == ">":
                angle -= 1
            if angle == 0 and ch == "(":
                out.append([buf, items(True)])
                buf = ""
                # after a group: optional comma
                if pos < len(text) and text[pos] == ",":
                    pos += 1
            elif angle == 0 and ch == ")":
                if buf:
                    out.append([buf, None])
                return out
            elif angle == 0 and ch == ",":
                if buf:
                    out.append([buf, None])
                buf = ""
            else:
                buf += ch
        if buf:
            out.append([buf, None])
        return out

    return items(False)


def without(nodes, name: str, only: int | None = None, seen: list[int] | None = None, path: str = "", paths: list[str] | None = None):
    """`nodes` less every instance of `name`, or less the `only`th (0-based, in pipeline order) alone; `paths` collects the nest of
    each instance met."""
    seen = [0] if seen is None else seen
    out = []
    for node, children in nodes:
        if children is None:
            if node.split("<")[0] == name:
                at = seen[0]
                seen[0] += 1
                if paths is not None:
                    paths.append(path)
                if only is None or only == at:
                    continue
            out.append([node, None])
        else:
            kept = without(children, name, only, seen, f"{path}/{node}" if path else node, paths)
            if kept:
                out.append([node, kept])
    return out


def show(nodes) -> str:
    return ",".join(node if children is None else f"{node}({show(children)})" for node, children in nodes)


def pin_text_of(text: str) -> str:
    """`text` as the pin parses it; a QB program's `__main` is internal with no caller, so it is made a root."""
    text = re.sub(r"define internal (.*@__main\()", r"define \1", text)
    return re.sub(r" (threeway|flagsresult|releases)\b", "", text)


def pin_text(path: Path) -> str:
    """The IR as the pin parses it (see `pin_text_of`)."""
    return pin_text_of(path.read_text())


def instructions(ir: str) -> int:
    return sum(1 for line in ir.splitlines() if re.match(r"^\s+(%\S+ = |store |call |br |ret |switch |unreachable)", line))


def main(passes: list[str]) -> int:
    root = Path(os.environ.get("LLRM_ABLATE_IRS", Path.home() / "scratch/fa/pinir")).expanduser()
    irs = sorted(root.glob("*/01-globalopt.ll"))
    # LLRM_ABLATE_TARGET=m16 for the m16 stage IRs (QCport), LLRM_ABLATE_OPTSIZE=1 for -Os (the O2 list on optsize IR)
    of = pin.target(os.environ.get("LLRM_ABLATE_TARGET", "m32"), optsize=bool(os.environ.get("LLRM_ABLATE_OPTSIZE")))
    full = show(tree([l for l in FIXTURE.read_text().splitlines() if not l.startswith(";")][-1]))
    base = {}
    skipped = []
    for ir in list(irs):
        done = pin.opt(full, pin_text(ir), of)
        if done.returncode != 0:
            skipped.append(f"{ir.parent.name}: {done.stderr.strip().splitlines()[0][:90] if done.stderr.strip() else 'failed'}")
            irs.remove(ir)
            continue
        base[ir] = instructions(done.stdout)
    print(f"# pin opt -O2 over {len(irs)} IRs ({root}); target {of.triple}, optsize {of.optsize}; baseline instructions {sum(base.values())}; the pin could not parse {len(skipped)}: {'; '.join(skipped)}")
    print("pass | IRs changed | instructions without it (sum over the changed) | delta")
    root_tree = tree([l for l in FIXTURE.read_text().splitlines() if not l.startswith(";")][-1])
    for name in passes:
        paths: list[str] = []
        without(root_tree, name, None, None, "", paths)
        # one run per instance, so a pass the pin runs three times is three rows
        for at, path in enumerate(paths):
            pipeline = show(without(root_tree, name, at))
            changed = delta = failed = 0
            for ir in irs:
                done = pin.opt(pipeline, pin_text(ir), of)
                if done.returncode != 0:
                    failed += 1
                    continue
                got = instructions(done.stdout)
                if got != base[ir]:
                    changed += 1
                    delta += got - base[ir]
            print(f"{name}#{at} ({path}) | {changed} | {delta:+d} | {'opt failed on ' + str(failed) if failed else ''}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
