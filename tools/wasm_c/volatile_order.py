#!/usr/bin/env python3
"""Order of the volatile loads in wasm objects: volatile_order.py OBJ... prints, per function, the symbols loaded in instruction order
(wasm is sequential in one thread, and a load cannot be hooked from node, so the emitted order is the observed order) and exits 1 if
one differs from the source's left-to-right order (tools/wasm_c/volatile_order.c)."""
import re
import subprocess
import sys

WANT = {"f3": ["g1", "g2", "g3"], "f5": ["g1", "q1", "d1", "g2", "q2"], "fnest": ["g1", "g2", "g3"]}


def loads(obj: str) -> dict[str, list[str]]:
    out = subprocess.run(["wasm-objdump", "-dr", obj], capture_output=True, text=True, check=True).stdout
    seqs: dict[str, list[str]] = {}
    current, last_was_load = None, False
    for line in out.splitlines():
        head = re.match(r"\s*[0-9a-f]+ func\[\d+\] <(\w+)>:", line)
        if head:
            current = head.group(1)
            seqs[current] = []
            continue
        if re.search(r"\|\s+(i32|i64|f32|f64)\.load", line):
            last_was_load = True
        elif re.search(r"\|\s+\S", line):
            last_was_load = False
        symbol = re.search(r"R_WASM_MEMORY_ADDR_LEB \d+ <(\w+)>", line)
        if symbol and current and last_was_load:
            seqs[current].append(symbol.group(1))
    return seqs


def wrong(obj: str) -> list[str]:
    got = loads(obj)
    return [f"{obj} {name}: {got.get(name)} != {order}" for name, order in WANT.items() if got.get(name) != order]


if __name__ == "__main__":
    bad = [line for obj in sys.argv[1:] for line in wrong(obj)]
    print("\n".join(bad) or "ok")
    sys.exit(1 if bad else 0)
