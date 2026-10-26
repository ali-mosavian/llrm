> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/floatalloc.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/floatalloc.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/floatalloc.rs sha256:55d890806b1ef0db282f6a7f576361216b2259895b3fd6541a39abf62db1f7d1 -->

# floatalloc

Converts floating values held in flat x87 registers to stack form: `fld`,
`fxch`, `fstp st(i)` and `st(i)` operands.

## How it works
`floatassign` has already decided which values are live in a register and
made every spill and memory operand explicit. This pass decides nothing: it
tracks which value sits in which stack slot (`_Stack::values`, top first)
and emits what puts each operand where the instruction needs it, as gcc's
reg-stack and LLVM's `X86FloatingPoint` do.

Blocks are visited in reverse postorder. The values live across a block
border form a bundle (`spillplacement::bundles`). The first exit to reach a
bundle fixes the order of its values; every other exit shuffles to that
order with `fxch` (`_Stack::leave`), and a value the successor does not read
is popped. A critical forward edge into a block that reads fewer values than
its bundle holds gets a block of its own, so the pop happens on that edge
only (`_split_for_the_stack`). A phi's copies on one edge are simultaneous
and taken as one group, and a source that dies there is renamed, not copied.
A call, a barrier or an instruction naming an x87 register by position needs
the stack empty (`flush`); a value live across one is an error.

A two-operand operation follows gcc's `handleTwoArgFP`. An operand dies
where nothing reads it again, and the result takes its slot:

```text
 operands                          emitted
 --------------------------------  ------------------------------------
 neither on top, one dies          fxch the dying one to the top
 neither on top, both live         fld a copy of left; the copy dies
 the top is left, right lives      op st(0), st(i)
 the top is right, left lives      reversed op (fsubr, fdivr)
 both die                          popping form: faddp, fsubp, ...
 one value twice, still live       fld a copy; the result replaces it
```

Operand order is never changed by an exchange: subtraction and division take
their reversed form instead. With `[right, left]` on the stack and both read
later:

```text
 fld  st(1)           ; copy of left        [left', right, left]
 fld  st(1)           ; copy of right       [right', left', right, left]
 fsubp st(1), st(0)   ; left' - right'      [result, right, left]
```

A store or a compare that reads a value still live does not copy it first:
`fst`, `fist` (a word or a dword; `fistt` for `fisttp`) leave the value on
the stack, and `fcom`/`fcomp st(i)` compare it where it lies, as gcc's
reg-stack and LLVM's `X86FloatingPoint` do. A quadword conversion has only
its popping form and keeps the copy.

More than eight operands on the stack is refused. Order, rounding and
storage conversions are not changed.

Afterwards `fisttp` is made as a 387 has it: `fistp` between `fldcw` of a
control word with rounding set toward zero and a `fldcw` of the saved one.
The two words are saved once at entry.

Where the switches go is gcc's mode switching, in one analysis and one
placement over the graph (`_needs`, `_modes`;
[`optimize_mode_switching`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/mode-switching.cc#L495),
the i386 entity
[`I387_TRUNC`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/config/i386/i386.cc#L14607)).
Each instruction needs a mode: truncating for a conversion, the caller's for
whatever rounds, a call or a return, none for exact work. A block assumes on
entry the mode its first need names, or its successors' (the heavier by loop
depth), and a switch goes on each edge whose ends disagree, so a loop of
conversions switches on its entry edge and its exits, not twice a trip
(`fldcw` is 4 clocks on a 486, 7 on a P5), and a run of blocks switches
once. Unlike gcc's lazy code motion this does not solve for the least number
of insertions: it follows demand and takes a critical edge's source mode.

## Example
The same `keep` as in `floatassign`. Each value becomes `ST0`, the stack
position implied by the order of the instructions. `a` is read again after
the multiply, so the multiply works on a copy pushed first (`fld ST0`).

Before (after `FloatAssign`):

```lir
keep: 14 instructions
  block 0x0000
    0x0000  v1 := fld [[bp+0x8]]  ; inserted
    0x0002  v3 := fmul v1, [[bp+0x10]]  ; inserted
    0x0003  [[bp-0x10]] := fstp v3  ; inserted
    ; ...
    0x0007  [[bp-0x18]] := fstp v1  ; inserted
    0x0007  call   ; inserted; delivers v6@EAX; clobbers EAX,ST0
    0x0009  v1 := fld [[bp-0x18]]  ; inserted
    0x0009  v8 := fadd v1, [[bp-0x10]]  ; inserted
```

After (`FloatAlloc`):

```lir
keep: 14 instructions
  block 0x0000
    0x0000  ST0 := fld [[bp+0x8]]  ; inserted
    0x0002  ST0 := fld ST0  ; inserted
    0x0002  ST0 := fmul ST0, [[bp+0x10]]  ; inserted
    0x0003  [[bp-0x10]] := fstp ST0  ; inserted
    ; ...
    0x0007  [[bp-0x18]] := fstp ST0  ; inserted
    0x0007  call   ; inserted; delivers v6@EAX; clobbers EAX,ST0
    0x0009  ST0 := fld [[bp-0x18]]  ; inserted
    0x0009  ST0 := fadd ST0, [[bp-0x10]]  ; inserted
```

From `ISEL_DUMP=1` of the nib function `keep(a: f64, b: f64)` with `-m32
-O0`.

## Runs
After `floatassign` in the machine phases, once per function, on bodies with
floating values only (the truncation fix-up runs where there is a `fisttp`).
Not gated by an -O level.

## Needs
[`crate::backend::floatassign`](floatassign.md), `live` and `spillplacement::bundles`;
[`crate::backend::floatregions`](floatregions.md) for the instruction boundaries.

## See also
`docs/optimizations/floating-pop-reuse.md`; gcc `reg-stack.cc` and LLVM
`X86FloatingPoint.cpp`.
