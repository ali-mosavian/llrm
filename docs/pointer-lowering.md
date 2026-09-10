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

Whole-pointer integer loads and stores now survive SSA substitution and
lower to a local pointer materialization. For a word load, one possible
allocation is:

```text
MIR:     value = load(pointer)
backend: push es
         push eax       ; whole pointer
         pop bx         ; offset
         pop es         ; selector
         mov cx, es:[bx]
         pop es         ; restore the surrounding address-space resource
```

The allocator chooses the general registers; MIR names none. The expansion
preserves flags and balances the stack. Focused encoding tests check both
load and store bytes. Distinct pointer values are not treated as proof of
disjoint memory. Other pointer memory operations are explicitly unsupported.

This is a backend foundation, not a claim that huge-array programs already
use it. The frontend must accumulate a full-width byte displacement, raise
the descriptor pointer as a whole value, and use PTR_OFFSET. The production
pipeline must establish and supply the runtime
pointer ABI. Actual emitted-code and DOS cross-64K regression cases are then
required before removing the existing huge-array refusal.
