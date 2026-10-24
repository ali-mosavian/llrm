# Lowering the QB constructs `semantic.rs` still refuses

Use across 815 real programs (repo, private tools): PALETTE USING 5, ON [LOCAL] ERROR RESUME NEXT 3, REDIM PRESERVE 1,
OPEN FOR ISAM 1, END/SYSTEM with a code 0. Order follows use. One PR each, with a PDS 7.1 BC measurement in the body.
QB 4.5's BC refuses PALETTE USING and RESUME NEXT; measure with PDS 7.1 (`/E /X`).

## PALETTE USING array(i)  (PDS 7.1 BC)
`SCREEN 13: PALETTE USING p(0)` (p AS LONG): `call B$CSCN` (mode), then a far pointer built on the stack
(`push ds; pop es; push es; push <p offset>`), `mov bx,<fixup>; mov [bx+2],ds; push bx`, `call far B$PALU`.
Externs: B$GRPUSED, B$PALU. Needed: a contract row for B$PALU in `runtime/qb/src/runtime.toml`, a C `B$PALU` beside
`B$PAL2` in `runtime/qb/gfx.c` (attribute i takes element i; LONG array in DAC modes, INTEGER in EGA modes), the lowering
at `"PALETTE_USING"` in `semantic.rs`, and the argument layout checked against BC's.

## ON ERROR RESUME NEXT  (PDS 7.1 BC)
`push cs; push 0FFFEh; call far B$OEGA`: the ON ERROR registration with a 0FFFEh sentinel for the handler.
After a failing statement the program continues at the next one (probe: OPEN of a missing file, then PRINT ERR gave 53).
This compiler lowers ON ERROR at MIR level (`llrm-hir/src/mir/handling.rs`, `onerror::goto`), so RESUME NEXT needs a
design: a check after each statement that can raise, or a resume map. Cost in executed instructions to be measured.
