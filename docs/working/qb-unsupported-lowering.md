# Lowering the QB constructs `semantic.rs` still refuses

Use across 815 real programs (repo, private tools): PALETTE USING 5, ON [LOCAL] ERROR RESUME NEXT 3, REDIM PRESERVE 1,
OPEN FOR ISAM 1, END/SYSTEM with a code 0. Order follows use. One PR each, with a PDS 7.1 BC measurement in the body.
QB 4.5's BC refuses PALETTE USING and RESUME NEXT; measure with PDS 7.1 (`/E /X`).

## PALETTE USING array(i)  (PDS 7.1 BC)
`SCREEN 13: PALETTE USING p(0)` (p AS LONG): `call B$CSCN` (mode), then a far pointer built on the stack
(`push ds; pop es; push es; push <p offset>`), `mov bx,<fixup>; mov [bx+2],ds; push bx`, `call far B$PALU`.
Externs: B$GRPUSED, B$PALU. QB 4.5's `rt/gwpal.asm`: `B$PALU(fpData far, pAd near)`, cleanup 6, the same operands
`graphics_array` builds for GET/PUT; the count of entries is what remains of the array from `fpData` (element size from the
descriptor); -1 leaves an attribute alone. Needed: a contract row for B$PALU in `runtime/qb/src/runtime.toml`, a C `B$PALU` beside
`B$PAL2` in `runtime/qb/gfx.c` (attribute i takes element i; LONG array in DAC modes, INTEGER in EGA modes), the lowering
at `"PALETTE_USING"` in `semantic.rs`, and the argument layout checked against BC's.

## ON ERROR RESUME NEXT  (PDS 7.1 BC)
**BC.** `push cs; push 0FFFEh; call far B$OEGA`: the ON ERROR registration with 0FFFEh where a handler address goes.
After a failing statement the program continues at the next one (probe: OPEN of a missing file, then PRINT ERR gave 53).
QB 4.5's runtime source has no such sentinel (it is PDS-only), so the behaviour above is measured, not read.

**This compiler already has the mechanism.** `ON ERROR GOTO h` registers a handler block through the
`$QB$OERG:<block>:<scope>` marker (`llrm-hir/src/mir/handling.rs`); a `RESUME NEXT` inside `h` is `B$RESN`, which the
MIR lowering turns into a switch back to the statement after the one that faulted (`Resume::Next`, `handling.resumes`).
So ON ERROR RESUME NEXT is `ON ERROR GOTO h` where `h:` holds just `RESUME NEXT`.

**Shape.** Per procedure with an ON ERROR RESUME NEXT, one synthetic handler block that emits `B$RESN`; the statement
registers it exactly as ON ERROR GOTO registers its label's block (`semantic.rs`, `Statement::OnError`). The pre-pass
that collects handlers (`error_handlers`, `error_handler`, `error_handler_local`; `semantic.rs` near line 1121 and 1437)
must see `OnErrorResumeNext` too, as `ON [LOCAL] ERROR RESUME NEXT` carries the same `local` flag. ERR and ERL work as
for any handler. Cost: nothing on the path with no error; the same resume switch an explicit handler already pays.

**Rejected: a check after every statement.** It adds a compare and branch per statement that can raise (every I/O, every
array access under /D) to a path that mostly never errors; the resume switch is built once per procedure instead.

**Test.** A program that OPENs a missing file under ON ERROR RESUME NEXT, then PRINTs ERR, run in DOSBox: 53 as BC prints;
and the MIR has one handler block and no per-statement check.
