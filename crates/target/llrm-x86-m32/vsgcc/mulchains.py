"""Lower bounds on the cost of c*x as a register chain (shl, lea base+idx*s, add, sub, neg), mod 2^32.
Each op's bytes/clocks are the least the i486 form can take, so a chain found here is never dearer than a real one."""

import sys

M = 1 << 32


# op: (bytes, clocks); lea 3 B / 1 clk, shl r,imm 3 B / 2 clk, add/sub/neg 2 B / 1 clk. copies (mov) are free here.
def ops(vals):
    n = len(vals)
    for i in range(n):
        for k in range(1, 32):
            yield (vals[i] << k) % M, 3, 2
        yield (-vals[i]) % M, 2, 1
        for j in range(n):
            for s in (1, 2, 4, 8):
                yield (vals[i] + vals[j] * s) % M, 3, 1
            yield (vals[i] + vals[j]) % M, 2, 1
            yield (vals[i] - vals[j]) % M, 2, 1


def last_ops(vals, target):
    """The (bytes, clocks) of each single op that turns `vals` into `target`, found by inverting it."""
    found = []
    for a in vals:
        if (
            a << (target.bit_length())
        ) % M == 0 and a:  # shl: a << k == target for some 1 <= k < 32
            for k in range(1, 32):
                if (a << k) % M == target:
                    found.append((3, 2))
                    break
        if (-a) % M == target:
            found.append((2, 1))
        for b in vals:
            if (a + b) % M == target or (a - b) % M == target:
                found.append((2, 1))
            for s in (1, 2, 4, 8):
                if (a + b * s) % M == target:
                    found.append((3, 1))
    return found


def search(target, depth):
    """Pareto set (bytes, clocks) of chains of <= depth ops that reach target."""
    target %= M
    best = {}

    def go(vals, d, b, c):
        if vals[-1] == target:
            best[(b, c)] = d
            return
        if d == depth:
            return
        if d == depth - 1:
            for ob, oc in last_ops(vals, target):
                best[(b + ob, c + oc)] = depth
            return
        seen = set(vals)
        for v, ob, oc in ops(vals):
            if v in seen:
                continue
            go(vals + [v], d + 1, b + ob, c + oc)

    go([1], 0, 0, 0)
    pareto = []
    for b, c in sorted(best):
        if not any(b2 <= b and c2 <= c and (b2, c2) != (b, c) for (b2, c2) in best):
            pareto.append((b, c))
    return pareto


if __name__ == "__main__":
    c, depth = int(sys.argv[1]), int(sys.argv[2])
    print(c, depth, search(c % M, depth))
