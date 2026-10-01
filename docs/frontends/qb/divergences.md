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
