> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/liverangeshrink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/liverangeshrink.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/liverangeshrink.rs sha256:4abea52267a136a3b34704e0f42b52edad148bad7753dcdf6125c0886cede9b8 -->

# liverangeshrink

Moves an instruction up to just after the last definition of its operands,
so the operands' live ranges end sooner. Translated from
[LiveRangeShrink.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveRangeShrink.cpp)
(`LiveRangeShrink`, x86 `addPreRegAlloc`, before PHI elimination, on SSA).

## How it works
Each block is walked once from its first instruction, with an order number
for every instruction between two code motion barriers (`BuildInstOrderMap`;
a barrier is `Barrier` or `Escape`, here the pin's unmodeled side effect).
An instruction that is safe to move (`safe_to_move`), defines one value
and reads no register by name is a candidate. Each value it reads must be
read once in the function and defined once, in the class of its own value;
a value read that is the result of a copy does not count. The instruction
goes to just after the latest definition among them that is in the block
(`FindDominatedInstruction`), if more than one value read is counted: the
values it reads then end where it is, and the one it defines starts later.

```text
 before                      after
 a = load [p]                a = load [p]
 b = load [q]                b = load [q]
 x = ...                     s = add a, b    (a and b end here)
 y = ...                     x = ...
 s = add a, b                y = ...
```

MIRROR: the order map and its rebuild after a barrier, the single-use,
single-def and same-class test of each operand, the count of more than one
shortened range, the copy exemption, the landing after the latest
definition, and `isSafeToMove`'s store rule (a load stays below the stores
seen before it in its barrier-free stretch).

DEVIATE LRS1: the pin asks `MRI.getRegClass`; here a value's class is the
set that [`crate::backend::regclass`](regclass.md) confines it to and its width, and the
x87 stack. An instruction that writes the flags moves only where no
instruction between the landing and its old place reads them before one
writes them (the pin: `EFLAGS` is a physical register, so an instruction
with a live flags def does not move), and one that reads them does not
move. Exit: none; the register and flag model of the LIR.

## Example
```text
b1: v1 = load [p]      b1: v1 = load [p]
    v2 = load [q]          v2 = load [q]
    v3 = mov 7             v4 = add v1, v2
    v4 = add v1, v2        v3 = mov 7
```

## Runs
A machine phase after `machinesink` and before `phielim`
(`flow::machine_with`), in every optimized machine.

## Needs
Classes from [`crate::backend::regclass`](regclass.md), effects from
[`crate::backend::liveness`](liveness.md).
