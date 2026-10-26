> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/fpstack.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/fpstack.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/fpstack.rs sha256:03e0bb40d3529124c547f76c7e19200017f20d6ed0ed2aa1e17350bb6a753352 -->

# fpstack

The x87 stackifier: flat floating values in stack form, as LLVM's
`X86FloatingPoint` does it.

## How it works
A translation of
[`X86FloatingPoint.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/X86FloatingPoint.cpp)
(`FPS`). `floatassign` leaves every floating value in a flat register while
it is live; this tracks which value sits in which stack slot and emits what
the instruction needs: `fxch` to bring an operand up (`moveToTop`), `fld
st(i)` to copy one that stays (`duplicateToTop`), the popping form of the
instruction where an operand dies (`popStackAfter`), `fstp st(i)` for a
value that dies below the top (`freeStackSlotBefore`).

Liveness is the pass's own (`setKillFlags`): scanning a block backwards from
its live-out values marks an operand killed where nothing reads it after,
and a result dead where nothing does. Across a border the values live at
it are a bundle (`spillplacement::bundles`, LLVM's `EdgeBundles`): the first
exit to reach one fixes the order of its values, every other exit shuffles
to it (`shuffleStackTop`), and a block that holds more than it reads kills
the rest at its entry (`adjustLiveRegs`). No critical edge is split. Blocks
go in depth-first order, so one predecessor of each reachable block has
fixed its entry first.

DEVIATE 1: a floating value is a value number, not `FP0`..`FP6`: the stack
model keeps the numbers in `values`, top first, and a scratch register is a
fresh number. Registers become values first (`fpregs`). More than
`float_stack` live values at once is refused where LLVM's `pushReg` is
fatal.

DEVIATE 2: LLVM has no simultaneous copies at this stage (`PHIElimination`
writes them one at a time through temporaries); a group of phi copies here
is one step, as gcc's `move_for_stack_reg` takes it (`copy_group`).

DEVIATE 3: a call, a barrier or an instruction naming a stack position
needs the stack empty. `handleCall` resets `StackTop` because the register
allocator spilled everything; a value live across one is an error here, as
`floatassign` does not spill across one.

DEVIATE 5 (temporary,
[#2337](https://github.com/ali-mosavian/llrm/issues/2337)): `v op v`
where `v` stays puts the result below the copy when `v` is read next, so
no `fxch` follows.

DEVIATE 6 (temporary,
[#2338](https://github.com/ali-mosavian/llrm/issues/2338)): a bundle still
to be fixed takes the order a fixed bundle ahead needs, through blocks that
only rename.

DEVIATE 7 (temporary,
[#2339](https://github.com/ali-mosavian/llrm/issues/2339)): critical forward
edges into a block that reads fewer floating values than its bundle holds
are split first, as `floatalloc` does (`_split_for_the_stack`): the values
are popped on that edge only.

DEVIATE 4: an unreachable block with live floating values has no predecessor
to fix its entry; its live values are taken in ascending order (LLVM
asserts).

## Runs
As the body of `floatalloc` (slot `floatalloc@8`), after `floatassign`, once
per function with floating values.

## Example
`a * b` where `a` is read again afterwards. Before:

```lir
v1 := fld [a]
v2 := fld [b]
v3 := fmul v1, v2        ; v2 dies, v1 stays
```

After: `fld [a]; fld [b]; fmul st(0), st(1)` leaves the product on top and
`a` under it.

## Needs
[`crate::backend::floatassign`](floatassign.md), `live`, `spillplacement::bundles`.
