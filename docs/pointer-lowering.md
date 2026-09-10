# Whole-pointer offset lowering

MIR's `PTR_OFFSET(pointer, byte_displacement)` produces one whole pointer.
It does not say how selectors, registers or segment boundaries work.
`backend/pointers.py` owns that representation and requires an explicit ABI
model. CPU arithmetic tuning does not establish an operating-system ABI.

The runtime source `runtime/rt/gwini.asm` documents `b$HugeShift`: each 64K
crossing advances the selector by `1 << b$HugeShift`. `nhinit.asm` initializes
12 for DOS. The backend supports a supplied shift, but the production
pipeline does **not yet select one automatically**. Calling pointer lowering
without an established model fails explicitly.

```text
MIR:     result = ptr_offset(base, bytes)
backend: total = low16(base) + bytes
         selector = high16(base) + ((total >> 16) << ABI.huge_shift)
         result = pack16(selector, low16(total))
```

Both temporary arithmetic and output packing wrap at the target widths.
On the DOS model, `2000:fffe + 2` becomes `3000:0000`, not `2001:0000`
(ordinary integer addition) and not `2000:0000` (discarding carry).
`2000:0004 - 8` becomes `1000:fffc`. A different established ABI can advance
selectors by a different amount; no pass above lowering changes.

The lowering emits operations over abstract variables; register allocation
still chooses every register. An unrelated live condition prevents inserting
its flag-clobbering expansion. Focused tests exercise arithmetic carry/borrow,
multi-page offsets, selector wrapping, alternate ABI, missing ABI and the
live-condition guard. Replacing the implementation with ordinary packed
integer addition makes four of the five DOS arithmetic cases fail.

## Remaining integration

This is a backend foundation, not a claim that huge-array programs already
use it. The frontend must accumulate a full-width byte displacement, raise
the descriptor pointer as a whole value, and use PTR_OFFSET. Memory operands
must consume that whole pointer without exposing encoding-specific splits to
MIR passes. The production pipeline must establish and supply the runtime
pointer ABI. Actual emitted-code and DOS cross-64K regression cases are then
required before removing the existing huge-array refusal.
