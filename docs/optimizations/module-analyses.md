# Module analyses after an edit

For contributors to the pass manager and the interprocedural stage. Explains how `Summaries` and `GlobalsAA` are brought up to date after a body changes instead of being recomputed over the whole module.

## The cost it removes

The interprocedural stage ([`interprocedural.rs`](passes/interprocedural.md)) inlines, specialises and reverts, and each splice reports the body it edited with `ModuleAnalyses::changed` ([`passes.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L1438)). The next consumer of a module analysis would otherwise recompute it over every body. `LLRM_DEBUG=time` shows the `summaries`, `call-effects` and `globals-aa` rows; `tools/pass-profile.py` sums them over the programs ([compile-time](../working/measurement/compile-time.md)).

## Mechanism

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef data fill:#7c3aed,stroke:#5b21b6,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  edit["<b>pass edits body f</b><br/><small>changed(f) / body_edited(f)<br/>ModuleAnalyses.edited += f</small>"]:::src
  ask["<b>next consumer asks</b><br/><small>get::#lt;Summaries#gt;()</small>"]:::mid
  held["<b>result held?</b>"]:::mid
  drop["<b>dropped result,</b><br/><small>kind.unchanged()?</small>"]:::mid
  reuse["<b>reuse</b>"]:::out
  run["<b>run()</b><br/><small>dirty = bodies whose mark differs<br/>from the memo's (+ declarations<br/>that differ by value)</small>"]:::opt
  upd["<b>alias::summaries_updating</b><br/><small>(procedures, known, memo, dirty)</small>"]:::mid
  clo["<b>closure = dirty + every body that</b><br/><small>reads them through calls<br/>(+ all entries' callbacks if an<br/>entry is in it)<br/>restarts from its direct summary#59;<br/>the rest is kept</small>"]:::data
  edit --> ask
  ask --> held
  held -->|"yes"| reuse
  held -->|"no"| drop
  drop -->|"yes"| reuse
  drop -->|"no"| run
  run --> upd
  upd --> clo
```

*What a body edit costs the next consumer of Summaries*

- `ModuleAnalyses::memo<T>` ([`passes.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/ir/llrm-mir/src/passes.rs#L1264)) holds what an analysis keeps for its next run, by type; `invalidate` does not drop it.
- `Summaries::run` ([`manager.rs`](analyses/manager.md)) keeps each body's calls and exposed-frame facts while the body's `mark` and the declarations are unchanged, and passes the dirty set to `alias::summaries_updating` ([`alias.rs`](analyses/alias.md)). A body outside the closure reads nothing inside it, so its summary stands. A change of the globals' facts, an added or removed global, or a missing memo makes it a whole run.
- `GlobalsAA` ([`globalsaa.rs`](analyses/globalsaa.md)) keeps its per-body contributions in a memo keyed by body and refolds them.
- Analyses that read no body (type ancestry, global sizes, call registers) survive a body edit.

## Check switch

`LLRM_CHECK_MODULES=1` runs every module analysis again from scratch beside the updated one and asserts the two are equal; the panic names the summaries that differ and the dirty set. Run it over the bench programs and the test programs when changing an analysis that has a memo.

## Related

[early-summaries](early-summaries.md) covers what the pipeline does with summaries between edits. [analyses](https://github.com/ali-mosavian/llrm/tree/main/crates/opt/llrm-analysis/src) lists the module analyses.
