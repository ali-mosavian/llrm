> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/occurrence.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/occurrence.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/occurrence.rs sha256:e6758ffbaef0698c025c1cb188844ef095dd650ed93b586799e80e294fbb3759 -->

# occurrence

The instructions of a function as occurrences: operations and phis, apart.

## How it works
An `InstId` names an instruction and outlives rewrites, so it is the key
of an occurrence. Two equal instructions have different ids. `operations`
yields every non-phi instruction in layout order with its block;
`operations_in` yields those of a set of blocks (numbered as `cfg::id`) in
block-id order, so walking a loop costs the loop, not the function; `phis`
yields the phis in layout order. Operations and phis are separate walks.

```text
 block b2:  %p = phi ...   <- phis()
            %y = add ...   <- operations()
            ret %y         <- operations()
```

## Provides
The three walks. Consumers: [`crate::induction`](induction.md), [`crate::ssa`](ssa.md), and the
`unswitch` and `loopexit` passes in llrm-transforms.
