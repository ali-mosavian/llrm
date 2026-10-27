> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/regalloc_input.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/regalloc_input.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/regalloc_input.rs sha256:4904a5cb9712ee6d118876ff64d17aae08864e512bb288d53a085577888f3b0f -->

# regalloc_input

Test support: the machine-phase list and the LIR a fixture has reached just
before a chosen phase.

## How it works
`before_phase_skipping` parses a MIR fixture under `tests/check/mir`,
selects one function for the 16-bit target (`isel::selected`), lays out its
frame (`frame::of`) and builds the machine phase list with `flow::machine`.
It runs the phases in order until the one whose class name is `phase_class`,
skipping any named in `skipped`, and returns the body so far and the
remaining phases starting with that one.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
flowchart LR
  classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
  linkStyle default stroke:#64748b,stroke-width:2.2px
  f["<b>fixture.mir</b>"]:::mid
  i["<b>isel</b>"]:::mid
  fr["<b>frame</b>"]:::mid
  run["<b>phase 1 .. k-1</b><br/><small>run here, body returned</small>"]:::mid
  rest["<b>phase k .. last</b><br/><small>returned, unrun</small>"]:::mid
  f --> i
  i --> fr
  fr --> run
  run --> rest
```

*Where the fixture's phases stop*

The wrappers fix the usual arguments. `before_regalloc` and
`before_regalloc_in` stop before `RegAlloc` with the spiller run, as in
production. `before_regalloc_unspilled` leaves out `SsaSpill`, handing the
allocator the pressure the spiller removes, for tests of the allocator that
must hold for any input. `Calls` states what a call leaves alone: C's
convention keeps everything but ax, bx, cx, dx, es and the flags; a Nib or
BASIC program's keeps nothing. `through` runs a body through given phases.

## Example
None: a test-support module with no transformation of its own. Its callers
show the LIR it hands over; read `lirtext.rs` for the text format.

## Used by
Tests of [`crate::backend::allocate`](allocate.md), `crate::backend::ssaspill`,
[`crate::backend::loopslots`](loopslots.md) and the jumps and total-allocation test files.
Compiled only under `cfg(test)`.
