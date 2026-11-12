"""A cell is only a measurement if the program produced its expected value, or is a named exception."""
import sys

# (program, level): (observed value, expected value in the .out, cause). The observed value is exact: a
# different value is a failure, not an exception. stft_single: clang -ffp-contract=on, gcc fast and ours print
# 40144774; -ffp-contract=off prints 502187692 (clang and gcc), clang fast 232171158; none is the .out's
# 78803551, the DOS/x87 value. The float result depends on contraction, so aarch64 cannot reach it.
# Every level prints 40144774 (O1 and O3 were missed at first: their cells failed the check).
CAUSE = "verified: value depends on -ffp-contract (off 502187692, on 40144774)"
EXCEPTIONS = {("stft_single", level): ("40144774", "78803551", CAUSE) for level in ("O1", "O2", "O3", "Os", "Oz")}


def same_value(reported, expected):
    """`reported` is count.py's first field (`<value>` or `<value>:<file md5>`), `expected` the program's .out text.
    A 32-bit value may print signed or unsigned."""
    value = int(str(reported).split(":")[0])
    want = int(expected.split()[-1])  # `reported` holds the last report: a program with two (grep: lines, checksum) is judged on its last
    return value == want or (value - want) % (1 << 32) == 0


def status(prog, level, reported, expected):
    """`ok`, `exception` (the named observed value), or `fail`."""
    if same_value(reported, expected):
        return "ok"
    level = level if level.startswith("O") else "O" + level  # cell.sh passes 1/2/3/s
    named = EXCEPTIONS.get((prog, level))
    if named and str(reported).split(":")[0] == named[0] and expected.split()[-1] == named[1]:
        return "exception"
    return "fail"


if __name__ == "__main__":
    prog, level, reported, out = sys.argv[1], sys.argv[2], sys.argv[3], open(sys.argv[4]).read()
    result = status(prog, level, reported, out)
    if result == "fail":
        sys.exit(f"expect.py: {prog} {level} reported {reported}, {sys.argv[4]} says {out.split()[-1]}")
    print(result)
