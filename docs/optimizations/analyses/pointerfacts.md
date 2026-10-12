> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/pointerfacts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/pointerfacts.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/pointerfacts.rs sha256:03c2cf1175f02ad24127f077476982bbdcb94caf11ff9ccc00e4ebdaa7430453 -->

# pointerfacts

Constant byte offsets between pointer values, and disjointness of two
accesses that share a base.

## How it works
`Offsets::relative(pointer)` walks `getelementptr inbounds` chains whose
indices are all integer constants, summing the byte offset with the data
layout (`collect_offset`), and returns the value it stopped at and the
offset from it. It stops at a value that is not such a `getelementptr` (a
parameter, an alloca, a global, a variable index, a step not `inbounds`).
`fixed` is the same through steps that need not be `inbounds`, for when the
address matters and not the object it stays in. A cycle is refused, and so
is an offset that does not fit the pointer's index width.

`inbounds` is the MIR's own proof that the result stays in its base's
allocation, so two pointers that reach one base lie in one allocation. Only
then does `comparable` return their two offsets, and `disjoint` and
`same_bytes` compare the byte ranges `[offset, offset + bytes)` of two
`Location`s. Two unrelated pointer values never prove disjointness.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  base["<b>%base</b>"]:::mid
  a["<b>%a</b><br/><small>relative: (%base, 0)</small>"]:::mid
  b["<b>%b</b><br/><small>relative: (%base, 8)</small>"]:::mid
  base -->|"gep inbounds 0"| a
  base -->|"gep inbounds 8"| b
```

*Location{%a, 8} and Location{%b, 4} are disjoint, since 0 + 8 <= 8*

Facts are read from the current SSA on each call, not stored on operands a
later transformation could retarget.

## Provides
`Offsets` (made by `offsets`), `Location`. Consumers: [`crate::memoryssa`](memoryssa.md),
[`crate::interprocedural`](interprocedural.md) (`cannot_fault`) and `argpromotion` in
llrm-transforms.

## Needs
`llrm_mir::datalayout::DataLayout` for offsets and index widths.
