> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/splitparams.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/splitparams.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/splitparams.rs sha256:9f1956e01102c9049de8324c1decc247edef3dc2fdceccb46a3f85c62624d585 -->

# splitparams

Splits the live range of an argument register where the calls begin, so
the frame and the kept registers can wait for them. Translated from gcc's
`split_live_ranges_for_shrink_wrap` (gcc/ira.cc).

## How it works
A function that tests its arguments and returns at once (a recursion's
base case) keeps an argument across its calls, so the allocator copies it
into a callee-saved register at the entry. That copy is a use of a kept
register in the entry block, and shrink wrapping then sets the frame up
there for every path. Here the entry keeps the register the argument
arrives in, and the block that dominates every call (outside any loop)
copies it to a new value that the rest of the body reads:

```text
  entry: n = arrive(eax)          entry: n = arrive(eax)
         cmp n, 2                        cmp n, 2
         jl  done                        jl  done
  body:  ... call ... n ...       body:  m = mov n
  done:  ret                             ... call ... m ...
```

Refused where the dominator is the entry, where it lies in a loop (it climbs
to the nearest block that does not), where every path runs through it (no
path would skip the frame), or where the entry itself calls.

## Runs
On SSA, before the phis go, after every pass that moves values. Only where
`Target::splits_params_for_shrink_wrap` is true (flow drops the phase
elsewhere). The coalescer keeps the copy it makes
(`CoalescerEnv::keeps_split_params`): joined, it would be the entry's again.

## See also
[`crate::backend::shrinkwrap`](shrinkwrap.md), which places the set-up this makes possible.

## Example
`fib(n)` tests `n < 2` and returns; the block of its calls reads `m = mov n`, so the entry sets up no frame.
