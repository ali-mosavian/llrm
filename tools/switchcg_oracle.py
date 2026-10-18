#!/usr/bin/env python3
"""Golden decisions of the pinned llc for switches, read from its MIR after instruction selection.

Each switch is on `i32` with cases `value:target`; targets are blocks a..e, the default is `z`.
llc's summary is `JT:<targets of the table>` per jump table and `BT:<mask>` per bit test
(`MOV32ri mask` then `BT32rr`); the tests in crates/opt/llrm-transforms/src/switchcg_tests.rs
recompute the same tokens from the Rust translation of SwitchLoweringUtils.cpp.

    tools/switchcg_oracle.py > crates/opt/llrm-transforms/src/switchcg_golden.txt

Needs $LLVM_BUILD (default ~/scratch/llvm-build) and $LLVM_SRC (default ~/scratch/llvm-src) at the pin.
"""

import os
import re
import subprocess
import sys
import tempfile

COMMIT = "d1106deb71cc"
LLVM_BUILD = os.path.expanduser(os.environ.get("LLVM_BUILD", "~/scratch/llvm-build"))
LLVM_SRC = os.path.expanduser(os.environ.get("LLVM_SRC", "~/scratch/llvm-src"))
LLC = [f"{LLVM_BUILD}/bin/llc", "-mtriple=i386-linux-gnu", "-O2", "-stop-after=finalize-isel"]

# Shapes of case lists; each is (values, targets as a cycle). Hand-picked to reach dense
# runs, holes, one target, two, three, four, negatives, and clusters of equal targets.
SHAPES = [
    (range(0, 6), "abcabc"),
    (range(0, 8), "ab"),
    (range(0, 4), "a"),
    (range(0, 12), "abc"),
    (range(0, 12), "abcd"),
    (range(0, 20), "abcde"),
    ([0, 1, 2, 3, 10, 11, 12, 13], "aabb"),
    ([0, 2, 4, 6, 8, 10], "abcabc"),
    ([0, 2, 4, 6, 8, 10], "ab"),
    ([1, 3, 5, 7, 9], "a"),
    ([1, 5, 9, 13, 17, 21], "ab"),
    ([0, 100, 200, 300, 400], "abcde"),
    ([0, 1, 2, 100, 101, 102, 103], "aabbbb"),
    ([0, 1, 2, 3, 4, 20, 21, 22, 23, 24], "aaaaabbbbb"),
    ([-3, -2, -1, 0, 1, 2], "abcabc"),
    ([-8, -6, -4, -2, 0, 2, 4, 6], "ab"),
    ([5, 6, 7, 9, 10, 11, 12, 15, 16], "aabbbccdd"),
    ([0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30], "abc"),
    ([0, 1, 2, 3, 5, 6, 7, 8, 40, 41, 42, 43, 44, 45], "aaaabbbbccccdd"),
    ([10, 11, 12, 13, 14, 15, 16], "abcdabc"),
    ([0, 1, 3, 4, 6, 7, 9, 10, 12, 13], "aabbaabbaa"),
    ([2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31], "abcabcabcab"),
    ([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 100, 101, 102, 103, 104, 105], "abcdabcdabcdabcd"),
    ([31, 32, 33, 34, 35, 36], "abcabc"),
    ([0, 31, 62, 93, 124, 155, 186], "abcdeab"),
    ([-2, -1, 0, 1, 4], "c"),  # a dominant cluster is peeled before bit tests
]


def spec_of(values, targets):
    values = list(values)
    return " ".join(f"{value}:{targets[i % len(targets)]}" for i, value in enumerate(values))


def program(spec):
    cases = [pair.split(":") for pair in spec.split()]
    names = sorted({target for _, target in cases} | {"z"})
    lines = ["define i32 @f(i32 %x) {", "entry:", "  switch i32 %x, label %z ["]
    lines += [f"    i32 {value}, label %{target}" for value, target in cases]
    lines.append("  ]")
    lines += [f"{name}:\n  ret i32 {10 * (i + 1)}" for i, name in enumerate(names)]
    lines.append("}")
    return "\n".join(lines) + "\n"


def check_pin():
    """The premise: the llc is the pinned one, and the source tree it reads is at the pin."""
    version = subprocess.run([f"{LLVM_BUILD}/bin/llc", "--version"], capture_output=True, text=True).stdout
    head = subprocess.run(["git", "-C", LLVM_SRC, "rev-parse", "HEAD"], capture_output=True, text=True).stdout
    if "24.0.0git" not in version or not head.startswith(COMMIT):
        sys.exit(f"not the pin {COMMIT}: llc says {version.splitlines()[1:2]}, source is at {head.strip()}")
    print(f"# llc 24.0.0git at {COMMIT}")


def decisions(spec):
    with tempfile.NamedTemporaryFile("w", suffix=".ll") as file:
        file.write(program(spec))
        file.flush()
        mir = subprocess.run(LLC + [file.name, "-o", "-"], capture_output=True, text=True, check=True).stdout
    names = dict(re.findall(r"^  bb\.(\d+)\.(\w+):", mir, re.M))
    tokens = []
    for entry in re.findall(r"blocks:\s*\[([^\]]*)\]", mir):
        blocks = re.findall(r"%bb\.(\d+)", entry)
        tokens.append("JT:" + ",".join(names[block] for block in blocks))
    lines = mir.splitlines()
    for number, line in enumerate(lines):
        mask = re.search(r"MOV32ri (-?\d+)", line)
        if mask and "BT32rr" in lines[number + 1]:
            tokens.append(f"BT:{int(mask.group(1)) % 2**32}")
    return sorted(tokens)


def main():
    check_pin()
    kinds = set()
    for values, targets in SHAPES:
        spec = spec_of(values, targets)
        tokens = decisions(spec)
        kinds.update(token.split(":")[0] for token in tokens)
        print(f"{spec} => {' '.join(tokens) or '-'}")
    # Premise: an oracle that never saw a table or a bit test reads nothing.
    if kinds != {"JT", "BT"}:
        sys.exit(f"instrument reads nothing: saw only {kinds}")


if __name__ == "__main__":
    main()
