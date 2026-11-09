"""A cell is only a measurement if the program produced its expected value."""
import sys


def same_value(reported, expected):
    """`reported` is count.py's first field (`<value>` or `<value>:<file md5>`), `expected` the program's .out text.
    A 32-bit value may print signed or unsigned."""
    value = int(str(reported).split(":")[0])
    want = int(expected.split()[0])
    return value == want or (value - want) % (1 << 32) == 0


if __name__ == "__main__":
    reported, out = sys.argv[1], open(sys.argv[2]).read()
    if not same_value(reported, out):
        sys.exit(f"expect.py: reported {reported}, {sys.argv[2]} says {out.split()[0]}")
