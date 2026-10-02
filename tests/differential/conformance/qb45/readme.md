# QB 4.5 compatibility suite

This is the base of the QB/PDS/VBDOS compatibility ladder. Every program is a
standalone QB 4.5 source file: it prints exactly its manifest `expected` value
on success, and a uniquely named `FAIL <case> <check>` value at the first
failed invariant. Verdicts use bare `PRINT`, so COMMAND.COM captures them with
`EXE > RESULT.TXT`; `OPEN "CON"` and file-handle verdicts are rejected because
the named console device bypasses that redirection.
Run a case in an otherwise empty DOS working directory because `Q45F10.BAS`
creates `q45f10.tmp` before removing it. Do not substitute FreeBASIC, QB64,
or a host BASIC runtime for QB 4.5's compiler and BCOM/BRUN runtime.
The `environment` manifest table is part of the runtime recipe, not ambient
host state; the environment case preallocates its private `QBCOMPAT` entry so
replacement/removal behavior is deterministic in QB45's fixed DOS block.

`suite.toml` is deliberately machine-readable. `parser`, `lowering`, and
`runtime` state the completion obligation, not a claim that a current build
has met it:

- `required` is a deterministic compatibility obligation for the completed
  frontend and selected Microsoft runtime;
- `pending` is a deterministic QB 4.5 probe whose lowerer/runtime path still
  needs measurement and integration; and
- `reference-only` is legal QB 4.5 source whose display/event result needs a
  DOS display or event instrument, so it is not a headless golden-output gate.

The current frontend has not completed this manifest. `--emit-frontend` uses
the real HIR/MIR/OMF path and records failures beside partial stage dumps;
`--run-frontend` fails explicitly because linking the emitted object to the
selected Microsoft runtime is not yet integrated. A `required` row remains an
obligation until that exact qb object links to BCOM45 and its redirected DOS
behavior is captured.

The sources progress from lexical/numeric/control-flow behavior to procedures,
arrays, strings, UDTs, DATA, sequential and RANDOM files, errors, segment
memory, display, events, module chaining, native calls, and static procedure
storage. `Q45G16.BAS` clears SCREEN 13, sets its two
corner pixels, checks them with POINT, then copies the documented mode-13
64,000-byte `A000:0000` graphics page with BSAVE. No `Q45G13.BSV` artifact is
declared because no original-toolchain golden has been measured.
Numeric checks avoid formatting-sensitive output;
the executable assertion is the one PASS/FAIL line. `Q45M12.BAS` changes only
the middle INTEGER through its address, checks two adjacent canaries, and
resets `DEF SEG`. `Q45E14.BAS`
checks registration and a manually invoked event routine; a separate
instrumented DOS run must establish asynchronous TIMER dispatch.

The QBasic-port documentation and parser tests at
`~/work/personal/qbasic-port` were consulted to choose scenarios, but they are
not an oracle for acceptance, lowering, ABI, or runtime behavior. Microsoft QB
4.5 compilation, raw object inspection where ABI matters, and BCOM/BRUN output
remain the authority. New sources use explicit `AS` declarations where QB45's
grammar permits them; legacy QB function return types and external library
declarations retain the type characters the Microsoft grammar requires. The
suite does not use underscored QB45 identifiers, preprocessor directives,
FreeBASIC syntax, ABI, or runtime facilities.

Hardware and integration coverage is explicitly represented but remains
reference-only until a QB 4.5 DOS run measures it: display modes/palette/page
semantics, graphics commands, and asynchronous keyboard, serial, music,
light-pen, joystick, and timer delivery. The deterministic source portions of
those cases still check registration and handler calling convention without
screen scraping or interactive input. `Q45C17.BAS` runs with
`Q45N17.BAS`; all remaining cases are single modules. The remaining
unrepresented QB 4.5 facilities are device- or service-dependent: printer
formatting, terminal input, child-process behavior, network locking, and
hardware port polling. They need a measured DOS fixture rather than invented
golden behavior. Directory mutation and private environment-variable behavior
now have deterministic DOS fixtures.

## QuickHelp inventory and evidence

`coverage.toml` is an exhaustive, machine-readable inventory of the 989
unique topics decoded from the installed Microsoft `QB45QCK.HLP` (200 topics),
`QB45ADVR.HLP` (533), and `QB45ENER.HLP` (256). It records the topic context,
title, class, case mapping, completion status, and a source/line evidence
pointer. The nested `QB45/QB45/HLP` copy has the same SHA-256 values and is not
counted twice. A `covered` row points at a source line that constructs the
documented form. `required-reference` means the feature is represented but
still needs a device, process, external-object, or fault instrument; those
rows are deliberately not claimed as executable output passes. `out-of-scope`
rows are QuickBASIC editor, Advisor, or compiler UI documentation rather than
BASIC program behavior.

The inventory was extracted with
`/Users/alim/work/personal/qb-qrender/tools/hlpextract.py`; its QuickHelp
decoder is the evidence reader, while QB 4.5's `BC.EXE` and BCOM45 runtime are
the behavioral authority. The bounded original-compiler probe compiled the
new deterministic sources with `/O /D` and produced zero severe errors. It
also tested two QuickHelp restrictions as acceptance probes. Both internal
and outer-label `GOTO`s from an include file compiled with zero severe errors.
The `GOTO` results contradict the documented
restriction. The 42-character identifier is rejected with `Syntax error`,
agreeing with the documented 40-character maximum. The probes remain in
`negative/` as observed compiler behavior rather than assumed rejects.

A bounded compile-only sweep copied the original 36 suite cases to an isolated DOS work
drive and used the same compiler/options. QB 4.5 requires a leading comment
for sources that otherwise begin with a declaration; after adding a neutral
apostrophe-comment prologue where needed, all 36 sources compile
with zero severe errors. This is an acceptance measurement only: headless
runtime verdict capture remains a separate qb-frontend/link/DOS gate. The
five initial focused additions (`conversions`, `math-functions`,
`string-parts`, `file-positions`, and `signed-array-bounds`) were separately
compiled, linked, and run. A second bounded run did the same for 12 split cases
covering numeric/character/text functions, binary packing,
arithmetic/relational/logical/precedence behavior, typed sequential I/O,
string mutation, and exact divide-by-zero/subscript errors. All 17 produced
their exact expected oracle line with zero severe compiler errors. The second
run caught two test-oracle defects before check-in: VAL's decimal conversion
differs from the source literal by about 1.4e-14, so the assertion now uses an
independently chosen tolerance, and `(2+3)*5` is 25, not the mistyped 35.
Those Microsoft runs establish expectations only and do not pass the qb
frontend.

A third bounded original-QB oracle run compiled, linked, and redirected ten
additional focused cases: exact `PRINT USING` file bytes, CINT/CLNG half-tie
rounding, binary GET/PUT positions and byte layout, all ON GOSUB targets plus
ON GOTO and out-of-range fallthrough, pre/post DO forms and signed FOR steps,
SELECT CASE list/range/relation/else forms, and deterministic DOS directory
mutation/rename/removal, RND/RANDOMIZE mode sequences, floating pack/unpack
bytes, and negative DOUBLE SQR/NaN behavior. All ten produced their exact
expected line. A
fail-first directory probe also caught `CURDIR$` being treated as an empty
implicit string variable rather than a QB 4.5 intrinsic; that unsupported
claim was removed, and later path-sensitive file operations now independently
witness CHDIR. Two further oracle probes caught decimal DATA rounding at the
DOUBLE byte level and the false assumption that negative SQR raises BASIC
error 5; the fixtures now construct an exact quarter arithmetically and assert
the measured unordered NaN with no BASIC error.

A fourth bounded original-QB run compiled, linked, and redirected seven more
focused cases. Exact witnesses now cover LSET/RSET justification and
right-truncation, all five FILEATTR mode codes plus distinct DOS handles,
RESET and CLEAR buffer flushing/closure with exact ERR/ERL and post-RESUME
checkpoints, SHARED aliasing, STATIC-local persistence with shared-name
shadowing, and ENVIRON insertion/replacement/removal observed through
ENVIRON$. All seven produced their exact expected line. Fail-first probes
removed an invalid BYVAL-on-BASIC-SUB case after QB45 and its help established
that BYVAL is for declared non-BASIC procedures, changed the STATIC fixture to
QB45's parameterized FUNCTION grammar, and made environment preallocation an
explicit manifest precondition after an unseeded DOS environment raised
out-of-memory. CLEAR's pre-state also produces the independent `QnB` file
witness before it is reset, preventing dead setup from making a missing clear
look correct. Those are oracle/setup corrections, not qb-frontend passes.

A fifth bounded run added four more deterministic core cases: explicit LET
scalar and same-type record assignment, COMMAND$ with manifest-pinned DOS
arguments, synchronous SHELL with an exact child-produced file, and TAN at
positive, negative, and zero inputs. All four compiled, linked, and produced
their exact redirected oracle line. The fail-first COMMAND$ run also measured
that this COMMAND.COM batch path uppercases the mixed-case arguments, so the
fixture checks `ALPHA BETA42` rather than preserving an invented casing rule.

Ordinary comments and metacommands use apostrophe form throughout the corpus.
The only retained `REM` statement is the explicit comment-equivalence witness
in `Q45L00.BAS`; its manifest opts into that exception and the validator
rejects `REM` in every other case.

## Redirected verdict measurement

The discarded `OPEN "CON"` convention produced a zero-byte redirected file in
headless DOSBox-X. The corpus now uses only bare `PRINT` for PASS/FAIL verdicts;
the runner invokes `EXE > RESULT.TXT` and byte-compares the captured line with
the manifest expectation. `PRINT #` remains only inside cases that deliberately
test file-channel I/O.

`text-screen-capture` remains the first case and makes the separate display
instrument explicit. `Q45L00.BAS` clears color text mode, selects `DEF SEG = &HB800`,
and directly POKEs `QB45TEXT` plus attribute `07` at row 1, column 1 before
BSAVEing all 4,000 text-page bytes as `QBTEXT.BSV`. Its runtime assertion also
checks that an apostrophe-commented `qCount = 99` and a deliberate `REM`
`qTotal = 1` did not execute after the normal assignments; only that success
branch BSAVEs the artifact. QB 4.5's BSAVE header is seven bytes:
`FD 00 B8 00 00 A0 0F`. The byte oracle then requires `51 07 42 07 34 07 35
07 54 07 45 07 58 07 54 07` at offsets 7--22: the eight CP437 character bytes
for `QB45TEXT` interleaved with attribute `07`. The original-QB run produced a
4,007-byte artifact whose complete bytes are stored as `golden/QBTEXT.BSV`.
The runner must compare the full artifact and report the marker/header offsets
when it differs.
