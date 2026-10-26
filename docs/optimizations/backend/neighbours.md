> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/neighbours.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/neighbours.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/neighbours.rs sha256:1be5192ef6efafd573ea7bac0a7a0817557e3727bbe7f1ca123edc10507cc1b7 -->

# neighbours

The values one value interferes with: a sorted set that turns into a bit
vector once it is dense.

## How it works
A function with N values live together has up to N^2 interference pairs.
As tree sets each costs a node and a descent; as bits, one shift. A set
starts as a `BTreeSet`. When it reaches `DENSE_AT` members, and again at
each multiple, it becomes `Dense` if its largest id is below
`BITS_PER_MEMBER` times its size, so a set of widely spread ids stays a
tree. This is GCC IRA's choice for an allocno's conflicts: a vector of ids
or a bit vector over its id range, whichever is smaller
(`ira_allocate_object_conflicts`, `ira-build.cc`).

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  a["<b>Few(BTreeSet)</b>"]:::mid
  b["<b>Dense #123; words, len #125;</b><br/><small>bit v of word v/64</small>"]:::mid
  a -->|"len #gt;= 64,<br/>ids dense enough"| b
```

*Neighbour set representations*

`join` adds a whole group at once: every value in a mask except one becomes
a neighbour, a word at a time when the set is dense. Iteration is ascending
in both forms.

## Example
No honest example: a set type inside the allocator's interference graph,
with no LIR of its own. Read `tests` in this file.

## Used by
`coalesce`, for its interference graph.
