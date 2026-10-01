# Where llrm-qb differs from BC

Differences kept on purpose. Each is pinned by a test that runs llrm's own
behaviour and fails if BC comes to agree (`tests/test_qb_matches_bc.py`).

## The target of an overflowing INTEGER statement

Under BC /D (llrm `-ftrapv`) an INTEGER overflow raises error 6. llrm leaves
the target unchanged. BC, for `x = x + e` and `x = x - e` on a scalar, adds
into the variable and so stores the wrapped sum before reporting: after
`RESUME NEXT`, `x% = 32767: x% = x% + 1` leaves -32768 in BC and 32767 in
llrm. For any other statement shape BC leaves the target unchanged too.

The shapes are BC's instruction selection (`add [x],ax` with `jo`), not a
language rule; llrm does not copy them. Pinned by `tests/qb-bc/overflow-target-unchanged`.

## A chain of INTEGER additions

BC checks the overflow flag of the instructions it emits, in an order of its
choosing: `a = b = c = 20000`, `x = a + b - c` raises no error in BC and
yields 20000, and raises error 6 in llrm at `a + b`; `x = a + b + c` raises
in both. Not a rule BC states; llrm checks each operation.

## A constant times a quotient

BC reassociates `c1 * (x / c2)` into `x * (c1 / c2)`, folding `c1 / c2` to a
SINGLE constant at compile time: `.5 * (w / 7) * 1#` with `w% = -1` is the
SINGLE -(.5 / 7) in BC and the full-precision quotient in llrm, a last-bit
difference. llrm folds the constant subexpressions it is given (`(.5 / 7) * w`)
and does not reassociate. Not pinned: BC's rewrite is its optimizer's.
