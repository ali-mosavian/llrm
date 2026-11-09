#!/usr/bin/env python3
"""Mean over code paddings of the 6502 bench: tools/bench6502_pads.py OUT LOG...
Each LOG is one `BENCH_PAD=n` run of `cargo test -p llrm-6502 --test bench -- --ignored --nocapture`.
sim65 charges a cycle per page-crossing branch or indexed access, so one layout's cycles move by
about 2% with the code's address. The cycles of a cell are averaged, its bytes and instructions
are the first log's (they do not change with the padding). OUT has the format of a LOG.
With `--min` a cell's cycles are the least over the layouts: the layout lottery made visible next to the mean (diagnosis only; the gate is the mean)."""
import re, sys

CELL = re.compile(r"(\d+) B(?: linked)?(?:, (\d+) B body)?, (\d+) cy(?:, (\d+) in)?")


def _cells(text):
    """{(table header, kernel, column): the cell text} of every table row in a log. A log prints its tables in the order the
    parallel tests finish, so two logs of one run need not list them in the same order (2026-11-09: the O2 variant merges
    averaged the cycles of different kernels)."""
    out, header = {}, None
    for line in text.splitlines():
        if not line.startswith("|"):
            header = None if not line.strip() else header
            continue
        row = [c.strip() for c in line.strip().strip("|").split("|")]
        if row[0] == "kernel":
            header = tuple(row)
        elif header and len(row) == len(header) and not set(row[0]) <= {"-"}:
            for column, cell in zip(header[1:], row[1:]):
                out[(header, row[0], column)] = cell
    return out


def summary(text):
    """'N tables, kernels per table: a,b,...' so a merge that lost or doubled a table is visible in the log."""
    kernels = {}
    for header, kernel, _ in _cells(text):
        kernels.setdefault(header, set()).add(kernel)
    return f"{len(kernels)} tables, kernels per table: " + ",".join(str(len(k)) for k in kernels.values())


def merged(texts, reduce=None):
    """The first text with every cell's cycles the mean (or `reduce`, e.g. min) of the same cell, found by table, kernel and
    column, in all texts. The logs must hold the same cells, and a cell's bytes cannot move with the padding."""
    tables = [_cells(t) for t in texts]
    assert all(t.keys() == tables[0].keys() for t in tables), "the logs have different cells"
    pick = reduce or (lambda v: round(sum(v) / len(v)))
    out_lines, header = [], None
    for line in texts[0].splitlines(keepends=True):
        if line.startswith("|"):
            row = [c.strip() for c in line.strip().strip("|").split("|")]
            if row[0] == "kernel":
                header = tuple(row)
            elif header and len(row) == len(header) and not set(row[0]) <= {"-"}:
                for column, cell in zip(header[1:], row[1:]):
                    found = [CELL.search(t[(header, row[0], column)]) for t in tables]
                    if not all(found):
                        assert not any(found), f"{row[0]} {column}: a cell in some logs only"
                        continue
                    assert len({f.group(1) for f in found}) == 1, f"{row[0]} {column}: bytes differ between the logs"
                    mean = pick([int(f.group(3)) for f in found])
                    line = line.replace(cell, CELL.sub(lambda m: m.group(0).replace(f"{m.group(3)} cy", f"{mean} cy"), cell), 1)
        out_lines.append(line)
    return "".join(out_lines)


if __name__ == "__main__":
    args = sys.argv[1:]
    least = "--min" in args
    out, logs = [a for a in args if a != "--min"][0], [a for a in args if a != "--min"][1:]
    texts = [open(l).read() for l in logs]
    open(out, "w").write(merged(texts, min if least else None))
    print(f"{out}: {len(logs)} logs, {summary(texts[0])}")
