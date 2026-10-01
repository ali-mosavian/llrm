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

## SIN and COS

BC calls the runtime's `B$SIN8` and `B$COS8`; llrm computes them with the
x87's `fsin` and `fcos`. Measured over 40 arguments (DOSBox, QB 4.5, DOUBLE
bytes) against the C library's correctly rounded values: BC's results are up to
15 ulps off (mean 0.9), llrm's at most 1 (mean 0.1). 29 of the 40 differ in
their last bits. TAN, ATN, LOG, EXP and SQR agree exactly over 24 arguments.

Matching BC bit for bit means calling its routine: the argument goes in ST0
and the answer comes back in ST0, which the rich route's runtime calls do not
yet do (floats pass on the stack), and the call is 5 bytes where `fsin` is 2.
For a less accurate answer. Not done; pinned by `tests/qb-bc/sin-cos`
(llrm's output; BC must differ).
