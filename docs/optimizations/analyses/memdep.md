> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/memdep.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/memdep.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/memdep.rs sha256:66a30d8e3adbe495f165118b62db1e95d81cc235de74e325e66490808fe2a100 -->

# memdep

`MemoryDependenceAnalysis`: for a load, a store or a call, the nearest
instruction it depends on in its block (a def or a clobber), or that there
is none there and the question moves to the predecessors.

A translation of LLVM's
[`MemoryDependenceAnalysis.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemoryDependenceAnalysis.cpp)
and `.h` at d1106deb71cc, over the interface of [`crate::aa`](aa.md). Each function
names the C++ function it translates and keeps its order of checks; caps
stand at their `cl::opt` defaults.

## How it works
`get_dependency` scans a block backwards from the query, asking the alias
analysis about each instruction, and caches the answer with a reverse map
(`reverse_local_deps`) so `remove_instruction` can dirty exactly the answers
that named the removed instruction. A dirty answer keeps the instruction
after the removed one, so a rescan starts there.
`get_non_local_call_dependency` does the same per predecessor block for a
call.

## Example
```text
b0: store i16 1, ptr %p     ; <- Def of the load below (must alias)
    call void @g()          ;    writes memory it cannot name: Clobber of anything
    %x = load i16, ptr %p   ;    get_dependency(%x) = Clobber(call)
```

## Provides
`MemDepResult`, `MemoryDependenceResults` (`get_dependency`,
`get_non_local_call_dependency`, `get_non_local_pointer_dependency`,
`invalidate_cached_pointer_info`, `get_pointer_dependency_from`,
`get_simple_pointer_dependency_from`, `remove_instruction`,
`invalidate_cached_predecessors`, `verify_removed`), `get_location` and
`is_store_preserving_memory_location` (`Loads.cpp`, shared with GVN).

## See also
docs/architecture/gvn-port.md (stage 2).

DEVIATE, permanent (MIR lacks the construct):
1. Atomics, fences, `va_arg`, masked loads and stores: MIR has none, so the
   ordering and fence arms of `GetLocation` and of the scan are absent. A
   volatile access has no location (it is `isUnordered` false).
2. `getInvariantGroupPointerDependency` (and with it `NonLocalDefsCache`,
   only it fills): MIR has no `invariant.group`. A query on an instruction
   that carries one panics, `test_invariant_group_metadata_is_refused`.
3. `getFreedOperand`: there is no `TargetLibraryInfo`, so a call to `free`
   is a call with no location (it clobbers by its attributes).

DEVIATE, stated facts:
4. One `BatchAAResults` per scan, not one per `AAResults` call: it only
   memoises `alias` for one body, which does not change during a scan.
5. `EarliestEscapeAnalysis` belongs to the alias analysis (the batch takes
   no `EEA`), so `remove_instruction` forwards nothing to it; an alias
   analysis is made for one version of a body (`aa.rs`).
6. `predecessors` names each predecessor once; C++ names a block once per
   edge. The repeats only revisit a visited block.
7. A call is identical when its `tail` is a tail call in both, not the same
   kind: `isIdenticalToWhenDefined` compares `isTailCall`.

OWED: the non-local pointer queries (`getNonLocalPointerDependency` and its
helpers, the pointer caches in `removeInstruction`,
`invalidateCachedPointerInfo`): phi translation interns constants, which
needs `&mut Context` while the alias analysis holds `&Context` (#2500).
