# Measured QB procedure and array ABI

Audience: contributors to `qbfront` and the QB ABI adapter. Facts about the
Microsoft BASIC procedure, array and symbol ABI, measured from compiled BASIC
and not inferred from 16-bit C conventions. The adapter is
`crates/codegen/llrm-codegen/src/abi/qb.rs`.

A runtime result is primary evidence. A BC `/A` listing is not raw code: VBDOS
shows back-patched frame operands as zero.

## Ordinary `BYREF`

`tests/run/qb/procs.bas` was compiled with VBDOS `/O /FPi /R /G3 /E /Zi`, PDS 7.1
`/O /FPi /G2 /Zi`, and QuickBASIC 4.5 `/O /FPi /Zi`. In all three `Twice&`
loads one word from `[bp+6]`, treats it as a near address in DS, then reads the
LONG at `[si]` and `[si+2]`. Each procedure returns with `retf 2`.

The HIR therefore carries an ordinary `BYREF T` parameter as a near pointer
to `T`. It does not encode SI, BP, DS, `retf`, or a split LONG. The QB ABI
adapter and existing lowering own those details.

Dereferencing that pointer is ordinary, as for any variable, unless the
parameter is declared `AS VOLATILE T`. `IN_KEYSTROKE` is the measured case:
the VBDOS `/O` loop reloads `[si]` on every back-edge while the keyboard
interrupt handler owns the matching store, so its source says
`keyDown AS VOLATILE INTEGER`. Nothing is assumed: a BYREF the program does not
declare so is read once by a counted loop (TEXTFILL, the palette loops), and the
same holds of `DIM` and `DIM SHARED` variables. HIR records `volatile` on the
place or indirect place and lowering preserves it as an ordered memory access.

A variable passed as a `BYREF` actual goes as its own address, and a callee's write reaches it. Only a far
pointer to a field of a far dynamic array is bridged through a near slot, copied in before the call and out after it,
as BC does: where far pointers are the near ones (`m32`, one space) a four-byte pointer is the near address and no slot
is made. It was, and `Hanoi%`'s two recursive calls copied three arguments in and out each, 729,041 instructions to
`m16`'s 368,621 (`tests/qb_byref_flat.rs`).

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
compiler data labels are `<MODULE>$D<number>`. The Gorillas pair uses
`CENTER`, `B$SASS`, and `GORILLA$D61` on both sides.

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
not export `FNRAN`; llrm now does the same.

These rules are shared by QB 4.5, PDS 7.1, and VBDOS. Dialect selection may
change available syntax and runtime entries, never the spelling of a symbol
with the same source meaning.

## Array parameters

`tests/run/qb/arrprm.bas` was compiled from the same source in all three families.
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

The descriptor layout is a data far pointer at `+0`, rank and features at
`+8/+9`, an adjusted base at `+10`, element width at `+12`, then
reversed-dimension `(count, lower)` pairs from `+14`; see the
[diagram](memory-model.md#descriptor-layout).

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

Forwarding an array parameter passes the same descriptor pointer.

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
| `B$VARP` | address (the target's near integer: a dword at m32), type word | 4 (m16; whole slots at m32) |
| `B$SADD` | string descriptor pointer | 2 |
| `B$FINP` | count word, channel word (0: the keyboard) | 4 |
| `B$BLOD`, `B$BSAV` | file name descriptor, offset (word; a dword at m32), word (BLOAD: 1 if an offset was given; BSAVE: length) | 6 |
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

`B$ENRA` reserves and zeroes the frame with no separate stack-check call at
that site (VBDOS only).

`ON ERROR` registration is executable state, not function metadata. Each
source statement materializes `B$OEGA` with a far module offset or `B$OEGP`
with a local offset. `ON ERROR GOTO 0` passes zero at that exact position.

## `noundef` (note Q1)

llrm-c states `noundef` on scalar parameters and arguments as clang does
(`DetermineNoUndef`, CGCall.cpp). llrm-qb states none yet.

What guarantees a defined value: every QB variable reads as zero until
assigned (`B$ENRA` zeroes the frame, `memory-model.md`), so a `BYVAL` scalar and
the near or far pointer of a `BYREF` argument are never `undef`. A function
result not assigned is zero too.

What does not: the bytes a `BYREF` pointer reaches in a procedure that frames
itself (`zeroed_locals` off: its frame holds garbage until its own stores zero
it), an out-parameter the runtime is to fill, and padding of a `TYPE` passed
whole. Those stay unmarked, as clang leaves aggregates.
