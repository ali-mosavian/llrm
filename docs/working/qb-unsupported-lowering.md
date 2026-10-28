# Lowering the QB constructs `semantic.rs` still refuses

Use across 815 real programs (repo, private tools): PALETTE USING 5, ON [LOCAL] ERROR RESUME NEXT 3, REDIM PRESERVE 1,
OPEN FOR ISAM 1, END/SYSTEM with a code 0. Order follows use. One PR each, with a PDS 7.1 BC measurement in the body.
QB 4.5's BC refuses PALETTE USING and RESUME NEXT; measure with PDS 7.1 (`/E /X`).

## PALETTE USING array(i)  (PDS 7.1 BC)
`SCREEN 13: PALETTE USING p(0)` (p AS LONG): `call B$CSCN` (mode), then a far pointer built on the stack
(`push ds; pop es; push es; push <p offset>`), `mov bx,<fixup>; mov [bx+2],ds; push bx`, `call far B$PALU`.
Externs: B$GRPUSED, B$PALU. QB 4.5's `rt/gwpal.asm`: `B$PALU(fpData far, pAd near)`, cleanup 6, the same operands
`graphics_array` builds for GET/PUT; the count of entries is what remains of the array from `fpData` (element size from the
descriptor); -1 leaves an attribute alone. Needed: a contract row for B$PALU in `crates/frontends/qb/runtime.toml`, a C `B$PALU` beside
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

## REDIM PRESERVE
BC (PDS 7.1) calls `B$RDMP` with `B$RDIM`'s operands. The only real file using it (vbdos `INC/HELP.BAS`) preserves a
UDT array and a STRING array. The runtime (#4404) preserves raw-byte elements; a string array raises Subscript out of
range at run time. The compiler keeps the call and does NOT refuse a string array at compile time: a compile failure
would stop HELP.BAS compiling at all, a run-time error only the path that reaches it. String-array preserve (move the
string descriptors, re-own their strings) is due 2026-11-06 with a fail-first `tests/run/qb` program under DOSBox.
Order after #4329: PALETTE USING row, RESUME NEXT, string-array preserve.

## KILL, NAME, MKDIR, CHDIR, RMDIR
BC calls `B$KILL(psd)`, `B$NAME(psdOld, psdNew)`, `B$MDIR`, `B$CDIR`, `B$RDIR` (QB 4.5 `rt/dkstmt.asm`, `rt/directry.asm`;
pascal far, callee cleanup 2, 4, 2, 2, 2). llrm's runtime runs them over the OS layer's `files` group
(`runtime/shared/interface.toml`: delete, rename, mkdir, chdir, rmdir; DOS int 21h 41h, 56h, 39h, 3Bh, 3Ah). Error
numbers match BCOM45 in `tests/qbrt/file_names.bas`. KILL with `*` or `?` deletes each match found by the layer's
find-first/next (`tests/qbrt/kill_wildcard.bas`). Not done: KILL of a file open on a channel (BC's `B$CHKFOPEN` answers File already open).

## FILES
BC pushes the pattern's descriptor to `B$FILS` (an empty one for a bare FILES; QB 4.5 `rt/dkstmt.asm`). The runtime
(`runtime/qb/filelist.c`) prints the current directory, the entries 18 characters each (four to an 80-column line), a blank
line and the free space, over the OS layer's `find_first`, `find_next`, `current_directory`, `current_drive` and
`free_bytes`. `tests/qbrt/files_listing.bas` matches BCOM45 at m16 and m32.

## SHELL
BC calls `B$SSHL(psd)` with the command's descriptor, an empty one for a bare SHELL (QB 4.5 and PDS 7.1 BC output;
`rt/osstmt.asm`). The runtime (`runtime/qb/shell.c`) runs COMSPEC with `/C command` over a new OS-layer op, `execute`:
DOS 4B00h at m16, a DPMI real-mode call at m32 (DOS/32A's own 4Bh takes an undocumented block layout). At m16 the far heap's
arrays slide down and the DOS block shrinks to them for the child, then regrows (`qb_shell_release`/`qb_shell_reclaim`).
`tests/qbrt/shell_command.bas` matches BCOM45 at m16 and m32; `tests/run/c/os_execute.c` tests the op. Not done: the child's
exit code is not ERRORLEVEL-visible, and SHELL does not save and restore the screen or COM ports as QB's B$ShellSetup does.

## FILEATTR
BC calls `B$FATR(channel, field)` returning a LONG (QB 4.5 `rt/dvstmt.asm`): field 1 is the open mode (INPUT 1, OUTPUT 2,
APPEND 8, BINARY 32), 2 the DOS handle, any other Illegal function call. `tests/qbrt/file_attributes.bas` matches BCOM45 at m16
and m32. OPEN FOR RANDOM with `LEN =` is still refused by the compiler (separate gap).

## ENVIRON
BC calls `B$SENV(psd)` (QB 4.5 `rt/osstmt.asm`). The runtime (`runtime/qb/environ.c`) edits the program's environment block in
place over two OS-layer ops, `environment_read` and `environment_write`. As in QB, the table cannot grow past the end it had at
start (`b$env_len`): a variable fits only where the table has shrunk, else Out of memory; an empty name or no delimiter is
Illegal function call. `tests/qbrt/environ_statement.bas` matches BCOM45 at m16 and m32.
## VIEW
BC calls `B$VIEW(x1, y1, x2, y2, fill, border, screen)` (seven words, -1 for a colour left out) or `B$VEW0()` for a bare VIEW
(QB 4.5 `rt/grview.asm`). The runtime keeps a viewport in `gfx.c` that PSET, POINT, LINE, CIRCLE, PAINT, DRAW, GET, PUT and CLS
are cut to; an ordinary VIEW also moves the origin of the statements' coordinates to its corner, VIEW SCREEN does not. The fill
covers the viewport, the border is the box one pixel outside, the cursor goes to the viewport's centre; equal corners or ones off
the screen are Illegal function call. `tests/qbrt/view_graphics.bas` hashes the screen with POINT and matches BCOM45 at m16 and m32.

## WINDOW
BC calls `B$WIND(x1, y1, x2, y2, screen)` (four SINGLEs and a word) or `B$WIN0()` for a bare WINDOW (QB 4.5 `rt/grwindow.asm`,
`grfpinit.asm`). While a window is on, the point-setting calls map real numbers onto the viewport (the whole screen for VIEW
SCREEN): `round((x - xmin) * dx)`, y up unless WINDOW SCREEN; the last point is kept as reals, so STEP is in logical units;
CIRCLE's radius scales by `dx`; POINT(x, y) maps. The rounding is the FPU's, in its wider format: a double product lands on a
half where QB's lies just under it (`WINDOW SCREEN (-50,-20)-(50,20)`, centre (0, 0) is 159.4999...), so `gfx.c` keeps the
product's lost error. `tests/qbrt/window_graphics.bas` hashes the screen with POINT and matches BCOM45 at m16 and m32. Not done:
POINT(n) and PMAP, GET/PUT corners' y order under a window.

## Compile sweep (the driver)
First refusal per file over the 573 sample `.BAS` files (private tools, d32x, dos); a file counts once per procedure it fails in,
so numbers run about three times the files. Re-run after each PR (2026-10-25, on feat/parity plus VIEW):

| Refusal | Count |
|---|---|
| WINDOW | 36 |
| unknown constant CFONTBLOCKSIZE | 33 |
| CURRENCY | 33 |
| invalid character (non-ASCII) | 24 |
| OPEN ACCESS/LOCK/LEN/RANDOM (#4652) | 20 |
| string CONST (#4720) | 18 |
| WIDTH one-arg (#4722) | 9 |
| SCREEN extra arguments | 9 |
| NAME (#4667) | 9 |

UDT array and a STRING array. The runtime preserves both: a string array's kept strings follow to the new elements
(`str_owner_moved`) and only the cut-off ones are released. Tested on the host (`tests/hostarray/preserve_test.c`).

## SCREEN mode, colorswitch, active page, visible page
`B$CSCN` takes a count-led block like `B$COLR`: a presence word (and the value) per argument, then the count. The runtime
sets the mode, then the pages (`gd_set_pages`: draws to page `apage`, shows `vpage` by INT 10h AH=05h). Not done: the
colorswitch is ignored (QB with `SCREEN 0, 0` changes the video mode, and the screen is cleared at exit), and a text-mode page
other than 0 is Illegal function call where QB has pages. Test: `tests/qbrt/args_screen.bas`.
