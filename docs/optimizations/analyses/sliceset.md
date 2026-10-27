> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/sliceset.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/sliceset.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/sliceset.rs sha256:c4c3475c229284f7298b2ec23b103d769d08e4120ccdad6bef57523e21e208be -->

# sliceset

An ordered set of memory slices that holds most of itself as a run shared
with other sets, so the callback effects every call summary repeats are
built, compared and listed once.

## How it works
A function that may call back into the module (an address-taken function or
an entry) has, in its summary, the effects of all those callbacks. That run
of slices is the same for every such summary, every instantiation of it at a
call and every call's effect. Building, comparing, filtering and listing a
tree set holding it each time is wasted work.

`Shared` is one immutable run of slices behind an `Rc`, with facts computed
once: whether none is in a `Frame` or `Stack` object (`outlives`), whether
any is a `Parameter`'s, and which objects it holds whole or in part. A
`SliceSet` is `own` (a `BTreeSet<Slice>`) plus an optional `base:
Rc<Shared>`, the two disjoint. Seen as a set, it is exactly the union.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  ss["<b>SliceSet</b><br/><small>own, few</small>"]:::src
  sh["<b>Shared</b><br/><small>(one allocation, many holders)<br/>set, outlives, parameters,<br/>wholes, parts, coalesced</small>"]:::opt
  other["<b>another SliceSet</b>"]:::mid
  ss -->|"base"| sh
  other -->|"base (same Rc)"| sh
```

*A SliceSet is own plus a base; another SliceSet's base is the same Rc, so
ptr_eq is a shortcut*

`absorb` takes the other set's run by reference when this set has none or
the same one, and copies slices otherwise. `retain` and `retain_outliving`
ask the run as a whole where `shared_kept` can answer, and `materialize` it
into `own` where only per-slice tests can. `coalesced` drops the subranges
of an object the set has every byte of; the run is materialized only if its
own wholes and parts disagree or the set's own wholes cover a part of it.
`mapped` rewrites `own` and keeps the run, so the map must not change what
the run holds. Equality compares by `Rc::ptr_eq` and `own` when the runs are
the same.

## Example
None: this is a container, with no MIR input and no printed output. Its
test, `a_set_with_a_shared_run_is_the_union_of_it_and_its_own` in
`sliceset.rs`, checks a `SliceSet` holding a shared run against a `BTreeSet`
of the same slices.

## Provides
`SliceSet`, `Shared` and `whole`. Consumers: [`crate::alias`](alias.md) (call
summaries and `callbacks_over`) and
the `interprocedural` pass in llrm-transforms.
