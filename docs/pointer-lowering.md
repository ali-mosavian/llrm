# Whole-pointer offset lowering

MIR's `PTR_OFFSET(pointer, byte_displacement)` produces one whole pointer.
It does not say how selectors, registers or segment boundaries work.
`backend/pointers.py` owns that representation and requires an explicit ABI
model. CPU arithmetic tuning does not establish an operating-system ABI.

The runtime source `runtime/rt/gwini.asm` documents `b$HugeShift`: each 64K
crossing advances the selector by `1 << b$HugeShift`. `nhinit.asm` initializes
12 for DOS. All three shipped libraries export `b$HugeShift`; the production
pipeline adds that external dependency when a body contains PTR_OFFSET and
loads its initialized byte below MIR. It does not infer the ABI from CPU
tuning. Direct backend callers can supply an established constant shift;
calling pointer lowering without a model fails explicitly.

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

## Whole-pointer memory

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

Numeric HUGE accesses with a proved local integer memory consumer now
use this path. The frontend sign-extends indices and lower bounds, zero-extends
counts, accumulates the byte displacement at width 4, loads the descriptor's
whole pointer and emits PTR_OFFSET. CSE can number that pure computation.
No selector extraction is added to MIR. The frontend also proves that the
helper's old selector result is dead before removing it. Unproved consumers,
string indirection, floating accesses and other layouts still refuse explicitly.

`fixtures/regressions/huge2.bas` is compiled with `/AH` on QB 4.5, PDS 7.1
and VBDOS. Its 201-by-201 INTEGER array includes two transposed pairs of
accesses: `(4,161)/(5,161)` cross byte 65536 under QB/PDS, while
`(163,2)/(163,3)` do so under VBDOS's reversed dimension order.
Original and native programs print `123 456 789 111 222`, then `DONE`, on
all three runtimes. All ten HARY calls disappear from the native objects.

This probe caught a real emission defect: new descriptor loads encoded
zero displacements without relocation records. VBDOS printed `123 789 789`
on the first three-access version; the matching QB/PDS outputs were accidental.
The assembler now carries newly introduced symbolic memory references through
layout, and object writing creates their DGROUP-framed offset relocations.
The regression checks the emitted descriptor fields and runtime external,
and fails when those records are removed. Byte temporaries also carry their
register class, preventing allocation to nonexistent byte halves of SI/DI.

Remaining work includes generalized consumers and loop optimization, plus
loop-entry bounds guards when checking is requested. Checked mode still uses
the runtime helper; no bounds checks are silently retained in native output.

`hugelp.bas` adds a loop over both boundary-crossing pairs. Its six helper
sites (two in the loop and four reads after it) all become native MIR on
QB, PDS and VBDOS. Each original/native pair prints `456 457 789 790` and
`DONE`. Recognition permits stack-independent index calculations between
argument pushes and scalar preparation before the memory consumer. A CFG
walk checks every continuation, including loop exits, for an observation
of the old selector before an established overwrite. Unknown paths refuse.
Dead word-merge dependencies are removed only after checking transitive
high-half uses; this prevents an unused offset from surviving in a loop phi.

The production non-debug HARR case already has induction variables: the
inner loop stores the current sum, accumulates it, increments that sum and
advances the pointer by 42 bytes. The huge-loop path still needs comparable
address induction; helper removal alone is not that performance result.

HUGELP now proves its finite loop accesses stay within the allocation before
claiming that element stores cannot overwrite descriptor memory. Unknown
branches, out-of-range accesses, descriptor writes and proof-budget exhaustion
discard the proof. An unknown call ends it; only references that no future
path can revisit retain the established result. This is an exact bounded
proof, not general symbolic range analysis or a runtime bounds check.

Before, both loop accesses loaded descriptor metadata and the base pointer.
After, dimension metadata folds to constants and the whole base pointer is
loaded once before the loop; no descriptor loads remain inside it. The
32-bit address calculations and PTR_OFFSET operations still remain per access.
Original/native HUGELP outputs remain `456 457 789 790`, then `DONE`, on
QB, PDS and VBDOS. Tests also preserve the existing HARR dimension proof.

Folding exposed an encoding refusal: `sub reg, 0xfffffffe` was rejected
instead of encoded as subtraction of -2. Immediate selection now normalizes
the value to its operand width for both register and memory arithmetic.
Regression cases compare the emitted bytes of signed and unsigned spellings.
