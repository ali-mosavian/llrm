---
architecture-index: strong alias analysis
architecture-order: 60
---
# Strong alias analysis

For contributors writing or reading an analysis or pass that asks about
memory. One machine-independent vocabulary answers every such question, for
every frontend; it lives in `crates/opt/llrm-analysis/src/` (`memory.rs`,
`regions.rs`, `alias.rs`, `memoryssa.rs`).

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  pv["<b>pointer value</b>"]:::mid
  prov["<b>Provenance</b><br/><small>slices, restrict roots</small>"]:::mid
  ls["<b>load/store</b>"]:::mid
  mr["<b>MemRef</b><br/><small>root + constant offset<br/>+ one index, width</small>"]:::mid
  ov["<b>may they meet?</b><br/><small>(two MemRefs)</small>"]:::mid
  call_["<b>call</b>"]:::mid
  eff["<b>bytes of objects</b><br/><small>read or written</small>"]:::mid
  cons["<b>GVN, DSE, LICM,</b><br/><small>promote</small>"]:::opt
  pv -->|"points_to"| prov
  ls -->|"MemRef::of"| mr
  mr -->|"regions::overlapping"| ov
  call_ -->|"Summary/Effect"| eff
  ov --> cons
  eff --> cons
```

*From pointer values to answers about memory*

## Objects and slices

A memory object has a `MemoryKind`, a stable id and an optional extent. The
kinds MIR produces are `Frame` (an alloca), `Global`, `Parameter` (what a
pointer parameter points to), `Nonlocal` (what a callee may reach), `Absolute`
(linear memory) and `Unknown`. Different concrete objects are disjoint.
`Nonlocal` reaches globals and escaped objects but not an unescaped frame;
`Unknown` reaches everything.

An access is a half-open byte `Slice` of one object, with a stride and an
element width. Two slices intersect by their modular byte lanes, not by their
hull, so interleaved even and odd lanes are disjoint. Non-wrapping range facts
narrow an indexed access to its possible start offsets.

## Points-to and escape

`points_to` ([source](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/alias.rs#L2436))
has two phases. `point_values` flows provenance through SSA values, `select`s,
phis and exact pointer spill slots: an exact store is a strong update, an
overlapping or unresolved store kills the fact, a join keeps a slot only when
every edge defines it, and a missing phi arm gives `Unknown`. The escape
phase is a second forward dataflow: publishing a pointer, returning it or
passing it to a capturing callee exposes its object from that point on, so a
call sees only what escaped before it.

The value solve does not read call arguments or captures, so it is its own
analysis (`PointerValues`) and `Pointers` and `CallEffects` each run only their
escape phase on it.

## Calls

A `Summary` holds the slices a procedure reads and writes in its own
parameter space, the parameters it captures, and unknown-read and
unknown-write flags. Summaries are solved to a fixed point over the call
graph; a body that may be replaced (weak, linkonce) describes no call.
`instantiated` rebases a summary onto each actual pointer's provenance. An
unknown callee reaches all nonlocal storage, the whole object behind each
pointer actual and what has already escaped; it cannot reach an unrelated
frame object. The data model and its cost are in [modref.md](modref.md).

What the C library is known to do (`strlen` reads its arguments and keeps
none) is stated as attributes on the declaration. A local definition overrides it; a name's
decoration alone grants no contract.

## Language promises

`!tbaa` types make two accesses disjoint unless one type is an ancestor of the
other (`typed_apart`); character and untyped accesses stay conservative.
`llrm-cc` writes C's aliasing classes as a TBAA tree, and a C `restrict`
parameter becomes a `noalias` parameter, which gives it a restrict root that
travels with the pointer through copies, arithmetic and phis. `llrm-nib` and
`llrm-qb` state their own facts the same way, through `llrm_hir::facts`.
