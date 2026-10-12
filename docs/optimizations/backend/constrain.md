> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/constrain.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/constrain.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/constrain.rs sha256:f11141e6ec4f5676082f7f2b00e810606565474ea6ddebe8f3b68e830023f967 -->

# constrain

Give each value a register is required for a value of its own, live across
that instruction alone, copied in and out.

## How it works
Some instructions need a value in a fixed register (`mul` in AX:DX,
a shift count in CL, a call argument in its ABI register; `requires` and
`delivers` on the instruction, and operand classes the register file
states). If the allocator had to place the original value there, that
value would be pinned across its whole live range. Instead
`constrained` rewrites the instruction to read and write fresh values
pinned to the required register, and connects them to the original with
copies.

```text
  before                       after
                               mov  v9, v1        ; v9 pinned AX
  mul  v1 ; reads AX, writes   mul  v9            ; requires v9 in AX
           DX:AX               mov  v2, v10       ; v10 pinned DX
                               mov  v3, v11       ; v11 pinned AX
```

A required source gets a copy before the instruction; a required
destination, and each `delivers` entry, gets a copy after it. A value
read in one required register and written in another is two
requirements, each with its own copy. If the same value fills two
required slots the second slot reads a distinct copy. Inside a block a
copy already made for the same value and register root is reused by the
next instruction that requires it there, until something redefines the
value or puts anything else in the root. A requirement the value already
meets (it is pinned to that register at the right width) needs no copy.
The result is the new body and the map of fresh values to registers.

The same file holds the other splits the allocator applies to a body
before it places values:

- `distinct_roles`: a value that is both the base and the index of one
  16-bit address needs two registers at once; the index reads a copy.
- `distinct_classes`: a value read as a byte (AX..DX) and as the index of
  a frame array (SI, DI) has no register that fits all reads. The reads that
  fit the most others keep the value; each remaining read takes a copy, live
  across that instruction alone.
- `addressed`: a candidate shape the allocator tries when the first
  allocation spills. A spilled value that is a base or index in more than
  one instruction and has no other role (no plain operand, selector,
  requirement or hidden use) is copied at each addressing instruction, so
  the copy can sit in an address register while the value is spilled.
- `required`, `required_in` and `required_added`: the pin map of a
  body, of some blocks, or of only the instructions added since a previous
  body.

## Used by
Register allocation (`allocate.rs`), which calls `distinct_roles`,
`distinct_classes` and `constrained` on the body entering allocation, the
`required` family each time the body is rewritten, and `addressed` for a
candidate shape.

## See also
The phase map: `crate::backend`
