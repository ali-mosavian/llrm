#!/usr/bin/env python3
"""sizes.py [--check]: write (or check) sizes.csv, the bytes of each global routine of z80_rt.s and the routines each
one calls, jumps to or falls into: `name,bytes,callee;callee`. A size is the distance to the next global symbol in the
assembler listing, as the link map gives it. The compiler reads the file to price the runtime a function reaches
(src/runtime_size.rs)."""
import os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
SOURCE = os.path.join(HERE, "z80_rt.s")
OUT = os.path.join(HERE, "sizes.csv")


def falls_through(words):
    if words[0] in ("ret", "halt", "reti", "retn"):
        return len(words) > 1
    if words[0] in ("jp", "jr"):
        return "," in " ".join(words[1:])
    return True


def calls(path):
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
        line = re.sub(r"^\s*\w+\$:", "", line)
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


def table():
    with tempfile.TemporaryDirectory() as work:
        copy = os.path.join(work, "z80_rt.s")
        open(copy, "w").write(open(SOURCE).read())
        subprocess.run(["sdasz80", "-g", "-l", "-o", "z80_rt.rel", "z80_rt.s"], check=True, capture_output=True, cwd=work)
        listing = open(os.path.join(work, "z80_rt.lst")).read()
    globals_ = set(re.findall(r"\.globl\s+(\w+)", open(SOURCE).read()))
    addresses, end = {}, 0
    for line in listing.splitlines():
        label = re.match(r"\s+([0-9A-F]{8})\s+(?:(?:[0-9A-F]{2} )*\s*)?(?:\[.*?\])?\s*\d+\s+(\w+)::?", line)
        if label and label.group(2) in globals_:
            addresses[label.group(2)] = int(label.group(1), 16)
        code = re.match(r"\s+([0-9A-F]{8})\s+((?:[0-9A-F]{2} ){1,4})", line)
        if code:
            end = max(end, int(code.group(1), 16) + len(code.group(2).split()))
    order = sorted(addresses.items(), key=lambda one: one[1])
    graph = calls(SOURCE)
    rows = []
    for at, (name, start) in enumerate(order):
        stop = order[at + 1][1] if at + 1 < len(order) else end
        rows.append(f"{name},{stop - start},{';'.join(sorted(graph.get(name, ())))}")
    return "\n".join(sorted(rows)) + "\n"


if __name__ == "__main__":
    text = table()
    if "--check" in sys.argv:
        sys.exit(0 if open(OUT).read() == text else "sizes.csv is stale: run runtime/sizes.py")
    open(OUT, "w").write(text)
