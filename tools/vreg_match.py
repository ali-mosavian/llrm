#!/usr/bin/env python3
"""Match virtual registers of our allocator input to the pin's, then list the
first allocation decisions that differ.

    vreg_match.py --ours-in ours.err --ours-dec ours.dec \
                  --pin-mir pin.mir --pin-ra pin.ra --function f

Vreg numbers differ between the two, so registers are matched by the
instruction that defines them: both pre-allocation streams reduce to
(mnemonic, #defs, #uses) and are aligned with difflib; aligned instructions
pair their defined vregs.

  ours.err  `LLRM_INPUT_DUMP=<fn>` lines `IN b0[1] #id d[..] u[..] .. Some((Some("mov"), ..`
  ours.dec  `LLRM_GREEDY_DECISIONS` lines `dec v6 stage=Assign .. -> assign EDX`
  pin.mir   MIR before greedy (`llc -stop-before=greedy`, or -print-before=greedy)
  pin.ra    `llc -debug-only=regalloc`: `selectOrSplit CLS:%N ..`, `assigning %N to $reg`
"""
import argparse
import collections
import difflib
import re


def mnemonic(name):
    """`MOV32rr` -> `mov`, `COPY` -> `mov`, `mov` -> `mov`."""
    name = name.strip()
    if name == "COPY":
        return "mov"
    m = re.match(r"([A-Za-z]+?)(?:\d|$|_)", name)
    mn = (m.group(1) if m else name).lower()
    # Condition codes are an operand in the pin (JCC, SETCC, CMOV), the mnemonic in ours.
    if re.fullmatch(r"j(?!mp)[a-z]+", mn):
        return "jcc"
    if re.fullmatch(r"set[a-z]+", mn):
        return "setcc"
    if re.fullmatch(r"cmov[a-z]+", mn):
        return "cmovcc"
    return mn


def ours_stream(text):
    """[(mnemonic, [def vregs], [use vregs])] in program order."""
    out = []
    rx = re.compile(r'^IN b\d+\[\d+\] #\d+ d\[([^\]]*)\] u\[([^\]]*)\] .*?Some\(\(Some\("([^"]*)"')
    for line in text.splitlines():
        m = rx.match(line)
        if m:
            ids = lambda s: [int(x) for x in s.split(",") if x.strip()]
            out.append((mnemonic(m.group(3)) or "pseudo", ids(m.group(1)), ids(m.group(2))))
    return out


def pin_stream(text):
    out = []
    for line in text.splitlines():
        s = re.sub(r"^\d+[Br]\s+", "", line.strip())  # slot index prefix
        if not s or s.startswith(("bb.", "#", ";", "---", "...")):
            continue
        lhs, eq, rhs = s.partition(" = ")
        if not eq:
            lhs, rhs = "", s
        rhs = re.sub(r"\(.*?\)", "", rhs)
        op = rhs.split()[0] if rhs.split() else ""
        if not re.match(r"^[A-Z]{2,}[A-Za-z0-9_]*$", op):
            continue
        defs = [int(x) for x in re.findall(r"%(\d+)", lhs)]
        uses = [int(x) for x in re.findall(r"%(\d+)", rhs)]
        out.append((mnemonic(op), defs, uses))
    return out


def shapes(stream):
    """Per instruction: mnemonic, #defs and, for each use, the mnemonic of
    the instruction that last defined it (so the token carries local
    dataflow). Uses defined by the entry pseudo are physical registers: the
    pin has none of them as vregs, so they are left out."""
    last, out = {}, []
    for mn, defs, uses in stream:
        feeds = tuple(sorted(last.get(u, "?") for u in uses if last.get(u) != "pseudo"))
        out.append((mn, len(defs), feeds))
        for d in defs:
            last[d] = mn
    return out


def match(ours, pin):
    """({our vreg: pin vreg}, fraction of aligned uses that agree with it)."""
    sm = difflib.SequenceMatcher(None, shapes(ours), shapes(pin), autojunk=False)
    pairs = [(ours[a0 + k], pin[b0 + k])
             for tag, a0, a1, b0, b1 in sm.get_opcodes() if tag == "equal" for k in range(a1 - a0)]
    votes = collections.defaultdict(collections.Counter)
    for o, p in pairs:
        for ov, pv in zip(o[1], p[1]):
            votes[ov][pv] += 1
    mapping = {o: c.most_common(1)[0][0] for o, c in votes.items()}
    agree = total = 0
    for o, p in pairs:
        pu = set(p[2])
        for u in o[2]:
            if u in mapping:
                total += 1
                agree += mapping[u] in pu
    return mapping, (agree / total if total else 0.0)


def reg_key(name):
    """`EDX`, `$rdx`, `$edx`, `R8D`, `$r8d` -> a width-free key."""
    n = name.lower().lstrip("$")
    n = re.sub(r"^([re])(?=[a-ds][xil]|[sb]p|[sd]i)", "", n)
    return re.sub(r"^(r\d+)[dwb]$", r"\1", n)


def ours_decisions(text, function=None):
    """[(vreg, reg key or "other")]; `== <fn>` header lines split a multi-function log."""
    out, current = [], None
    for line in text.splitlines():
        if line.startswith("== "):
            current = line[3:].strip()
        if function and current != function:
            continue
        m = re.match(r"dec v(\d+) .*-> (assign (\S+)|.*)", line)
        if m:
            out.append((int(m.group(1)), reg_key(m.group(3)) if m.group(3) else "other"))
    return out


def pin_decisions(text, function):
    """[(vreg, reg key or "other")] of the last greedy run of `function`."""
    runs, cur, in_fn = [], None, False
    for line in text.splitlines():
        if line.startswith("********** GREEDY REGISTER ALLOCATION"):
            cur = None
            in_fn = "pending"
        elif in_fn == "pending" and line.startswith("********** Function:"):
            cur = [] if line.split()[-1] == function else None
            if cur is not None:
                runs.append(cur)
            in_fn = False
        elif cur is not None:
            m = re.match(r"selectOrSplit \S+:%(\d+) ", line)
            if m:
                cur.append([int(m.group(1)), "other"])
                continue
            m = re.match(r"assigning %(\d+) to (\$\S+?):", line)
            if m and cur and cur[-1][0] == int(m.group(1)):
                cur[-1][1] = reg_key(m.group(2))
    return [tuple(d) for d in runs[-1]] if runs else []


def report(ours_in, ours_dec, pin_mir, pin_ra, function, limit=15):
    mapping, agreement = match(ours_stream(ours_in), pin_stream(pin_mir))
    inverse = collections.defaultdict(list)
    for o, p in mapping.items():
        inverse[p].append(o)
    ours = {}
    for v, reg in ours_decisions(ours_dec, function):
        ours.setdefault(v, reg)  # the first decision of a vreg; later ones are its splits
    lines = [f"matched {len(mapping)} vregs ({agreement:.0%} of aligned uses agree); ours decided {len(ours)}"]
    differ = same = unmatched = merged = 0
    for p, reg in pin_decisions(pin_ra, function):
        os_ = [o for o in inverse.get(p, []) if o in ours]
        if not os_:
            unmatched += 1
        elif len(os_) > 1:
            merged += 1  # one pin vreg is several of ours: a coalescing difference, not an assignment one
            if merged <= limit:
                lines.append(f"pin %{p} -> {reg}   ours has {len(os_)} vregs: " + " ".join(f"v{o}->{ours[o]}" for o in os_[:6]))
        elif ours[os_[0]] == reg:
            same += 1
        else:
            differ += 1
            if differ <= limit:
                lines.append(f"pin %{p} -> {reg}   ours v{os_[0]} -> {ours[os_[0]]}")
    lines.append(f"same {same}  differ {differ}  pin vreg = several of ours {merged}  pin decisions without a match {unmatched}")
    return "\n".join(lines)


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ("ours-in", "ours-dec", "pin-mir", "pin-ra"):
        ap.add_argument("--" + name, required=True)
    ap.add_argument("--function", required=True)
    ap.add_argument("--limit", type=int, default=15)
    a = ap.parse_args()
    read = lambda p: open(p, errors="replace").read()
    print(report(read(a.ours_in), read(a.ours_dec), read(a.pin_mir), read(a.pin_ra), a.function, a.limit))
