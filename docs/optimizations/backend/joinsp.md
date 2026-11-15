> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/joinsp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/joinsp.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/joinsp.rs sha256:3454cc465f6cea5056b727012f05de52174e4c5d0da064e8b9702a496d63577b -->

# joinsp

A copy of the stack pointer that nothing moves it from is the stack pointer:
`RegisterCoalescer::joinReservedPhysReg`, the case where a virtual register
is copied from a reserved register.

## How it works
`llvm.stacksave` is `v = mov sp` and `llvm.stackrestore` is `mov sp, v`.
When no instruction between the save and its restores can write the stack
pointer, `v` holds what the stack pointer holds, the restores are
`mov sp, sp` and both go. Without it the restore is a write to the stack
pointer that no frame model can follow, and the function keeps a frame
register (`has_fp`: `OpaqueSpAdjustment`) though its frame is static.

The pin's test is that the live range of `v` overlaps no def of a unit of
the reserved register (all of them dead defs: calls, `ADJCALLSTACK*`,
`sub rsp`). Here the range is the instructions between the save and the last
restore on every path, and the defs are the instructions below that can
write the stack pointer.

```text
  v = mov sp                    (gone)
  ... loops, no call ...   =>   ... loops, no call ...
  mov sp, v                     (gone)
  call report                   call report
```

DEVIATE 1: only the case the pin's `CP.isFlipped()` branch handles whose
uses are all restores; a `v` read any other way is left (the pin would
replace each read by the register, which here could place the stack
pointer where an encoding takes none). Exit: none seen.

DEVIATE 2: the other branch (`sp = mov v` where `v` is defined once in the
block, moving the def to write `sp`) is not translated: a dynamic
allocation's `sub` writes the stack pointer through the same pin pass but
nothing here depends on it.

DEVIATE 3: a block's phis and any other mention of `v` (`requires`,
`delivers`, `undefs`) end the join: the pin's `RHS.containsOneValue()` is
here a single `defines` and no reader but the restores.

## Example
`stacksave`, a loop over a static array, `stackrestore`, a call
(vla_callee_constant, x86_64 -O2): no `mov rsp, rax`, no frame register.

## Runs
Before the coalescer, after two-address conversion; not at -O0, where the
pin does not coalesce.

## See also
LLVM `RegisterCoalescer`
(https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L2314);
`crate::coalescer`, [`crate::backend::framefree`](framefree.md)
