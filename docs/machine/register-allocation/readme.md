# Register allocation

Audience: an engineer who must change or debug the register allocator. This page is the index and the story from a function's virtual values to its registers and frame slots; each stage has a page of its own.

The code is in `crates/codegen/llrm-codegen/src/backend`. The allocator is LLVM's greedy allocator (`RAGreedy`, with its `SplitKit`, `SpillPlacement`, `InlineSpiller` and last-chance recoloring) with these differences: pricing is by `executed::work` and block frequencies, the search runs the allocation on several shapes of a body and keeps the cheapest, and a second route puts an SSA spiller in front.

| Page | Holds |
|---|---|
| [inputs-and-classes](inputs-and-classes.md) | The register file per target, classes, what an instruction forces, what is refused |
| [live-ranges](live-ranges.md) | Intervals, holes, spill weights, how a register's holders are queried (there is no interference graph) |
| [assignment](assignment.md) | The queue, the ladder from a free register to a spill, the search over shapes, the cost model |
| [splitting](splitting.md) | How a range is cut: local, region and block splits, placement, carving |
| [spilling](spilling.md) | Stores, reloads, slots and their sharing |
| [rematerialization-and-forwarding](rematerialization-and-forwarding.md) | Values made again, reads in place, forwarding |
| [coalescing-and-copies](coalescing-and-copies.md) | Phi elimination, ties, the coalescer, hints, parallel copies |
| [after-allocation](after-allocation.md) | Peephole, loop slots, the frame, shrink-wrapping |
| [x87](x87.md) | Floating values on the register stack |
| [debugging](debugging.md) | Flags, environment switches, dump and debug lines |
| [examples](examples.md) | Three functions through the phases, with their dumps |

## Where it sits

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  classDef data fill:#7c3aed,stroke:#5b21b6,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  isel["<b>isel, frame</b><br/><small>SSA LIR, constraints</small>"]:::src
  ssa["<b>SsaSpill</b><br/><small>route B only</small>"]:::data
  phi["<b>PhiElimination, x87 phases</b><br/><small>parallel copies</small>"]:::mid
  tw["<b>TwoAddress, Coalescer</b><br/><small>ties, joins</small>"]:::mid
  ra["<b>RegAlloc</b><br/><small>constrain, assign, split, spill</small>"]:::opt
  pp["<b>ParallelCopy, Peephole, LoopSlots, Scheduler, ControlFlow</b>"]:::mid
  masm["<b>masm</b><br/><small>frame, saved registers</small>"]:::out
  isel --> ssa --> phi --> tw --> ra --> pp --> masm
```

*The phases of `flow::machine_with`, in order. `Prologue` is in the list and skipped where functions are made (`assemble::phased_to`); `masm` writes the prologue.*

`RegAlloc` consumes LIR out of SSA (no phis; copy groups), with virtual values for everything but pinned ones, and with the constraints stated on instructions. It produces LIR in which every `Held` operand is a register or a frame cell, with the spill traffic marked. Each phase's output is verified under `LLRM_VERIFY=1`.

## The story

1. **Selection** gives a function as SSA LIR over virtual values, with `requires`, `delivers` and `clobbers` on the instructions that call, return or use fixed registers, and a frame ([inputs-and-classes](inputs-and-classes.md)).
2. **Out of SSA.** Phis become parallel copies, `TwoAddress` ties destinations to sources, x87 values go to the register stack ([x87](x87.md)), and the coalescer joins the copies it can prove safe ([coalescing-and-copies](coalescing-and-copies.md)).
3. **Prepare.** `RegAlloc::transform` gives every required register a value of its own, fixes class conflicts, and refuses a body no allocation can exist for.
4. **Allocate.** Every value is queued with its live interval ([live-ranges](live-ranges.md)). Each is tried in a free register, then by evicting cheaper holders, then split, then recolored, then spilled ([assignment](assignment.md), [splitting](splitting.md), [spilling](spilling.md)). Splits and spills rewrite the body in place and the affected values are queued again.
5. **Search.** If something was spilled, the body is allocated again in other shapes and the cheapest result by `executed::work` is kept.
7. **Apply.** Each operand naming a value is replaced by its register.
8. **After.** Parallel copies are ordered, the peephole removes copies and reloads, loop slots move into free registers, and `masm` builds the frame ([after-allocation](after-allocation.md)).

The allocator does not fail where a register exists. The only refusal it gives is `unallocatable`, before any placement; every other body ends with a register or a slot for each value ([`regalloc_total_tests.rs`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/regalloc_total_tests.rs#L1) holds a fixture for each refusal it once made, and the [fuzz lane](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/regalloc_fuzz_tests.rs#L1) generates bodies at high pressure with every constraint kind; `FUZZ_SEEDS=N` sets the seeds).

## Terms

| Term | Means |
|---|---|
| value | a virtual register, `vN` in dumps; wide or narrow, it is placed in one 32-bit root |
| interval | the slots a value is live in: sorted segments with holes |
| pinned, fixed | a value that must be in one register |
| class | the registers a value may be in |
| weight | what spilling a value costs per slot it is live |
| stage | how far a value is in `Assign`, `Split`, `Spill`, `Done` |
| piece | the value a split makes for a region of a range |
| remat | a value made again where it is read, with no slot |
| shape | a rewrite of the body the search tries |
| route | the allocator alone, or `SsaSpill` first |
