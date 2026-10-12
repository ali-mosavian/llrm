> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/valuetrack.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/valuetrack.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/valuetrack.rs sha256:6805c80b2c89c9e4e9841b7a39f53de17a2c0ea34a8fad17baf014fac806580e -->

# valuetrack

Where each `-g` variable's value is over the emitted code.

## How it works
Instruction selection says which instructions define a value a variable
names, and masm marks where each landed: `Mark::Def` (the register or frame
cell the instruction before the mark wrote) and `Mark::Note` (a variable
takes a value). Nothing else of the allocator is read.

The emitted bytes are decoded and followed from the entry along every path,
with what each register and frame cell holds. A move of a whole register or
cell copies what it holds, any other write loses it, a call loses what the
callee clobbers, and a push loses the cells it covers. Where paths join,
only what all of them agree on is kept. If the function lets a frame cell's
address out, a call or a write through a pointer may write any cell, so
those lose every cell.

```text
 code:   mov [bp-4],ax        ; Def: value 7 is in [bp-4]
         mov bx,ax            ; bx holds 7 too
         call f               ; bx lost; [bp-4] kept unless it escaped
 note:   x takes value 7      ; x is in [bp-4]
```

A variable is where its value is: a register if one holds it, else a cell,
or a constant. No value is the answer wherever the code cannot be followed
or the value is in no register or cell. An optimizer that wrote the value
somewhere unrecorded costs the variable its location there, never a wrong
one.

## Runs
On the emitted code of a procedure, from `debuginfo`, when `-g` places
variables. A variable any of whose notes was not emitted gets no location.

## See also
LLVM's instruction-referencing
[`LiveDebugValues`](https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/LiveDebugValues/InstrRefBasedImpl.cpp).
