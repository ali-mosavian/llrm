> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/occurrence.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/occurrence.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/occurrence.rs sha256:65932a02379e3b52c890b6c81854f9deef2c978c2ebc1134ab71c3891686e079 -->

# occurrence

Walks a function's instructions as occurrences, operations and phis apart,
so an analysis can key by identity and keep two equal instructions distinct.

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

## Example
```mir
define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  %p = phi i16 [ %x, %b0 ], [ 1, %b1 ]
  %y = add i16 %p, 1
  ret i16 %y
}
```

| walk           | yields, in layout order                              |
|----------------|------------------------------------------------------|
| `phis`         | `%p`, with block `b2`                                |
| `operations`   | the two `br`s, `%y`, the `ret`                       |

From unit test
`operations_and_phis_partition_every_instruction_in_layout_order`
in `occurrence.rs`.

## Provides
The three walks. Consumers: [`crate::induction`](induction.md), [`crate::ssa`](ssa.md), and the
`unswitch` and `loopexit` passes in llrm-transforms.
