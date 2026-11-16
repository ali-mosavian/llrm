"""A cell is only a measurement if the program produced its expected value."""
import os
import sys

# A program whose float model differs on aarch64 (fused a*b+c) has its own NAME.arm64.out.


def expected_path(bench_dir, prog):
    """The reference output a cell is judged on: NAME.arm64.out, else NAME.ci.out (the part of the corpus the harness
    feeds), else NAME.out."""
    for suffix in (".arm64.out", ".ci.out", ".out"):
        path = f"{bench_dir}/{prog}/{prog}{suffix}"
        if os.path.exists(path):
            return path
    return f"{bench_dir}/{prog}/{prog}.out"


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
    prog, level, reported, out = sys.argv[1], sys.argv[2], sys.argv[3], open(sys.argv[4]).read()
    result = status(prog, level, reported, out)
    if result == "fail":
        sys.exit(f"expect.py: {prog} {level} reported {reported}, {sys.argv[4]} says {out.split()[-1]}")
    print(result)
