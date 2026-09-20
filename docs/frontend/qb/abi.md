# Measured QB procedure and array ABI

This file records facts used by the source frontend. They are not inferred
from 16-bit C conventions. The probes are compiled BASIC, and the offsets
below are visible in the emitted instructions.

## Ordinary `BYREF`

`suite/procs.bas` was compiled with VBDOS `/O /FPi /R /G3 /E /Zi`, PDS 7.1
`/O /FPi /G2 /Zi`, and QuickBASIC 4.5 `/O /FPi /Zi`. In all three `Twice&`
loads one word from `[bp+6]`, treats it as a near address in DS, then reads the
LONG at `[si]` and `[si+2]`. Each procedure returns with `retf 2`.

The HIR therefore carries an ordinary `BYREF T` parameter as a near pointer
to `T`. It does not encode SI, BP, DS, `retf`, or a split LONG. The QB ABI
adapter and existing lowering own those details.

Dereferencing that pointer is volatile. `IN_KEYSTROKE` is the measured case:
the VBDOS `/O` loop reloads `[si]` on every back-edge while the keyboard
interrupt handler owns the matching store. Reusing or hoisting the first read
hangs while waiting for release. This is the general published-pointee rule
for QB `BYREF`, not a keyboard-name special case; HIR records it on the
indirect place and lowering preserves it as an ordered memory access.

A call-site side table records the stack permutation, far-call distance, and
cleanup owner. Those facts are applied after semantic optimization by emitting
the existing MIR `ARG` form. The semantic `CALL` therefore keeps its typed
arguments long enough for interprocedural and alias analysis; physical stack
order never becomes an optimizer-visible machine property.

Ordinary BASIC SUB/FUNCTION calls use Pascal order: the first formal is pushed
first, and the callee removes the arguments. `DECLARE ... CDECL` alone reverses
the push order and assigns cleanup to the caller. The raw VBDOS
`COM_TOKENIZE argv(), argc, " ", cl` site shows descriptor, count, separator,
then command line in that order; reversing it corrupted the populated array's
far heap before the semantic call-order fix.

## OMF symbol spelling and scope

The source OMF adapter reproduces Microsoft's linker names exactly. Public
SUB/FUNCTION names are ASCII uppercase with the BASIC type suffix removed;
runtime EXTDEF names retain their table spelling, including `$` and case;
compiler data labels are `<MODULE>$D<number>`. The unmodified Gorillas pair
therefore uses `CENTER`, `B$SASS`, and `GORILLA$D61` on both sides.

Source globals retain their **effective** BASIC type suffix, not merely their
spelling in a `DIM`. A QB 4.5 `/Zi` OMF probe establishes: untyped
`DIM SHARED implicit` is `IMPLICIT!`; `AS INTEGER`, `LONG`, `SINGLE`,
`DOUBLE`, and `STRING * 8` are respectively `%`, `&`, `!`, `#`, and `$`; an
untyped numeric array is likewise `IMPLICITARRAY!`. The same probe reads the
names from `$$SYMBOLS`, where BC actually records its globals.

Scope is independently represented in HIR. A source SUB or FUNCTION is an
external definition, while `DEF FN` and compiler-outlined module GOSUB bodies
are internal definitions. Internal procedures retain callable code labels and
the same BASIC frame ABI but do not produce PUBDEF records. This matters for
interoperability: BC's Gorillas object defines `FNRAN%` in its listing but does
not export `FNRAN`; qbopt now does the same.

These rules are shared by QB 4.5, PDS 7.1, and VBDOS. Dialect selection may
change available syntax and runtime entries, never the spelling of a symbol
with the same source meaning.

## Array parameters

`suite/arrprm.bas` was compiled from the same source in all three families.
The caller passes one word: the address of the array descriptor. VBDOS and PDS
materialize that address, store DS into descriptor word `+2`, push the address,
and far-call the procedure. QuickBASIC pushes the relocated descriptor address
directly. Every `FillNums` implementation then does:

```text
mov si,[bp+6]       ; near pointer to the descriptor
... [si+0Ah]       ; adjusted data offset used by BC's constant-index shortcut
mov es,[si+2]       ; data selector
... [es:bx]         ; element access
retf 2
```

The emitted descriptor is the runtime layout already validated by
`raising_arrays.py`: a data far pointer at `+0`, rank/features at `+8/+9`, an
adjusted base at `+10`, element width at `+12`, then reversed-dimension
`(count, lower)` pairs from `+14`.

The source frontend represents the formal parameter as a near pointer to the
descriptor. Huge descriptor element access loads the whole data pointer and
uses a 32-bit byte displacement. A local fixed-bound variable-STRING array made
by `B$DDIM` instead uses the measured near form: a 16-bit data offset at
descriptor `+0Ah`, implicitly paired with DS, and 16-bit element arithmetic.
Both emit generic HIR `load`, arithmetic, `ptr_offset`, and indirect memory
operations. The HIR does not expose the descriptor layout, and MIR gains no
QB-specific operation; interpretation remains confined to QB semantics.

Dimension traversal is selected independently. The default layout is
column-major (first subscript fastest); BC `/R` is row-major (last subscript
fastest). qb-qrender is built with `/R`. This changes emitted address
arithmetic rather than the dialect or runtime profile, so HIR records it as
the program's `array_order` option.

Forwarding an array parameter passes the same descriptor pointer. Constructing
a descriptor for a source-declared fixed or dynamic array is a separate object
emission responsibility and remains an explicit lowering gate until the fresh
OMF writer can emit its data pointer relocation and runtime-family fields.

`ERASE` accepts either descriptor identity form. A local/module array supplies
an addressable descriptor place; an array formal already supplies the incoming
near descriptor pointer. `B$ERAS` remains a runtime boundary because dynamic
heap unlinking is observable work, not numeric array addressing to inline.
Automatic exit cleanup of a local variable-STRING array uses the measured
record-array entry `B$ERS1`; using generic `B$ERAS` after populated elements
produced the runtime's `Far heap corrupt` diagnostic.

## Audited file and string calls

Typed source sites refine cleanup only where runtime source and legacy
listings establish an exact stack shape:

| Entry | Typed parameters | Cleanup |
|---|---|---:|
| `B$GET3`, `B$PUT3` | channel word, record far pointer, length word | 8 |
| `B$GET4`, `B$PUT4` | channel word, record number dword, record far pointer, length word | 12 |
| `B$SSEK` | channel word, record number dword | 6 |
| `B$LEFT`, `B$STRI`, `B$STRS` | two near words | 4 |
| `B$FEVS`, `B$FEVI` | descriptor pointer or ordinal word | 2 |

These contracts retain conservative memory, error, and clobber effects;
exact cleanup does not imply purity. Floating `BYVAL` source operands are
stored explicitly to binary32/binary64 before stack materialization.
Extended precision is the evaluation format, not the procedure ABI format.

## Debug-type corroboration

The `/Zi` records independently identify VBDOS/PDS array parameters as the
ordinary BYREF wrapper and pointer chain around an array type. QuickBASIC 4.5
uses a direct pointer-to-array record instead. This difference is debug-format
spelling, not a different calling convention; the three instruction listings
above all pass the same one-word descriptor address.

## Link-only runtime dependencies

`SCREEN` has a second ABI effect beyond its `B$CSCN` call. A constant mode
references `B$CGAUSED`, `B$EGAUSED`, `B$VGAUSED`, `B$HRCUSED`, or
`B$OLIUSED`; an expression references `B$GRPUSED`. These zero-cost private
relocations pull the selected graphics module from the Microsoft runtime.
Without one, `SCREEN 9` reaches `B$CSCN` but raises error 5.

`ON ERROR` registration is executable state, not function metadata. Each
source statement materializes `B$OEGA` with a far module offset or `B$OEGP`
with a local offset. `ON ERROR GOTO 0` passes zero at that exact position.
