> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/aa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/aa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/aa.rs sha256:385f62bc690c7509d45c5a04155e4bed1f09499eaf08f416ba6e0d952f115c2e -->

# aa

The interface of alias analysis, as LLVM's `AAResults` and `BatchAAResults`
(llvm/Analysis/AliasAnalysis.h at d1106deb71cc): the queries a client asks
(MemorySSA, MemDep, LICM, GVN, DSE), not an implementation.

## How it works
Names are LLVM's in snake case; the primary queries are `alias`,
`get_mod_ref_info_mask`, `get_memory_effects`, `get_arg_mod_ref_info` and
the `get_mod_ref_info*` forms; every other query is a provided method over
those, as in LLVM.

Nothing here keeps a fact about the whole function: a query is answered
from the uses and definitions it reaches, and what one query learns is kept
in the `AAQueryInfo` of its batch and dropped with it, never shared between
two versions of a body.

## Example
A client asks whether a load and a store may touch the same bytes:

```text
let mut batch = AAQueryInfo::default();
let load = MemoryLocation::get(&unit, load_inst)?;
let store = MemoryLocation::get(&unit, store_inst)?;
if aa.alias(&load, &store, &mut batch, None) == AliasResult::NoAlias { /* independent */ }
```

## Provides
`LocationSize`, `AAInfo`, `MemoryLocation`, `AliasResult`, `ModRefInfo`,
`IRMemLocation`, `MemoryEffects`, `AAQueryInfo`, the `AAResults` trait and
`BatchAAResults`. Implemented by [`crate::basicaa`](basicaa.md) (S1); no client reads
it yet.

## See also
LLVM `AliasAnalysis.h`, `MemoryLocation.h`;
docs/architecture/basicaa-port.md.
