> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/phielim.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/phielim.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/phielim.rs sha256:e15d89397bea5856f2fdb0c1bf305207d03f5e7fa36bc4da342203ab38f61aea -->

# phielim

Takes LIR out of SSA: each phi becomes parallel copies on its incoming
edges.

## How it works
For each phi `r = phi [p1: v1, p2: v2]` the pass puts `r := v1` at the end
of `p1` and `r := v2` at the end of `p2`, before the terminator. The copies
of one edge carry one group number, so [`crate::backend::parcopy`](parcopy.md) later
orders them as a single simultaneous assignment. A phi with an incoming edge
from outside the body is left alone.

A copy on a branch's edge instead of in its predecessor is made only when
it costs the target at least a tenth less (`EdgePrices::cheaper_on_edge`:
the edge's chance times its copies and the `jmp`, against the copies every
time), and never in a build for size: the odds are a guess, and an edge
that wins by less costs a block and a jump the clocks do not show.

Before copying, three shortcuts avoid a copy altogether. A one-input phi
renames its result to its input. A loop-exit one-input phi is the exception:
it becomes a copy opening the exit, so a call after the loop does not pin
the loop's register to a call-preserved one. On a critical edge, where every
incoming value is defined once in its own predecessor, is read by nothing
but this phi (or only past the loop the edge closes), and the phi's result
is not live after that definition, the definitions are renamed to define the
result outright. That is what gcc's out-of-SSA does with a phi argument that
does not interfere with the result.

Otherwise a predecessor with several successors makes the edge critical. A
copy there may run in the predecessor only if no other path reads either end
of it (`_observed`: whether a block that reads it is reachable from the
predecessor's other successors without entering the successor, searched
from both ends so that the smaller region is walked) and, where block odds
are known, the edge is not cheaper
(`EdgePrices::cheaper_on_edge`: the edge's chance times copies plus a `jmp`
against the copies run every time). Otherwise the copy goes in a new block
on the edge, which holds its copies and a jump to the successor:

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  p["<b>P</b>"]:::mid
  q["<b>Q</b>"]:::mid
  s["<b>S</b><br/><small>r = phi[P: v]</small>"]:::mid
  p --> q
  p --> s
```

*Before: edge P to S is critical (r = phi[P: v])*

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  p["<b>P</b>"]:::mid
  q["<b>Q</b>"]:::mid
  e["<b>E</b><br/><small>r := v #59; jmp S</small>"]:::src
  s["<b>S</b>"]:::mid
  p --> q
  p --> e
  e --> s
```

*After: the copy goes in a new block E on the edge*

The new block's label is above every physical offset (`(entry + 1) << 32`),
in a namespace of its own per body entry.

`LLRM_PHIELIM_LLVM=1` lowers the phis that are not x87 by the translation of
LLVM's `PHIElimination` ([`crate::phielimination`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/phielimination.rs)) first, and this pass the
rest. Off by default: it leaves a cycle of loop-carried values to be broken
by copies through a register, where the parallel copy here breaks it with an
`xchg`, and the i64 Fibonacci loop
(`test_an_i64_is_a_dword_pair_through_phis_calls_and_returns`)
spills a dword with it.

`unsplit` undoes a split whose copies coalescing removed: a block of only a
jump is bypassed, so the jump is not taken on every pass. The peephole calls
it first.

## Example
A loop carrying three values has three phis at its head:

Before:

```lir
block 0x0000
  0x0000  v2 := mov 0x1  ; inserted
  0x0000  v3 := mov 0x0  ; inserted
  0x0000  jmp   ; inserted
block 0x0001
  v5 := phi 0x0000:v2 0x0009:v4
  v6 := phi 0x0000:v3 0x0009:v5
  v8 := phi 0x0000:v1 0x0009:v7
  ; ...
block 0x0009
  0x0009  v4 := add v6, v5  ; inserted
  0x000a  jmp   ; inserted
```

After:

```lir
block 0x0000
  0x0000  v2 := mov 0x1  ; inserted
  0x0000  v3 := mov 0x0  ; inserted
  0x0000  v5 := mov v2  ; inserted; parallel-copy 1
  0x0000  v6 := mov v3  ; inserted; parallel-copy 1
  0x0000  v8 := mov v1  ; inserted; parallel-copy 1
  0x0000  jmp   ; inserted
block 0x0001
  ; ...
block 0x0009
  0x0009  v4 := add v6, v5  ; inserted
  0x000a  v5 := mov v4  ; inserted; parallel-copy 2
  0x000a  v6 := mov v5  ; inserted; parallel-copy 2
  0x000a  v8 := mov v7  ; inserted; parallel-copy 2
  0x000a  jmp   ; inserted
```

Each phi became one copy at the end of each predecessor, one group per edge.
The copies of group 2 read values (`v5`) that others write, which
[`crate::backend::parcopy`](parcopy.md) orders after allocation.

From `ISEL_DUMP=1` of a C Fibonacci loop (`t = a + b; a = b; b = t;`)
compiled with `llrm-cc -O2 -m16`: the stage before `PhiElimination`, and
`PhiElimination`.

## Runs
A machine phase after `ssaspill` (spiller route only) and before
`machinesched`, in every machine (`flow::machine_with`); `EdgePrices` come
from the CPU profile, and with none a copy stays in its predecessor. Also
`placed_on_edges`, which [`crate::backend::floatalloc`](floatalloc.md) uses to put its
selected transfers on exact edges.

## Keeps
Result is out of SSA except for phis it refused. Value identities are
unified where a phi was trivial, so the body's widths and operands are
renamed throughout.

## See also
- LLVM `PHIElimination`:
  <https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/PHIElimination.cpp>
