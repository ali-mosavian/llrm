> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/slotstores.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/slotstores.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/slotstores.rs sha256:d9e2b51073b259748f7fe0f38732651ca8a1debc9c6cac2cf91111d22e9c0333 -->

# slotstores

The stores that reach a load of a frame slot nothing else can write.

## How it works
A frame object whose address is not exposed (`frameescape`) is accessed by
loads and stores named by its own address alone, so no call and no store
through another pointer can change it. One whose address is exposed can
also be changed by a call that may write and by a store through a pointer
of unknown root; a store through another object's address cannot. The
state of such a slot is the state of its cells, which a store sets and an
overlapping store or one of those writes kills: the memory SSA of a single
object, with no pointer fact needed to build it. For each load of a cell it
names the stores whose value the load may read: every store on the paths
that reach it, where every path has stored the cell exactly and none since
wrote another part of it. That is the exact content of a spill slot, which
the pointer solve takes as a copy of what was stored instead of everything
stored into the object anywhere.

Each cell a pointer load names is one variable, put in SSA form as
`PromoteMemToReg` does (phis at the iterated dominance frontier of its
stores, renamed down the dominator tree): its definitions are the stores of
exactly that cell, and a store of another, overlapping part of the slot, a
store at an index the body does not know and the entry are definitions of
nothing. A load's stores are those the phis it reads join, and it has none
when any path reaches it with nothing stored.

## Example
A pointer spilled and reloaded, with a second store on one arm:

```mir
define void @f(i1 %c, ptr %p, ptr %q) {
b0:
  %slot = alloca ptr
  store ptr %p, ptr %slot
  br i1 %c, label %b1, label %b2
b1:
  store ptr %q, ptr %slot
  br label %b2
b2:
  %r = load ptr, ptr %slot
  ret void
}
```

`%r` is read from either store: `%p` or `%q`.

## Provides
`reaching`: for each load asked, its stores, or none where its source is
not exact. Consumer: [`crate::alias`](alias.md) (`point_values`).

## Needs
[`crate::basicaa`](basicaa.md) (`object_cell`), [`crate::memory`](memory.md) (`Unit::exposed`),
[`crate::cfg`](cfg.md).

## See also
LLVM `PromoteMemToReg`
(`llvm/lib/Transforms/Utils/PromoteMemoryToRegister.cpp`)
and gcc `execute_update_addresses_taken` for the same slots.
