> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/addresssink.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/addresssink.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/addresssink.rs sha256:12fa411cfc04875ec8482116064850aab132921c2f6c297490a1f6bf17044233 -->

# addresssink

Comparison and address sinking, so isel sees one form: LLVM's CodeGenPrepare
`sinkCmpExpression` and address sinking, and GVNSink's merge of equal arms.

## How it works
Three rewrites, each on one function, none of which touches an edge.

A comparison (`icmp` or `fcmp`) read by a branch, select or other user in
another block is made again at the top of that block (one copy per block);
the original goes once nothing reads it. A phi's incoming block is not a
read. isel can then keep the result in the flags instead of an `i1` held
across the loop body (`sete`, a store of the byte, `cmp`, `jne`). This runs
only where the target has a single condition register
(`!target().multiple_condition_registers()`).

A phi read only as an address (`spill::address_only`: loads, stores through
it, and GEPs of up to three levels that are themselves only addresses) whose
arms each compute the same pure operation (`gep`, `zext`, `sext`, `trunc`,
`add`, `sub`, `mul`, `shl`, `and`, `or`, `xor`, of equal type and flags, to
depth 3) over operands that reach the join (they dominate it) is that
computation made once, before the join's first non-phi. What only the phi
read, the arms' copies, is erased.

```text
 b1: %p1 = gep @g, (%i - 3)        b1: br b5
 b3: %p3 = gep @g, (%i - 3)   ==>  b3: br b5
 b4: %p4 = gep @g, (%i - 3)        b4: br b5
 b5: %p = phi [%p1][%p3][%p4]      b5: %k = sub %i, -3
     store %c, %p                      %p = gep @g, %k
                                       store %c, %p
```

A phi of this kind whose arms are all one value (or the phi itself) is that
value; this is how a closing phi left by `lcssa` goes. Each phi is visited
once, and the phis that read it are queued again.

A phi read as a value, or whose arms differ in kind, stays.

## Runs
Once, last of the function passes before `spares` and `homes`, after
`gepoffset`: until then gvn's partial redundancy elimination makes exactly
this phi so a load after the join reads what the arms stored, and an earlier
sink would be undone. Off with `-fno-strength-reduce` and at -O0.

## Keeps
Blocks and edges are unchanged, so dominators and loops are preserved. A
change that only replaced trivial phis also keeps the assumption cache and
the exposed frame objects; value facts are recomputed.

## See also
LLVM `CodeGenPrepare::sinkCmpExpression` and `optimizeMemoryInst`; issue
##386 (RGBLIGHTS `a(i) = ...` under an `IF`).
