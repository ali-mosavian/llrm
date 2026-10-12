> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/mirror.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/mirror.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/mirror.rs sha256:8c9af31db7095031bbe5f8c7cbd75859983dbdf48e4b6afd7657273193ec1751 -->

# mirror

A pointer that follows a counter which only some trips step: the address
`base + k * i` of a target whose addresses take no scale `k`.

`i` is stepped on some paths of the loop and joined (`if (a[j] < p) { ...;
++i; }`), so it is no counter for LSR, which reduces `i * k` only for the
unconditional ones: every `a[i]` is then `lea bx,[i+i]` and a base
register, where the other accesses of the loop want that register too
(quicksort's partition at -m16 shuffles bx between its two accesses). The
value `base + k * i` moves with `i`: a pointer phi made of the same phis,
with the step `k * c` where `i` steps by `c`. The accesses read the pointer.
Its geps are not `inbounds`: the pointer moves with every step of `i`, one
of which may leave the object where no access follows.

The analysis is `induction::step_cycle`; this makes the pointers, as a step
of `lsr` (it runs per loop before the counters are chosen). Where the target
takes the scale in its addresses (`AddressForm::scales`)
there is nothing to save and the loop is left. The counter itself stays, for
whatever else reads it; a loop that reads it nowhere loses it to `Dead`.
