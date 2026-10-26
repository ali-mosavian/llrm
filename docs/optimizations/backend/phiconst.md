> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/phiconst.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/phiconst.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/phiconst.rs sha256:50df919002ef90de54243b22fe0c856fcae5ca3e056454b53bc41a4425881940 -->

# phiconst

One value for a constant that several edges of a phi carry.

## How it works
A phi that takes the same constant from several predecessors, each making
it for itself (`v7 := mov 0` in one block, `v8 := mov 0` in another),
takes it from one value instead: made once, at the end of the nearest block
that dominates those predecessors. Nothing else changes; the phi's other
edges keep their values. This is what a source variable's constant is in
gcc's SSA, one name used by every phi it reaches, and it is what lets the
web of the phi, where it lives in a stack slot or one register, be written
at that definition and in the arm that assigns, and not on each edge of the
join: D_DRAW_FACES -Os lines 562-567 (six `lm_*` shorts zeroed at the top
of a loop and assigned under nested conditions) were stored on four edges.

The constant is made there when the blocks it replaces together run at
least as often as that block (every path from there to the phi reads it: a
move on a path that does not holds a register for nothing, in bytes as in
clocks; priced by bytes alone, QCport -Os grew 630 B, 29 objects), when the
phi's own block does not dominate it (a loop's header would make it after
the phi), and for no relocatable constant and no x87 value.

```text
  d:  ...                         d:  ...
      br c, p1, p2        ==>         h := mov 0
  p1: v1 := mov 0                     br c, p1, p2
  p2: v2 := mov 0                 p1: ...                p2: ...
  j:  r = phi [p1: v1, p2: v2]    j:  r = phi [p1: h, p2: h]
```

## Example
`x = 0; ... if (a) x = g[1]; ... use(x)` in a loop: the zero is made in the
block under each test that does not assign, `x`'s phi takes three zeros and
`g[1]`. After the pass the three zeros are one `v9`, made in the block above
the tests, and the allocator holds `x` in one register or one slot, written
there and in the arm.

```text
  before                               after
  top:   v1 := mov 0                   top:   v9 := mov 0
         br a, t1, j                          br a, t1, j
  t1:    v2 := mov 0                   t1:    br c, t2, j
         br c, t2, j                   t2:    v3 := load g+2
  t2:    v3 := load g+2                       br j
         br j                          j:     x = phi [top: v9, t1: v9, t2: v3]
  j:     x = phi [top: v1, t1: v2, t2: v3]
```

## Runs
Phase 1 of the machine phases, before `SsaSpill`, so both allocation routes
take it.
