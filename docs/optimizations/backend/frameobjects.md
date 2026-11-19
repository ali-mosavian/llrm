> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/frameobjects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/frameobjects.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/frameobjects.rs sha256:1f1143dc80fd31f6dac931cc017e94047f1339b84751d7f8640fcda3b2fb640e -->

# frameobjects

Where each frame object goes and how big the frame is: LLVM's
`PEIImpl::calculateFrameObjectOffsets`
([PrologEpilogInserter.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PrologEpilogInserter.cpp#L856)).

## How it works
The objects are numbered; fixed ones (arguments, the return address) have
offsets already and sit at the negative numbers of the pin, here the
first `fixed` entries. For a stack that grows down the walk is:

1. `Offset` starts at the local area (the return address) and rises to the
   far end of every fixed object (`FixedOff`).
2. The callee-saved objects, last to first, then every other live object in
   index order, each `AdjustStackOffset`: add its size, align to its
   alignment, take the negative as its offset.
3. A call frame reserved in the frame adds the largest outgoing size; the
   total aligns to the stack alignment (the transient one where the function
   neither adjusts the stack nor has a variable-sized object), raised to the
   largest object alignment.

The stack size is `Offset` less the local area.

```text
  objects (size/align):  a 4/4, b 1/1, c 8/8     local area 4
  a: 4+4 = 8  -> [-8]    b: 8+1 = 9 -> [-9]    c: 9+8 = 17 -> 24 -> [-24]
  no call: transient alignment 4, size 24 - 4 = 20
  a call with 12 outgoing bytes: 24 + 12 = 36 -> 48 (16), size 44
```

DEVIATE FO-1 (exit: frame indices, owner llrm-target-x86-m16, 2026-11-20): nothing calls
this yet. `frame::Frame` hands out a slot when the spiller asks and the
displacement goes into the instruction at once; the pin names a frame index
and `replaceFrameIndices` fills the offset after this runs.

DEVIATE FO-1 also covers `order_frame_objects`
(`X86FrameLowering::orderFrameObjects`): translated, used by
`relayout::ordered` on the finished body, not yet between objects and
`calculate_frame_object_offsets` (owner llrm-target-x86-m16, 2026-11-20).

DEVIATE FO-2 (exit: with FO-1, owner llrm-target-x86-m16): the stack
protector, the local stack allocation block, scavenging slots,
`computeFreeStackSlots` and `StackSymbolOrdering` are not translated; i486
enables none (`enableStackSlotScavenging` is false, no register scavenger,
no `-fstack-protector` frontend).

## Example
The unit test `objects_are_laid_below_the_local_area_by_size_and_alignment`
is the sketch above.

## Used by
Nothing yet (FO-1).
