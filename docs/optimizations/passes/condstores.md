> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/condstores.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/condstores.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/condstores.rs sha256:0386937bb4ba877f472df85ff7e9625b5ace28a2797dcd5bbb849a46be2b638c -->

# condstores

Two conditional stores to one address, one in each of two if-then-else
diamonds or triangles in a row, are one store after them, predicated on
either condition: SimplifyCFG's `mergeConditionalStores`.

## How it works
`merged` takes the branches `pbi` and `qbi` of two blocks where every way
out of `pbi` leads to `qbi` and every way out of `qbi` to one block `post`,
through blocks of their own (or directly: a triangle). For each address
stored to in `pbi`'s arms and in `qbi`'s (`merged_to_address`), where each
side has that one store and nothing else touches memory on the way, and
the arms are cheap enough to be if-converted afterwards, both stores go,
and one store runs in a new block entered from `post`'s start when either
store would have run. The value stored is a phi in `post` of the two.

## Example
Two stores of `%g` become one after both conditions:

```text
  p: br %a, pt, pf        pt: store 1, %g     q: br %b, qt, qf      post:
  qt: store 2, %g   ==>   post: %v = phi [1, ..], [2, ..]
                                %c = or %a, %b ; br %c, then, rest
                          then: store %v, %g
```

## LLVM structure
| LLVM | here | |
|---|---|---|
| [`mergeConditionalStores`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4604) | `merged` | MIRROR |
| [`mergeConditionalStoreToAddress`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4426) | `merged_to_address` | MIRROR; D1, D2 |
| [`ensureValueAvailableInSuccessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4366) | `value_available_in_successor` | MIRROR |
| [`findUniqueStoreInBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4351) | `unique_store` | MIRROR |
| `simplifycfg-merge-cond-stores` (true), `-merge-cond-stores-aggressively` (false) | always on, `AGGRESSIVE` | MIRROR |

- D1. The MIR has no atomic stores, branch weights or `dbg.assign`: a
  volatile store is not merged, the weights of the new branch are not set.
- D2. The store's alignment is the smaller of the two, as in LLVM; the new
  block has no name.

## Used by
Named by `condtocond`, `simplifycfg`.
