> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/calleefacts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/calleefacts.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/calleefacts.rs sha256:c897e3b3efaf3c4e09a283acd46fda3fd8bdda37d1d3f9c5b54405c46e2b7301 -->

# calleefacts

Records which registers a private function writes, so its direct callers
keep values in the others across the call.

## How it works
A function nothing outside the module reaches (`callgraph::direct_only`),
and in no cycle of calls, takes part. It keeps its convention, but each
direct call of it clobbers only what its body wrote and did not save, not
the whole convention. A caller may therefore keep a value in any other
register across the call, scratch ones included.

The fact is made once per function, from its final LIR: `written_by`
collects the roots of every register the body writes (destinations,
registers it pins values in or takes results from, what its calls
disturb). `record` then takes away the frame's own registers and the ones
the prologue saves and the epilogue restores. A register saved by its low
half alone (a 16-bit push of a 32-bit one) and written loses its upper
half to the caller (`Written::high`); so does a register whose upper half
a callee in the body loses, unless this function saves it whole.

```text
 machined first                    machined after
   leaf  writes ax, dx  -record->  main: call leaf clobbers ax, dx
                                         cx, si, di stay live across it
```

`CalleeFacts::of` returns the facts and the order to machine the
functions in, callees before callers, so a caller finds its callee's fact
when it is selected (`isel.rs` reads `written`). A function that is
exported, has its address taken, is an interrupt handler or is in a cycle
takes no part and keeps the full convention.

## Runs
Per function in `assemble.rs`: `record` after the function is machined.
With `-fallocation-search-all` (-Omax) a caller of a known callee is also
machined without the facts and the cheaper kept.

## See also
LLVM's `RegUsageInfoCollector` and GCC's `-fipa-ra` do this one function
at a time; this ordering comes from the call graph.
