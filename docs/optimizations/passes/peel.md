> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/peel.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/peel.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/peel.rs sha256:0d98f2e1b0da6bbb29e3a68dccd498684f475cd31b603cb8462d5612158d26db -->

# peel

Whole-loop peeling: clone every iteration of a loop with a known trip
count ahead of it, priced before anything is cloned, and let Fold, Decide
and Dead prove the residual loop unreachable.

## How it works
Peeling is the CFG counterpart of full straight-line unrolling. For each
loop of a copy of the body, closed to LCSSA (`lcssa::closed`), the pass
- takes the trip count (`induction::trip_count`, else `trip_bound`) and
  skips the loop below two trips;
- asks `llrm_analysis::peelsize::admitted` whether the copies are worth
  their size: the budgets are `peelsize::Limits`, counted in instructions,
  and the loop's `Site` comes from `profit::site`;
- clones the iterations with [`crate::loopclone::peeled`](loopclone.md).

The copy is kept, and the next loop looked for, until none is admitted.
Each loop is peeled once, named by its latch. The residual loop stays as a
correctness fallback: the copies' header tests fold to constants, so the
branch into it is decided and `Dead` removes it.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  pre["<b>pre</b>"]:::mid
  h["<b>H</b>"]:::mid
  b["<b>B</b>"]:::mid
  ex["<b>exit</b>"]:::mid
  pre --> h
  h --> b
  b --> h
  b --> ex
```

*Loop of 3 trips*

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  pre["<b>pre</b>"]:::mid
  h1["<b>H1</b>"]:::mid
  b1["<b>B1</b>"]:::mid
  h2["<b>H2</b>"]:::mid
  b2["<b>B2</b>"]:::mid
  h3["<b>H3</b>"]:::mid
  b3["<b>B3</b>"]:::mid
  h["<b>H</b>"]:::mid
  b["<b>B</b>"]:::mid
  pre --> h1
  h1 --> b1
  b1 --> h2
  h2 --> b2
  b2 --> h3
  h3 --> b3
  b3 --> h
  h --> b
  b --> h
```

*After peel*

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  pre["<b>pre</b>"]:::mid
  b1["<b>B1</b>"]:::mid
  b2["<b>B2</b>"]:::mid
  b3["<b>B3</b>"]:::mid
  ex["<b>exit</b>"]:::mid
  pre --> b1
  b1 --> b2
  b2 --> b3
  b3 --> ex
```

*After fold, decide and dead*

The one machine number is the target's: a function with an instruction the
target does not price (`profit::priced`) is left alone, since
`peelsize` cannot size it. The target's `unroll_budget` is scaled by
`Limits::target_percent`.

## Runs
In [`crate::pipeline`](pipeline.md)'s per-body fixed point `Fixed`, after the scalar
passes settle: while a peel leaves fewer loops, the scalar rounds run
again and the pass is asked once more, so an outer loop whose inner loop
was peeled may be peeled in turn. On at -O3, -Omax and -Os (`peel`,
`-fpeel-loops`); off at -O1, -O2, -Og and -Oz. Budgets per level:
`docs/reference/levels.md`.

## Needs
`Registers` facts, `induction`, `peelsize`, `lcssa`, `loopclone`.

## Keeps
Nothing when it peels.

## See also
LLVM's `peelLoop` (`LoopPeel.cpp`) priced by `LoopUnrollPass`'s full-unroll
cost model; gcc's `try_unroll_loop_completely`, `-fpeel-loops`.
[`crate::unroll`](unroll.md) copies a loop in straight line instead.
