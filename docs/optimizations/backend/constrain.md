> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/constrain.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/constrain.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/constrain.rs sha256:96b33c7e91d0549384da6ec4993012fd62a66eda1e44ff6031ef1310f32381ad -->

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

- `distinct_classes`: a value read as a byte (AX..DX) and as the index of
  a frame array (SI, DI) has no register that fits all reads. The reads that
  fit the most others keep the value; each remaining read takes a copy, live
  across that instruction alone.
- cold reads (`distinct_classes_given`, last loop): a read that leaves a
  value fewer than `MinRCSize` (4) registers, in a block less deep in loops
  than the value's deepest reference, reads a copy of its own.

  DEVIATE N1: `InstrEmitter::AddRegisterOperand`
  ([`MinRCSize`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/InstrEmitter.cpp#L40),
  [`constrainRegClass`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/InstrEmitter.cpp#L349))
  copies every such read at selection and the coalescer joins the copy back
  where it can; here only cold reads are copied, after coalescing, and the
  route is kept only where `route_cost` is lower (`assemble::machined`).
  Exit: none; m16 is the only target with address classes.
- `hoisted`: a copy into a required register that a loop makes of a value
  defined before it, the same each trip, is made once in the loop's
  preheader. The register is then the value's for the whole loop, which
  nothing else in it may use: no other instruction requires, delivers,
  clobbers or defines a value pinned to its root, calls included. A loop
  with several preheaders is left alone.

  DEVIATE C1 (no number): the pin has no such step. Its x86 states a
  segment as a prefix, never a register copy, and `MachineLICM` does not
  move a copy into a physical register. Owner laa-runtime-checks-loadelim;
  exit: a pin-shaped segment class, or `MachineLICM` learning copies to a
  physical register; review 2026-11-30.
- `required`, `required_in` and `required_added`: the pin map of a
  body, of some blocks, or of only the instructions added since a previous
  body.

## Example
A call that takes `ax` and returns `eax` (`bump`). Before, it names the
values `v6`, `v7`; after allocation it names fresh ones, `v14`, `v15`,
copied in and out. `; inserted` notes are dropped.

Before (`Coalescer`):

```lir
0x000c  v6 := mov 0x1
0x000c  call   requires v6@AX; delivers v7@EAX; clobbers EAX
0x000e  cmp v5, 0x1
```

After (`RegAlloc`):

```lir
0x000c  AX := mov 0x1
0x000c  call   requires v14@AX; delivers v15@EAX; clobbers EAX
0x000c  CX := mov AX
0x000e  cmp BX, 0x1
```

From `ISEL_DUMP=1 llrm-nib -O2 -fno-inline-functions
-fno-inline-functions-called-once -fno-ipa-cp-clone
-march=i386` of the program under `calleefacts`.

## Used by
Register allocation (`allocate.rs`), which calls `distinct_classes` and
`constrained` on the body entering allocation, and the `required` family
each time the body is rewritten.

## See also
The phase map: `crate::backend`
