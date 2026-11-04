"""CODE+RODATA+DATA of the generic.lib / x6502.lib members in an ld65 map (drv.o and the cc65 startup are not the library)."""
import re
import sys


def lib_bytes(text):
    mod, tot = False, 0
    for l in text.splitlines():
        m = re.match(r"(\S+)\.lib\(\S+\.o\):", l)
        if m:
            mod = m.group(1).endswith(("generic", "x6502"))
        elif re.match(r"\S+:$", l):
            mod = False
        else:
            m = re.match(r"\s+(CODE|RODATA|DATA)\s+Offs=\w+\s+Size=(\w+)", l)
            if m and mod:
                tot += int(m.group(2), 16)
    return tot


if __name__ == "__main__":
    print(lib_bytes(open(sys.argv[1]).read()))
