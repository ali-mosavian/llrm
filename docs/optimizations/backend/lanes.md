> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/lanes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/lanes.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/lanes.rs sha256:a4a1fad275b4918f147c1a381f2d49328ede53a40a7240e67e8f8afcd2294a73 -->

# lanes

A set of register and flag lanes, as a 128-bit mask.

## How it works
A lane is a byte of a general register root (4 each, 7 roots of a 32-bit
target), a byte of a segment register (2 each, 6 segments), or a flag bit
against `RegId::None` (32). Long mode adds 8 roots and a root's upper four
bytes, which no view splits, are one lane: 5 lanes for each of 15 roots, 119
lanes in all, which fit the mask a 32-bit target has always had. The table
sorts the lanes in `(RegId, u32)` order, so flag bit `i` is mask bit `i`,
and iteration visits lanes in the order a `BTreeSet<Lane>` would. Each
register records the mask bit of its byte 0 and its byte count, so a lane's
bit is a table lookup.

```mermaid
%%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','textColor':'#64748b'}}}%%
block-beta
  columns 8
  o1["bit 32.."] c1["bytes, one run per register"]:3 s1[" "] n1["in RegId order"]:3
  o2["bit 0..31"] c2["flags (RegId::None, 0..31)"]:3 s2[" "] n2[" "]:3
  classDef off fill:transparent,stroke:transparent,stroke-width:2px,color:#64748b
  classDef cell fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff
  class o1,n1,s1,o2,n2,s2 off
  class c1,c2 cell
```

*Lane mask bits*

Union, difference, intersection, subset and disjointness are single word
operations. Inserting a lane that is none of the 72 panics.

## Other register files
The table above is the x86 one and the default. A file of another target
(arm64's 31 roots and 32 vector registers) has its own, built from the
file's roots: one lane for each root and every view of it (a write of a
`w` register writes the whole `x`, so a finer lane would never be told
apart), the 32 flag lanes, and at most 128 in all. `enter` makes a file's
table the one this thread's `Lanes` read until the guard drops; a mask
belongs to the table it was made under.

## Example
No example: a bit set type. `liveness.rs` shows what it holds.

## Used by
`liveness`, `peephole` and its sub-passes (`copysink`, `postrasink`,
`regthrash`, `exactaddress`, `sharedstores`), `loopslots`,
`schedule`, `parcopy`, and `peep/guards`, `peep/walk`, `peep/make`.
