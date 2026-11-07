#!/usr/bin/env python3
"""Mean over code paddings of the 6502 bench: tools/bench6502_pads.py OUT LOG...
Each LOG is one `BENCH_PAD=n` run of `cargo test -p llrm-6502 --test bench -- --ignored --nocapture`.
sim65 charges a cycle per page-crossing branch or indexed access, so one layout's cycles move by
about 2% with the code's address. The cycles of a cell are averaged, its bytes and instructions
are the first log's (they do not change with the padding). OUT has the format of a LOG."""
import re, sys

CELL = re.compile(r"(\d+) B(?: linked)?(?:, (\d+) B body)?, (\d+) cy(?:, (\d+) in)?")


def merged(texts):
    """The first text with every cell's cycles the mean of the same cell in all texts."""
    cells = [CELL.findall(t) for t in texts]
    assert all(len(c) == len(cells[0]) for c in cells), "the logs have different cells"
    means = [round(sum(int(c[i][2]) for c in cells) / len(cells)) for i in range(len(cells[0]))]
    at = iter(means)
    return CELL.sub(lambda m: m.group(0).replace(f"{m.group(3)} cy", f"{next(at)} cy"), texts[0])


if __name__ == "__main__":
    out, logs = sys.argv[1], sys.argv[2:]
    open(out, "w").write(merged([open(l).read() for l in logs]))
