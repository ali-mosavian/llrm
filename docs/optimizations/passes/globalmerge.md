> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/globalmerge.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/globalmerge.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/globalmerge.rs sha256:a349406b46463c9de42e36a51897abd0ee9c2e06cd0fb2a19aca40f13affb5c2 -->

# globalmerge

Merges internal globals into one struct, so that code addresses them as
offsets from one base instead of one `adrp`+`add` each (LLVM's
`GlobalMerge`).

## How it works
Variables of one address space and section are candidates when they are
local, initialized, not named by anything but instructions, and smaller
than the target's offset reach. Zero-initialized ones (`.bss`) are grouped
apart from the rest. Within a group, the globals each function uses
together are found; those used with another global are merged, in order of
size, into packed structs that stay within the reach. Every use becomes a
`getelementptr inbounds` of the struct, and the originals go.

## Example
```text
  @a = internal global i32 1      @_MergedGlobals = private global <{ i32, i32 }> <{ i32 1, i32 2 }>
  @b = internal global i32 2  ->  store i32 0, ptr getelementptr inbounds (<{..}>, @_MergedGlobals, 0, 1)
  store i32 0, ptr @b
```

## Deviations
GM1 (exit 2026-11-12, owner w15): a global another global's initializer,
an alias or a landing pad names stays unmerged, and external ones are not
merged: MIR has no alias to a member of a struct, and the arm64 writer
emits no alias. GM2 (same exit): a global with metadata stays unmerged.
The pin copies `!dbg`.

## Runs
Once per module after the mid-end, for the targets that ask
(`GlobalMerge`): `AArch64TargetMachine::addPreSched2`-time IR preparation,
offset 4095, only for functions with `minsize` below `-O3`.

## See also
LLVM's `GlobalMerge`
([GlobalMerge.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/GlobalMerge.cpp)).
