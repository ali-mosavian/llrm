# Nib reference twins

A twin is a C source with a Nib program's semantics, `bench/NAME/twin/SEMANTIC.c`, `SEMANTIC` one of `init` (locals zeroed) or `bounds` (indices checked). `bench.py` measures it as `NAME~SEMANTIC` and gives it to bcc/bc/tc/tcc/ow; it is not judged or ratcheted. A Nib program is held to the C compilers' bytes on its twin (`bar_program`).

Write one per program:

- Copy `bench/NAME/NAME.c`; keep the algorithm and the `NAME.out` output byte for byte.
- `init`: zero every local the Nib program zeroes (`= {0}`, `= 0`).
- `bounds`: wrap every array index in `CK(i, n)` (`(unsigned)(i) < (n) ? (i) : (ebnd(), 0)`) with `extern void ebnd(void); void ebnd(void) {}`; check what Nib checks, no more.
- Do not shortcut the Nib source's work; the twin must not be cheaper than Nib by construction.
- Run `bench.py --lang c --opt O1 Os NAME~bounds` and confirm `checked` is true.
- Reference: `bench/grep/twin/bounds.c`.
- Fixed-point programs: every Q operation is a 64-bit intermediate then shift (`docs/frontends/nib/language-spec.md` §3.4), written with `long long`. Borland C has no 64-bit type: tc/bcc/tcpp keep their column on the plain `NAME.c`, not on the twin.
