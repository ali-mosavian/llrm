# Floating environment: evidence before reuse

## Established evidence

QuickBASIC 4.5's source tree at
`/Users/alim/work/ms/msdos_60/45` contains:

- `runtime/crt/fpreset.asm:35–41`: `_fpreset` calls the math vector with
  `BX=1` to reset it, then with `BX=4, AX=1332h` to set the control word.
  Its comment says alternate/decimal math ignores that control-word argument.
- `runtime/rt/rtinit.asm:1066`: `B$RTRUNINI` calls `__fpreset`.
- `runtime/rt/erproc.asm:504`: the runtime error path unconditionally calls
  `__fpreset` before invoking BASIC error handling through `B$ONERR`.

For x87, `1332h` decodes as follows:

| Property | Setting |
| --- | --- |
| Invalid operation mask | 0: unmasked |
| Denormal operand mask | 1: masked |
| Divide-by-zero mask | 0: unmasked |
| Overflow mask | 0: unmasked |
| Underflow mask | 1: masked |
| Precision/inexact mask | 1: masked |
| Precision control | 3: extended precision |
| Rounding control | 0: nearest/even |

This establishes what that source implementation requests, not the state at
every call site or a verified runtime contract for PDS, VBDOS, or every
emulator variant. Do not propagate this setting into their MIR by analogy.

LLVM's local source provides the relevant model:

- `llvm/docs/LangRef.md`, constrained floating-point intrinsics: exception
  handling and rounding are separate properties. Ignoring exceptions permits
  assuming masked exceptions and unread status; strict mode does not.
  Strict mode does not guarantee the number or order of exceptions.
- `llvm/lib/Transforms/Scalar/EarlyCSE.cpp`,
  `SimpleValue::canHandle`: constrained arithmetic with strict exceptions is
  rejected, as is dynamic rounding. The latter check explicitly protects
  CSE across calls that could change rounding.

These files were inspected in `/Users/alim/work/other/llvm-project` (the
EarlyCSE file through `git show HEAD:...`, since its working-tree copy is
absent).

## Consequences for llrm

The all-strict default is not an arbitrary blocker to remove. The available
runtime evidence contradicts a blanket masked-exception assumption, and error
handling can reset the floating environment and stack.

Conversely, strict mode is not a proof that every possible reuse is illegal.
It means reuse needs evidence about the operations and intervening effects:

1. Establish the applicable runtime/environment contract without generalizing
   one compiler family's initialization to another.
2. Prove eligible values and operations cannot introduce observable exceptions,
   and account for status observation/reset and exceptional control flow.
3. Keep rounding conversions explicit: an extended arithmetic result is not
   the SINGLE value produced by storing and reloading it.
4. Reuse only with matching value, memory, precision, and environment facts.
   Unknown calls or environment effects invalidate those facts.

The first useful implementation target is exact finite-value analysis for
known inputs, not disabling exception tracking for all floating operations.
FPCSE's literal inputs make it a useful initial case; its runtime-input twin
must remain a separate test of generality. No speedup is established by this
audit, and its provisional numerical target remains provisional.

## Exact final-iteration specialization

An exact finite recurrence can retain its first invariant floating load as
the original exception checkpoint, then install the proven pre-final memory
state and execute the remaining operations once. The load must not read a
cell written by the loop. Every original iteration must evaluate exactly,
with every storage conversion included; calls, unknown effects, live outgoing
flags, and unsupported live-out values reject the transformation.

This preserves initial memory at the first check and reproduces the final
iteration's floating operands, operation order and conversions. It does not
delete the floating sequence or assume a rounding mode. In a module with an
ON ERROR handler, operations that can trap also stop dead-store elimination
and loop-store sinking: the handler can read memory before a later
overwrite. Without a handler a trap ends the program.

FPCSE now executes one such iteration, seeded with SINGLE 438.75
after its first load, and still prints 487.5. The numeric proof does not yet
cover runtime-input FPCSEX.

QuickBASIC's explicit literal bytes are raised as entry memory facts, not as
immutable cells. Only complete direct floating reads of unrelocated,
nonoverlapping BC_CN records in the main body qualify; public pool segments,
missing bytes and procedure entries do not. Calls and aliases invalidate the
facts normally. BC_CN's descriptor records are not assumed to be constants.
Exact floating stores feed memory analysis, so the original literal-loading
FP sequence establishes the same scalar entry values as PDS's integer stores.

## Bounded runtime integers

`crates/llrm-core/src/analysis/floatbounds.rs` now proves numerical exactness without proving a
specific value. Signed integer loads fit extended precision; integer sums,
differences and products qualify only when the entire resulting interval
fits the minimum dynamic precision. Destination conversions must fit too.
Bounds assert neither a zero sign nor a concrete constant. Division, unknown
floating inputs and unproved rounding still fail the proof. CSE retains its
same-block, unchanged-memory and intervening-effects checks.

The typed-MIR regression changes two identical unknown integer conversions
to one shared value. The emitted-code audit of all 60 existing floating
fixtures is unchanged: **no cost or assembly improvement is claimed**.
The expanded 148-object audit (primary corpus, all floating variants and
regression objects in scope) also has no changed outputs. The 61 focused
analysis/CSE checks pass; disabling the bounds makes both unknown-integer
reuse regressions fail as intended.

`tests/suite/fpicse.bas` exposes the missing frontend link. On all three primary
compilers, assigning a runtime LONG to two DOUBLE variables emits two
`B$FILD` calls, not typed FLOADs. The earlier INTEGER variant emitted
`B$FIL2`. These conversions must be recognized at the raise, with verified
helper contracts, before the bounds can remove their duplication. Direct
DOUBLE printing also exposed an unestablished `B$PSR8` lowering interface;
the fixture prints its integral results through CLNG instead.

Current emitted shape, before **and after** this analysis change:

```asm
; materialize input argument
call B$FILD
fstp qword [firstValue]
; materialize the same input argument
call B$FILD
fstp qword [secondValue]
```

The final LONG fixture passes three runtime cases on each compiler. Its
committed objects in `tests/fixtures/regressions/fpicse-{p-g2,q-o,v-g3}.obj`
come from `tools/e2e.py` using the matching `tools/configs.py` configurations,
with zero severe compile errors, in temporary run
`qbopt-fpicse-implicit-stxjen_l`. Source is DOS CRLF, as required by BC.

## Conversion helpers now enter typed MIR

The next implementation closes the demonstrated B$FILD case. The raise
recognizes its two extracted halves of one LONG loaded from unchanged
memory. It requires the established returning, memory-free, flags-only
contract, no live clobbered result, and the object's FIDRQQ linkage. Other
argument shapes and B$FIL2 remain outside this recognition.

Balanced floating regions can now receive SSA values independently of
unmodelled READ/PRINT calls elsewhere in their block. CSE shares the raised
integer conversion, and float allocation duplicates its live result before
the first store. The encoding protocol and relocation provenance stay in
the object module's side maps, not in MIR optimization decisions.

```asm
; before (argument setup abbreviated)
mov ax,[inputValue]
mov dx,[inputValue+2]
call B$FILD
fstp qword [firstValue]
wait
mov ax,[inputValue]
mov dx,[inputValue+2]
call B$FILD
fstp qword [secondValue]
wait

; after (/FPi instructions displayed as x87 equivalents)
fild dword [inputValue]
fld st0
fstp qword [firstValue]
fstp qword [secondValue]
wait
```

| FPICSE compiler | Modeled cost before -> after | Code bytes before -> after |
| --- | --- | --- |
| PDS /G2 | 4540 -> 3660 | 199 -> 170 |
| QB /O | 4562 -> 3682 | 201 -> 172 |
| VBDOS /G3 | 4550 -> 3670 | 201 -> 172 |

Costs use opportunity.py's default loop weighting, not wall-clock timing.
The 148-object audit changes 16 objects (FPICSE and FPEMU variants), all LIR,
with no new refusals. Validation: 46 focused tests; 57 DOS cases including
both programs on three primary compilers and FPEMU under QB event flags.
The initial missing relocation printed zero instead of -32768; the emitted
operand regression fails with that defect restored and passes with the fix.
Disabling helper recognition independently fails all three compiler checks.
Final stage dump: `/tmp/qbopt-fpicse-relocated-final`. Successful runtime
directories: `qbopt-fild-relocated-etv3ig3s` and `qbopt-fild-events-36wa7wbo`.

## INTEGER conversion helpers

The same frontend path now recognizes B$FIL2's 16-bit memory input. It uses
the established contract rather than duplicating its register interface in
a second table. A live DX result or flags still prevents removal: unlike
B$FILD, B$FIL2 sign-extends its argument into DX before loading the FPU.

FPI2CS before/after (relocation names restored, /FPi shown as x87):

```asm
; before                         ; after
mov ax,[inputValue]               fild word [inputValue]
call B$FIL2                      fld st0
fstp qword [firstValue]           fstp qword [firstValue]
wait                             fstp qword [secondValue]
mov ax,[inputValue]               wait
call B$FIL2
fstp qword [secondValue]
wait
```

| Compiler | Modeled cost before -> after | Code bytes before -> after |
| --- | --- | --- |
| PDS /G2 | 3820 -> 3660 | 182 -> 170 |
| QB /O | 3842 -> 3682 | 184 -> 172 |
| VBDOS /G3 | 3830 -> 3670 | 184 -> 172 |

The 151-object audit changes only the three new FPI2CS fixtures; all prior
outputs remain identical. The new fixture's -32768, 123 and 32767 cases
pass on all three compilers. Disabling B$FIL2 recognition fails the emitted
code regression on each compiler; the focused bounds/helper tests total 20.
Objects in `tests/fixtures/regressions/fpi2cs-*.obj` came from `tests/suite/fpi2cs.bas`
through e2e/configs in run `qbopt-fil2-final-agr4msen`. The first attempted
name exceeded the harness's six-character limit once its output prefixes
were added; that run proved nothing. Stage dumps: `/tmp/qbopt-fil2-final-stages`.

## Computed integer inputs

Conversion recognition no longer requires a memory producer. A computed
INTEGER, or a LONG whose halves come from one scalar value, is an ordinary
input to the typed conversion. The unchanged-memory case still uses its
original cell directly. Otherwise floating allocation materializes the
integer into a slot owned by the shared backend frame before FILD; the
normal prologue reserves it. No register or frame choice enters MIR passes.

FPCALC converts `inputValue + 1` twice. Its hot sequence changes from:

```asm
; before, argument setup abbreviated
inc ax
call B$FIL2
fstp qword [firstValue]
; reconstruct inputValue + 1
call B$FIL2
fstp qword [secondValue]

; after
inc ax
mov [bp-2],ax
fild word [bp-2]
fld st0
fstp qword [firstValue]
fstp qword [secondValue]
```

The frame and surrounding allocation costs are included, not hidden:

| Compiler | Modeled cost before -> after | Code bytes before -> after |
| --- | --- | --- |
| PDS /G2 | 3840 -> 3800 | 183 -> 176 |
| QB /O | 3862 -> 3822 | 185 -> 178 |
| VBDOS /G3 | 3850 -> 3810 | 185 -> 178 |

This is about a 1% modeled improvement, not the 19% of the simpler LONG
example. It also removes 18 object bytes from PDS/VBDOS FPEMU. The 154-object
audit changes only those FPEMU variants and the new FPCALC fixtures, all LIR,
with no new refusals. Focused analysis/allocation tests: 55 pass, including
2/4-byte temporary slots and fail-first emitted helper checks. Runtime:
FPCALC on three compilers (9 cases), changed FPEMU on PDS/VBDOS (24 cases).
All 33 pass. Fixtures were compiled from `tests/suite/fpcalc.bas` with the named
e2e/configs configurations in `qbopt-fpcalc-frame-26gr7dl1`; FPEMU validation
is in `qbopt-fpvalue-runtime-a6t2iz_t`. Stage dump:
`/tmp/qbopt-fpcalc-frame-stages`.
