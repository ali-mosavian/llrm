> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/cgpaddr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/cgpaddr.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/cgpaddr.rs sha256:a96d4b8499e52f83b9b191fa6f30a173d07be99dce396f2bd8f963598313ce52 -->

# cgpaddr

CodeGenPrepare: comparison and address sinking, so isel sees one form:
LLVM's `sinkCmpExpression`, `sinkAndCmp0Expression` and `optimizeMemoryInst`
(address-mode sinking, below), and GVNSink's merge of equal arms.

## How it works
Four rewrites, each on one function, none of which touches an edge.

A comparison (`icmp` or `fcmp`) read by a branch, select or other user in
another block is made again at the top of that block (one copy per block);
the original goes once nothing reads it. A phi's incoming block is not a
read. isel can then keep the result in the flags instead of an `i1` held
across the loop body (`sete`, a store of the byte, `cmp`, `jne`). This runs
only where the target has a single condition register
(`!target().multiple_condition_registers()`).

An `and` that only compares of it with zero read (`x & mask == 0`), some in
other blocks than its own, is made again before each of those compares,
where the target's lowering says the pair folds
(`Lowering::is_mask_and_cmp0_folding_beneficial`) and the copy would not
stretch two values read nowhere else (`sinkAndCmp0Expression`). Numbering
made one value of the repeated tests of a bit; the compares sunk into the
blocks that branch on it left the `and` held across all of them.

A phi read only as an address (`spill::address_only`: loads, stores through
it, and GEPs of up to three levels that are themselves only addresses) whose
arms each compute the same pure operation (`gep`, `zext`, `sext`, `trunc`,
`add`, `sub`, `mul`, `shl`, `and`, `or`, `xor`, of equal type and flags, to
depth 3) over operands that reach the join (they dominate it) is that
computation made once, before the join's first non-phi. What only the phi
read, the arms' copies, is erased.

Any phi that `simplifyInstruction` reduces to a value (arms all one value,
or the phi itself; `optimizeInst`'s phi case, L8947) is that value, whatever
reads it; this is how a closing phi left by `lcssa` goes. Each phi is
visited once, and the phis that read it are queued again.

A phi whose arms differ in kind stays.

DEVIATE C1 (m32): the rest of CodeGenPrepare's work is done by isel, which
rebuilds an address expression in the block that uses it, and by the select
lowering; the width of a switch condition is `cgpswitch`'s
(`optimizeSwitchType`); this file is the compare and address-phi part.
Evidence, 2026-10-23, per function on the same IR against the pin's `llc`
with and without the pass (`docs/working/codegenprepare.md`): at m32 the
pass changes 16-17 of about 160 functions in the pin and ours is at its
level (at -Os 1287 instructions against 1271 with and 1343 without), and
the `lea` count equals the pin's with the pass (2092 against 2139, 3983
without, QCport -Os). Owner w-4, review 2026-12-01. At m16 (QCport -Os)
ours has fewer `lea` than the pin with the pass (2118 against 2264) and
holds 59% of its gain; the residual 1505 instructions are spills and
reloads, not addresses: row AM-C2, resolved.

## Example
Before:

```mir
b1:
  %k1 = sub i16 %i, -3
  %p1 = getelementptr inbounds i16, ptr @g, i16 %k1
  store i16 0, ptr %p1
  br label %b5

; ...

b5:
  %p = phi ptr [ %p1, %b1 ], [ %p3, %b3 ], [ %p4, %b4 ]
  store i16 %c, ptr %p
  ret void
```

After:

```mir
b1:
  %k1 = sub i16 %i, -3
  %p1 = getelementptr inbounds i16, ptr @g, i16 %k1
  store i16 0, ptr %p1
  br label %b5

; ...

b5:
  %0 = sub i16 %i, -3
  %1 = getelementptr inbounds i16, ptr @g, i16 %0
  store i16 %c, ptr %1
  ret void
```

The phi of the three arms' addresses is the address computed once in the
join (`b3` and `b4` are as `b1`, with their own stores); the arms' copies
stay where their own stores read them.

From unit test
`test_a_phi_of_one_address_in_every_arm_is_that_address_in_the_join` in
`cgpaddr_sink_tests.rs`.

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
LLVM `CodeGenPrepare::sinkCmpExpression`, `sinkAndCmp0Expression` and
`optimizeMemoryInst`; issue
##386 (RGBLIGHTS `a(i) = ...` under an `IF`).
