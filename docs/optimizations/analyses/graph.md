> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/graph.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/graph.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/graph.rs sha256:1f23245f3efba010861c1aee58bd36cdd7c8292be85a158a4a3756eda186bb96 -->

# graph

Block-graph algorithms (loops, depth, reducibility, dominators) over any IR
whose blocks implement `loops::Node`.

## How it works
The module holds one submodule, `loops`. A `Node` is a block id and its
successor ids; nothing here knows an IR. The rich MIR adapts a `Function`
through [`crate::cfg`](cfg.md), which turns it into `Block`s in layout order.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  f["<b>Function</b>"]:::mid
  b["<b>[Block#123;at,succ#125;]</b>"]:::mid
  l["<b>graph::loops</b><br/><small>(Loop, depth, ...)</small>"]:::mid
  f -->|"cfg::graph"| b
  b --> l
```

*From a Function to loops*

The backend's `LirBlock` has its own copy of these walks in
`crates/codegen/llrm-codegen/src/analysis/loops.rs`; both build on
`llrm_support::graph`.

## Example
No honest example: this module only holds `loops`. Read the example there.

## Provides
The algorithms of `loops`. Consumers: [`crate::cfg`](cfg.md) (`Loop`, `Node`,
`irreducible_under`) and the loop passes of `llrm-transforms`, which read
`Loop` out of [`crate::cfg::Shape`](cfg.md).
