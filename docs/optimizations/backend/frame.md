> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/frame.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/frame.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/frame.rs sha256:4a9fce5253a9173d46dfcd411c6df7640fbd603c8bf706bc6c27058c090d2444 -->

# frame

The stack frame of one function body: where each spill slot, temporary and
selector-made stack object lives, and how many bytes the prologue reserves.

## How it works
Displacements are bytes from the frame register, negative below it. A
`Frame` has a `floor`, the deepest displacement already reached, and hands
out slots below it:

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','textColor':'#64748b'}}}%%
block-beta
  columns 8
  o1["bp+n"] c1["incoming arguments"]:3 s1[" "] n1[" "]:3
  o2["bp+0"] c2["caller's bp"]:3 s2[" "] n2[" "]:3
  o3[" "] c3["hole"]:3 s3[" "] n3["spill slots fill this first#59; ends at bp-hole"]:3
  o4[" "] c4["selector slots"]:3 s4[" "] n4["allocas, homed arguments (extents)"]:3
  o5[" "] c5["runtime frame"]:3 s5[" "] n5["BASIC: fixed size, from the runtime entry call#59; floor = lowest displacement already reached"]:3
  o6[" "] c6["spill slots"]:3 s6[" "] n6["one per value, at least a word (slot)#59; lowest: size() below the floor"]:3
  classDef off fill:transparent,stroke:transparent,stroke-width:2px,color:#64748b
  classDef cell fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff
  class o1,n1,s1,o2,n2,s2,o3,n3,s3,o4,n4,s4,o5,n5,s5,o6,n6,s6 off
  class c1,c2,c3,c4,c5,c6 cell
```

*Frame layout, incoming arguments at the top*

`of` builds the frame after selection. It scans the body for the lowest
frame cell referenced (outgoing native stack arguments excepted) and, for a
BASIC procedure that enters through the runtime's frame call, for the
runtime frame size: the constant in `cx` at that call plus the runtime's own
10 bytes (20 for VB/DOS). A size that is not a known constant, or a native
frame referenced below its reservation, is `Refused`.

`slot` gives a value a slot on first request, keyed by value or by a name;
it fills the hole just below the frame register first, then the space under
the lowest slot. A slot belongs to its value: sharing is done by the callers
that colour slots (`spiller`, isel's alloca groups), not here. `cell`
returns the slot as a frame memory operand. `saved` and `restore` roll back
the slots of a trial that is discarded.

`tagged` marks every frame cell in the body with the slot it is in (nearest
`extents` entry), so later phases and the verifier can tell which stack
object an address belongs to; a cell in no slot is refused. A pointer one
past a slot's end or one before its start belongs to that slot.

## Runs
`lir frame` in `assemble::staged`, once per function after selection and
`framefree`; the frame is then shared by every machine phase that spills.

## Needs
[`crate::backend::isel`](isel.md) for `extents`, `calls` and `hole`;
[`crate::backend::nativeframe`](nativeframe.md) for a native procedure's reservation.

## See also
`docs/optimizations/second-frame.md`.
