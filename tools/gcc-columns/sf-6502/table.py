"""bench_all.sh log -> softfloat-6502.tsv. usage: table.py LOG HEADER_LINE > OUT.tsv. Refuses a log with a failing cell or a missing one."""
import re
import sys

OPS = "add sub mul div itof utof ftoi ftou".split()
VARIANTS = {"generic": "sf32", "x6502": "sf32_6502"}


def table(log, header):
    gcc, hand, hbytes = {}, {}, {}
    for l in log.splitlines():
        m = re.match(r".*/(o2|os) (generic|x6502) (\w+) fails=(\d+) cycles/call=(\d+) bytes=(\d+) nv=(\d+)", l)
        if m:
            lv, v, op, f, c, b, nv = m.groups()
            assert f == "0", l
            gcc[(VARIANTS[v], "-O2" if lv == "o2" else "-Os", op)] = (int(c), int(b))
        m = re.match(r"hand (\w+) fails=(\d+) cycles/call=(\d+)", l)
        if m:
            assert m.group(2) == "0", l
            hand[m.group(1)] = int(m.group(3))
        m = re.match(r"hand bytes (\S+) (\d+)", l)
        if m:
            hbytes[m.group(1)] = int(m.group(2))
    assert len(gcc) == 32 and len(hand) == 8 and len(hbytes) == 3, (len(gcc), len(hand), len(hbytes))
    out = [header, "compiler\tvariant\tlevel\tmetric\titem\tvalue"]
    out += [f"hand\t-\t-\tclk\t{op}\t{hand[op]}" for op in OPS]
    for v in ("sf32", "sf32_6502"):
        for lv in ("-Os", "-O2"):
            out += [f"gcc\t{v}\t{lv}\tclk\t{op}\t{gcc[(v, lv, op)][0]}" for op in OPS]
    out += [f"hand\t-\t-\tbytes\t{n}\t{b}" for n, b in hbytes.items()]
    for v in ("sf32", "sf32_6502"):
        for lv in ("-Os", "-O2"):
            out += [f"gcc\t{v}\t{lv}\tbytes\t{op}\t{gcc[(v, lv, op)][1]}" for op in OPS]
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    sys.stdout.write(table(open(sys.argv[1]).read(), sys.argv[2]))
