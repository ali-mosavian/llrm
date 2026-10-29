> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/pipeline.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/pipeline.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/pipeline.rs sha256:6a9b3b58ed21fe7fa39b42a8282ded19457315d10cd161a0e66c8792ab639e81 -->

# pipeline

The MIR optimization pipeline every driver runs: the order of the passes,
the per-body fixed point, and which options switch them off.

## How it works
`recorded` (and `applied`, which drops its result) builds a
`PassManager` and adds, in this order: whole-module steps, `Fixed` over
every body, the whole-program [`crate::interprocedural`](../analyses/interprocedural.md) step, and loop and
address passes that run once, after the program is settled. Every pass
after `globalopt` is a function, module or program pass of the manager.
With `LLRM_VERIFY` set the module is verified after each manager pass;
with `Applied::dump` set each changed step of each body is written there.
A body sent back through `Fixed` by a later stage starts only the passes
that found nothing last time and read nothing that changed since
([`crate::settled`](settled.md)); `LLRM_CHECK_SETTLED=1` runs them and fails where one
changes the body.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart TB
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef note fill:transparent,stroke:transparent,color:#64748b
  linkStyle default stroke:#64748b,stroke-width:2.2px
  o0["-O0 (Options::optimize = false)<br/>available-externally only"]:::note
  a["<b>A  once, whole module</b><br/><small>globalopt, ports (every body),<br/>stamp</small>"]:::mid
  subgraph g_b["B  EACH BODY, TO A FIXED POINT (FIXED, MIR PIPELINE)"]
    sroa["<b>sroa</b><br/><small>(boundary#59; also after a copy)</small>"]:::mid
    rnd["<b>round, until no pass changes</b><br/><small>the body: fold ports simplifycfg<br/>decide equalcalls tailrec<br/>loopsimplify lcssa floatloop<br/>licm<br/>trivialunswitch inferspace<br/>dse gvn promote indvars<br/>algebraic dead#59; unroll once<br/>per round (a change sends<br/>the copy to sroa)</small>"]:::mid
    peel["<b>peel</b><br/><small>while it removes loops:<br/>sroa, then B's rounds</small>"]:::mid
    unsw["<b>unswitch (-O3, -Omax)</b><br/><small>simple-loop-unswitch,<br/>non-trivial</small>"]:::mid
    swap["<b>interchange (rounds only)</b><br/><small>sroa, then B's rounds</small>"]:::mid
    mrg["<b>merge (once)</b>"]:::mid
  end
  c["<b>C  whole program,</b><br/><small>interprocedural: inline,<br/>constants, dead pure calls<br/>(stages: LLRM_DEBUG=runs)</small>"]:::mid
  d["<b>D  globaldce,</b><br/><small>available-externally,<br/>calleepop#59; freeze the<br/>Summaries and GlobalsAA</small>"]:::mid
  e["<b>E  once, each body, with the</b><br/><small>analyses frozen<br/>[rotate#123;copy#125; loopsimplify<br/>lcssa fold algebraic<br/>indvars#123;exit#125; dead]: only<br/>with -ftree-ch, not at -Os<br/>fixednarrow lsr differences<br/>window rotate#123;proven#125;<br/>hoist gepoffset<br/>cgpaddr sink</small>"]:::mid
  f["<b>F  last: spares, homes</b><br/><small>(instruction ids selection sees)</small>"]:::mid
  a --> sroa
  sroa --> rnd
  rnd --> peel
  peel --> unsw
  unsw --> swap
  swap --> mrg
  mrg --> c
  c -.->|"a changed body goes<br/>back through B"| sroa
  c --> d
  d --> e
  e --> f
  style g_b fill:#47556912,stroke:#475569,stroke-width:1.6px,stroke-dasharray:7 5,color:#475569
```

*The mir pipeline, stages A to F*

The order of B is the `every` list of [`pipeline`](pipeline.md); `passes` returns it
as names. `Fixed` takes two passes out of it: `sroa` runs at structural
boundaries, `merge` once at the end. A pass that is off is not in the list.

A round runs the remaining passes in order. The body is at a fixed point
when a round changes nothing; a pass reports a change by preserving less
than every analysis. A pass that changed nothing is skipped until an edit
reaches what it reads (`FunctionPass::reads`, `footprint::of`). The limit
is `max(16, size + 1)` rounds, and a round that reproduces an earlier
printed body is an error, so an oscillating pair fails at once. `sroa` and
`promote` are skipped on a body with an irreducible loop: promotion needs
dominators.

Peeling repeats while it leaves fewer loops, so an outer loop whose inner
loop was peeled is peeled in turn. The passes that make a candidate
re-optimize it with a `Fixed` of its own that has unswitching
off.

In E, `lsr` is after the interprocedural step because nothing after it may
choose a loop's counters again; `rotate{proven}` is last of the loop
passes; `hoist`
runs again to lift what a rotated loop (run at least once) may now leave;
`gepoffset` follows it because it would move a constant `gep` out of its
loop; `cgpaddr` follows the last `gvn`, which makes the phis it sinks;
`sink` follows `cgpaddr`, once the passes that move work up have run.

## Example
No honest example: the pipeline is the order of the passes, not one rewrite,
and no one function shows it. `llrm-cc --dump DIR` writes the module after
each stage (`NN-stage.ll`) and, for each body, each changed step of the
fixed point (`N/NNN-rRR-pass.ll`); the pass examples come from those.

## Runs
Options are `Options`; `Level::options` in llrm-codegen's `driver/flags.rs`
maps `-O0`, `-O1`, `-O2`, `-O3`, `-Omax`, `-Os`, `-Oz`, `-Og` to
`Options::none`, `basic`, `standard`, `speed`, `aggressive`, `size`,
`min_size` (`-Og`: `basic` without `loopidioms`). `-f` flags set single
fields (`PASSES` in the same file). The per-level table, with the gcc and
LLVM counterpart of each limit, is
[docs/reference/levels.md](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/docs/reference/levels.md).

`Options::wanted` gates passes by name, so every pass can be bisected off:

| field | passes |
|---|---|
| `optimize` | all; off, only `available-externally` runs |
| `lcssa`, `floatloop`, `fold`, `simplifycfg`, `decide`, `dead` | the pass of that name |
| `hoist` | `hoist`, `loopmotion` |
| `forward` and `drop_loads` | `gvn`, `equalcalls` |
| `drop_stores` | `dse` |
| `sink` | `sink` |
| `promote` | `sroa`, `promote` |
| `strength` | `indvars`, `lsr`, `canonfreeze`, `window`, `gepoffset`, `cgpaddr` |
| `loopidioms` | `loopidioms`, `merge`, `memcpyopt` |
| `sibcalls` | `tailrec` |
| `unswitch`, `interchange`, `unroll_and_jam` | the passes of that name (`unswitch` and `unroll_and_jam` on at -O3 and -Omax; `interchange` on at every level but -O0) |
| `copy_headers` | the header copy in E (off by default; `-ftree-ch`) |

`pre` makes `gvn` insert on the path that lacks a value; `for_size` prices
`hoist`, `algebraic`, `loopidioms`, `lsr`, `window` and
`calleepop` in bytes. `limits` are the copy budgets of the unroller.

## Needs
`Summaries`, `GlobalsAA` and `ProgramSummaries` are required
up front, and frozen at the end of D so the passes of E read them without
recomputing after each edit. In LLVM's order `GlobalsAA` and
`GlobalsModRefAnalysis` are made before the walk and again at the end of D
alone (`RequireAnalysisPass`, `RecomputeGlobalsAAPass`), read cached
between.

## See also
gcc's pass lists, `passes.def`; LLVM's `PassBuilderPipelines.cpp`.
`LLRM_DEBUG=time` times each step, `runs` counts rounds, skips and the
work of passes that changed nothing.
