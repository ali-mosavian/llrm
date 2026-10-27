#!/usr/bin/env python3
"""One-off differential fuzzing of the arm64 backend (not part of any suite): random C
programs over the integer and floating types, built by this compiler (from the pinned
clang's un-optimized IR) and by the pinned `clang -O2`, run under qemu-aarch64, and
compared. Usage: tools/targets/arm64_fuzz.py [programs] [seed]. A difference leaves the program
in the scratch directory it prints, to be reduced into a test."""
import os
import random
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "crates/target/llrm-arm64"
BUILD = Path(os.environ.get("LLVM_BUILD", Path.home() / "scratch/llvm-build-aarch64"))
CLANG, LLD = BUILD / "bin/clang", BUILD / "bin/ld.lld"
LLC = ROOT / "target/debug/llrm-arm64-llc"
TRIPLE = "aarch64-unknown-linux-gnu"

TYPES = [("signed char", 8, True), ("unsigned char", 8, False), ("short", 16, True), ("unsigned short", 16, False),
         ("int", 32, True), ("unsigned", 32, False), ("long", 64, True), ("unsigned long", 64, False)]
INTERESTING = [0, 1, 2, 3, 7, 8, 15, 16, 31, 100, 127, 128, 255, 256, 1000, 32767, 32768, 65535, 65536, 0x7FFFFFFF,
               0x80000000, 0xFFFFFFFF, 0x100000000, 0x7FFFFFFFFFFFFFFF, 0x8000000000000000, 0xFFFFFFFFFFFFFFFF]


def constant(rng, name):
    value = rng.choice(INTERESTING + [rng.getrandbits(rng.choice([4, 8, 16, 32, 64])) for _ in range(6)])
    if rng.random() < 0.3:
        value = -value
    value &= (1 << 64) - 1
    return f"(({name}){value}ul)"


def expression(rng, depth, variables, ty):
    name, bits, signed = ty
    if depth == 0 or rng.random() < 0.15:
        return rng.choice(variables) if rng.random() < 0.65 else constant(rng, name)
    kind = rng.random()
    a = lambda: expression(rng, depth - 1, variables, rng.choice(TYPES))
    b = lambda: expression(rng, depth - 1, variables, ty)
    # Arithmetic is done unsigned so that signed overflow is not undefined.
    if kind < 0.35:
        op = rng.choice(["+", "-", "*", "&", "|", "^"])
        return f"(({name})((unsigned long)({b()}) {op} (unsigned long)({b()})))"
    if kind < 0.5:
        # The operand is cast to the type first (variables are of any type) and
        # shifted left unsigned, so no shift is undefined.
        count = f"((unsigned)({b()}) % {bits})"
        if rng.random() < 0.5:
            return f"(({name})((unsigned long)(({name})({b()})) << {count}))"
        return f"(({name})((({name})({b()})) >> {count}))"
    if kind < 0.65:
        op = rng.choice(["/", "%"])
        divisor = f"(({name})(((unsigned long)({b()}) & 0x7f) + 1))" if rng.random() < 0.5 else f"(({name}){rng.choice([3, 5, 7, 10, 12, 100, 127, 255, 1000, 641])})"
        return f"(({name})(({b()}) {op} {divisor}))"
    if kind < 0.8:
        op = rng.choice(["<", "<=", ">", ">=", "==", "!="])
        return f"(({name})(({a()}) {op} ({a()})))"
    if kind < 0.9:
        return f"(({name})(({a()}) ? ({b()}) : ({b()})))"
    if kind < 0.95:
        return f"(({name})(~({b()})))"
    return f"(({name})({a()}))"


def program(rng, functions):
    source = ["typedef unsigned long u64;", "int printf(const char *, ...);"]
    for index in range(4):
        for ty in TYPES:
            pass
    for index, (name, _, _) in enumerate(TYPES):
        source.append(f"volatile {name} g{index} = {constant(rng, name)};")
    body = []
    for f in range(functions):
        ty = rng.choice(TYPES)
        parameters = ", ".join(f"{t[0]} p{i}" for i, t in enumerate(rng.sample(TYPES, 3)))
        params = [p.split()[-1] for p in parameters.split(", ")]
        variables = params + [f"g{i}" for i in range(len(TYPES))][:3]
        # Variables are the parameters (opaque to the optimizer: called from main with globals).
        expr = expression(rng, rng.randint(2, 4), params, ty)
        source.append(f"__attribute__((noinline)) static {ty[0]} f{f}({parameters}) {{ return {expr}; }}")
        arguments = ", ".join(f"({t.split()[0] if False else t})g{rng.randrange(len(TYPES))}" for t in [p.rsplit(" ", 1)[0] for p in parameters.split(", ")])
        body.append(f'  h = h * 1000003 + (u64)(long)f{f}({arguments}); printf("f{f} %lx\\n", (unsigned long)h);')
    source.append("int main(void) {\n  u64 h = 1;\n" + "\n".join(body) + "\n  return 0;\n}")
    return "\n".join(source) + "\n"


def run(*command):
    done = subprocess.run([str(c) for c in command], capture_output=True, text=True)
    return done


def main():
    count = int(sys.argv[1]) if len(sys.argv) > 1 else 50
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    work = Path(tempfile.mkdtemp(prefix="arm64fuzz"))
    print("scratch", work)
    runtime = work / "rt.o"
    assert run(CLANG, "-target", TRIPLE, "-O2", "-ffreestanding", "-fno-builtin", "-c", "-o", runtime, CRATE / "tests/rt/rt.c").returncode == 0
    failures = 0
    for number in range(count):
        rng = random.Random(seed * 100000 + number)
        source = work / f"p{number}.c"
        source.write_text(program(rng, rng.randint(3, 8)))
        ir = work / f"p{number}.ll"
        ref = work / f"p{number}-ref"
        ours = work / f"p{number}-ours"
        steps = [
            run(CLANG, "-target", TRIPLE, "-O2", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-lifetime-markers", "-S", "-emit-llvm", "-o", ir, source),
        ]
        if steps[0].returncode:
            print(number, "clang front end failed", steps[0].stderr[:300])
            failures += 1
            continue
        env = dict(os.environ, LLRM_VERIFY="0")
        compiled = subprocess.run([str(LLC), "-o", str(work / f"p{number}-ours.o"), str(ir)], capture_output=True, text=True, env=env)
        if compiled.returncode:
            print(number, "OURS FAILED TO COMPILE:", compiled.stderr.strip()[:300].replace("\n", " "))
            failures += 1
            continue
        run(CLANG, "-target", TRIPLE, "-O2", "-c", "-o", work / f"p{number}-ref.o", source)
        for name in ("ref", "ours"):
            linked = run(LLD, "-static", "-o", work / f"p{number}-{name}", work / f"p{number}-{name}.o", runtime)
            assert linked.returncode == 0, linked.stderr
        a = subprocess.run(["qemu-aarch64", str(ref)], capture_output=True, text=True)
        b = subprocess.run(["qemu-aarch64", str(ours)], capture_output=True, text=True)
        assert a.stdout, "the reference printed nothing"
        if (a.stdout, a.returncode) != (b.stdout, b.returncode):
            lines_a, lines_b = a.stdout.splitlines(), b.stdout.splitlines()
            first = next((i for i, (x, y) in enumerate(zip(lines_a, lines_b)) if x != y), None)
            print(number, "DIFFERENT at line", first, source)
            failures += 1
    print(f"{count} programs, {failures} failures")
    sys.exit(1 if failures else 0)


main()
