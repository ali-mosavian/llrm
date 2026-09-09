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

## Consequences for qbopt

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
delete the floating sequence or assume a rounding mode. Strict floating
operations also stop dead-store elimination and loop-store sinking: an
exception can expose memory before a later overwrite.

PDS/VBDOS FPCSE now executes one such iteration, seeded with SINGLE 438.75
after its first load, and still prints 487.5. The numeric proof does not yet
cover QuickBASIC's constant-pool inputs or runtime-input FPCSEX.
