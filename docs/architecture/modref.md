---
architecture-index: call effects
architecture-order: 70
---
# Call effects

For contributors working on interprocedural analyses or their compile time.
How a call's reads and writes are computed and consumed, and what is still
open.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  bodies["<b>callee bodies</b>"]:::mid
  sum["<b>Summary per function</b>"]:::mid
  gaa["<b>GlobalsAA, callbacks</b>"]:::mid
  site["<b>call site + actuals'</b><br/><small>provenance (points_to)</small>"]:::mid
  eff["<b>Effect per call</b><br/><small>loads, stores, fills</small>"]:::opt
  users["<b>memoryssa, gvn, dse,</b><br/><small>promote, loopmotion,<br/>consts, avail</small>"]:::out
  bodies -->|"summaries<br/>(fixed point)"| sum
  gaa --> sum
  sum -->|"calls_annotated"| eff
  site --> eff
  eff --> users
```

*How a call's reads and writes are computed and consumed*

## What exists

A `Summary` ([source](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/alias.rs#L321))
is the modref record of one function: slices of objects read and written,
captured parameters, and unknown-access flags. The `Summaries` analysis holds
one per defined function; `GlobalsAA` adds which globals only the module
reaches by name, and the callbacks (what an address-taken function may do) are
held once.

[`calls_annotated`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/opt/llrm-analysis/src/alias.rs#L1643)
instantiates a callee's summary at each call against its actuals and returns
an `Effect`: the loads and stores (lists of `MemRef`) and the `fills` from
`initializes`. The `CallEffects` analysis holds it per body.

Most consumers ask a yes or no about one reference. Some enumerate the lists:
`consts` kill sets, `avail::after` and the dead-store walks.

`stamped_all` in `llrm-transforms` writes the proved `memory`, `nocapture` and
`initializes` attributes onto every declaration, callees first.

## Open

Per-body points-to and call-effects work is redone for every body version:
[#779](https://github.com/ali-mosavian/llrm/issues/779),
[#1166](https://github.com/ali-mosavian/llrm/issues/1166),
[#558](https://github.com/ali-mosavian/llrm/issues/558). The design target is
GCC's modref: a summary queried per reference, not a footprint built for every
call. The rules for keeping an answer sound across edits are in
[points-to-lifetime.md](points-to-lifetime.md).
