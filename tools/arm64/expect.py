"""A cell is only a measurement if the program produced its expected value."""
import os
from pathlib import Path
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import corpus_targets  # noqa: E402

TARGET = "arm64-linux"


def expected_path(bench_dir, prog):
    """The reference output a cell is judged on: NAME.ci.out (the part of the corpus the harness feeds), else the one
    `corpus_targets.expected_output` names for aarch64 (NAME.arm64.out, NAME.lp64.out, NAME.out)."""
    source = f"{bench_dir}/{prog}/{prog}.c"
    ci = f"{bench_dir}/{prog}/{prog}.ci.out"
    return ci if os.path.exists(ci) else str(corpus_targets.expected_output(Path(source), TARGET))


def same_value(reported, expected):
    """`reported` is count.py's first field (`<value>` or `<value>:<file md5>`), `expected` the program's .out text.
    A 32-bit value may print signed or unsigned."""
    value = int(str(reported).split(":")[0])
    want = int(expected.split()[-1])  # `reported` holds the last report: a program with two (grep: lines, checksum) is judged on its last
    return value == want or (value - want) % (1 << 32) == 0


def status(prog, level, reported, expected):
    """`ok` or `fail`."""
    return "ok" if same_value(reported, expected) else "fail"


if __name__ == "__main__":
    if sys.argv[1] == "--path":
        print(expected_path(sys.argv[2], sys.argv[3]))
        sys.exit()
    if sys.argv[1] == "--applies":
        source = Path(sys.argv[2])
        if not corpus_targets.applies(TARGET, source.read_text(), source):
            print(corpus_targets.reason(source.read_text(), source))
            sys.exit(3)
        sys.exit()
    prog, level, reported, out = sys.argv[1], sys.argv[2], sys.argv[3], open(sys.argv[4]).read()
    result = status(prog, level, reported, out)
    if result == "fail":
        sys.exit(f"expect.py: {prog} {level} reported {reported}, {sys.argv[4]} says {out.split()[-1]}")
    print(result)
