> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/nearest.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/nearest.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/nearest.rs sha256:36770fb8ed5e1f12968a3c3a9a73c4f31abbb77049e6f6f3a0e20328b8011762 -->

# nearest

The items placed in blocks that dominate a given block, nearest first.

## How it works
Loads look for a provider among the stores and loads that dominate them.
Scanning every item of the function per load is quadratic. `Nearest::of`
builds, per key, the blocks that hold an item of that key, sorted by their
preorder number in the dominator tree, and links each to the nearest block
above it in the tree that also holds one. `Nearest::dominating` binary
searches for the last block at or before the query block, walks the links up
until a block's subtree span contains the query, and yields the items from
there up. The work is the length of that chain, not of the function.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  entry["<b>entry</b>"]:::mid
  a["<b>A (K)</b>"]:::mid
  b["<b>B</b>"]:::mid
  c["<b>C (K)</b>"]:::mid
  e["<b>E (K)</b>"]:::mid
  f["<b>F</b><br/><small>query</small>"]:::mid
  entry --> a
  a --> b
  a --> c
  c --> e
  e --> f
```

*Key K holds items in A, C, E; sorted by preorder A C E with up links - A C.
A query at F finds E, the last block <= F, and walks the chain E -> C -> A*

Items of several keys are merged by a cursor per key: the walk takes the
largest preorder number first, and within a block the last item first. An
item reached through two keys is returned once. A block the dominator tree
does not reach is left out of the chains and, as the query, yields nothing:
everything dominates it, so the caller decides.

## Example
None: the module has no MIR input of its own. Its test,
`test_the_items_that_dominate_a_block_are_found_nearest_first` in
`nearest.rs`, places items in a generated chain of diamonds and compares
`dominating` with a scan of every item. The figure above is the worked case.

## Provides
The `Nearest` index and its `Walk` iterator. Consumers: [`crate::avail`](avail.md) and
`loadjoins` in llrm-transforms.

## Needs
`llrm_mir::domtree::DomTree` (`span`, the preorder interval of a
block).
