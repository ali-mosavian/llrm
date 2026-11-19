"""bench_all.sh log -> softfloat-z80.tsv. usage: table.py LOG OLD.tsv HEADER > OUT.tsv (hand cells and the column layout come from OLD.tsv).
Refuses a log with a failing cell or a missing one."""
import re
import sys

OPS = "add sub mul div itof utof ftoi ftou".split()


def table(log, old, header):
    gcc, lib = {}, {}
    for l in log.splitlines():
        m = re.match(r".*/(o2|os) (sf32\S*) (\w+) stubfails=0 fails=(\d+) cycles/call=(\d+) lib_bytes=(\d+)", l)
        if m:
            lv, v, op, f, c, b = m.groups()
            assert f == "0", l
            L = "-O2" if lv == "o2" else "-Os"
            gcc[(v, L, op)], lib[(v, L)] = int(c), int(b)
    assert len(gcc) == 32 and len(lib) == 4, (len(gcc), len(lib))
    rows = [l for l in old.splitlines() if l.startswith("hand\t")]
    out = [header, "compiler\tvariant\tlevel\tmetric\titem\tvalue"]
    out += [r for r in rows if "\tclk\t" in r]
    for v in ("sf32", "sf32_6502"):
        for lv in ("-Os", "-O2"):
            out += [f"gcc\t{v}\t{lv}\tclk\t{op}\t{gcc[(v, lv, op)]}" for op in OPS]
    out += [r for r in rows if "\tbytes\t" in r]
    for v in ("sf32", "sf32_6502"):
        for lv in ("-Os", "-O2"):
            out.append(f"gcc\t{v}\t{lv}\tbytes\tlibrary\t{lib[(v, lv)]}")
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    sys.stdout.write(table(open(sys.argv[1]).read(), open(sys.argv[2]).read(), sys.argv[3]))
