> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/floatalloc.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/floatalloc.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/floatalloc.rs sha256:d12e9e077a11a0315932078745301b3946311aebc35ba4c326ff312e8259f9e0 -->

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
