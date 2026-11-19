#!/usr/bin/env python3
"""No runtime routine may call itself through a libcall: `udivmod` -> `__udivsi3` -> `udivmod`.

m68k_runtime_guard.py LLRM_CC [CPU] [LEVEL...]   exit 1 and the cycles on stderr, else silent

`runtime/compiler-rt/lib/int/int32.c` divides 16-bit values in `udivmod` and relies on the optimizer to narrow
`(uint16_t)n / (uint16_t)d` to `divu`. The -O1 pipeline does not (as the pin's: narrowing is InstCombine on a zext, or
CVP), so at -O1 `udivmod` calls `__udivsi3`, which calls `udivmod`: a stack overflow in every program that divides
(nbody_fixed and div_constants_short at -O1, 2026-11-09). The shipped build (tools/build_rt_m68k.sh) is -Os.
"""
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
RUNTIME = ROOT / "runtime/compiler-rt"
SOURCES = sorted(str(p.relative_to(RUNTIME)) for p in RUNTIME.glob("lib/int/*.c"))  # one call graph across them: __udivsi3 is int32.c's, its callers are not

SYMBOL = re.compile(r"^\s+(?:jsr|jmp|bsr(?:\.\w)?|bra(?:\.\w)?|lea|pea)\s+\(?([A-Za-z_][\w$.]*)")
FUNCTION = re.compile(r"^([A-Za-z_][\w$.]*):")


def call_graph(asm: str) -> dict[str, set[str]]:
    """Function -> the symbols it references with a call, jump, `lea` or `pea` (a hoisted call target is a `lea`)."""
    graph: dict[str, set[str]] = {}
    current = None
    for line in asm.splitlines():
        if m := FUNCTION.match(line):
            current = m.group(1)
            graph.setdefault(current, set())
        elif current and (m := SYMBOL.match(line)):
            graph[current].add(m.group(1))
    return graph


def cycles(graph: dict[str, set[str]]) -> list[list[str]]:
    """The functions of `graph` that reach themselves: each strongly connected set of more than one, and each self-call (Tarjan)."""
    index: dict[str, int] = {}
    low: dict[str, int] = {}
    stack: list[str] = []
    on: set[str] = set()
    found: list[list[str]] = []
    counter = [0]

    def visit(v: str) -> None:
        index[v] = low[v] = counter[0]
        counter[0] += 1
        stack.append(v)
        on.add(v)
        for w in sorted(graph.get(v, ())):
            if w not in graph:
                continue
            if w not in index:
                visit(w)
                low[v] = min(low[v], low[w])
            elif w in on:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            part = []
            while True:
                w = stack.pop()
                on.discard(w)
                part.append(w)
                if w == v:
                    break
            if len(part) > 1 or v in graph.get(v, ()):
                found.append(sorted(part))

    for v in sorted(graph):
        if v not in index:
            visit(v)
    return found


def compile_source(cc: str, source: str, cpu: str, level: str) -> str:
    sys.path.insert(0, str(HERE))
    import llrmbin
    env = {**os.environ, **llrmbin.cc_environment(), "LLRM_CC_TARGET": f"m68k-{cpu}"}
    run = subprocess.run(
        [cc, f"-{level}", "-w", "-S", "-o", "-", "-I", str(RUNTIME / "include"), "-I", str(RUNTIME / "config/m68k"),
         "-include", str(RUNTIME / "config/m68k/config.h"), str(RUNTIME / source)],
        capture_output=True, text=True, env=env, timeout=180,
    )
    if run.returncode != 0:
        raise SystemExit(f"m68k_runtime_guard: {source} at -{level} does not compile: {run.stderr[:200]}")
    return run.stdout


def runtime_cycles(cc: str, cpu: str, level: str) -> list[list[str]]:
    """The call cycles of the runtime sources built at `level` for `cpu`."""
    graph: dict[str, set[str]] = {}
    for source in SOURCES:
        for name, callees in call_graph(compile_source(cc, source, cpu, level)).items():
            graph.setdefault(name, set()).update(callees)
    return cycles(graph)


def main() -> None:
    cc, cpu = sys.argv[1], (sys.argv[2] if len(sys.argv) > 2 else "68000")
    bad = False
    for level in sys.argv[3:] or ["O2", "Os"]:
        for cycle in runtime_cycles(cc, cpu, level):
            print(f"m68k_runtime_guard: -{level} {cpu}: {' -> '.join(cycle)} call each other", file=sys.stderr)
            bad = True
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
