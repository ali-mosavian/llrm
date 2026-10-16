> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/schedule.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/schedule.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/schedule.rs sha256:0c5ba7940cbb38eb9ebbb65583b5380747f6bb8dbdee094f2cebe00300a2027f -->

# schedule

Reorders independent register-and-immediate instructions after allocation so
a consumer does not wait on its producer's latency.

## How it works
The scheduler is narrower than a flat-32-bit compiler's. The 16-bit output
has segment state, far calls, source-map anchors and precise x87 exception
ordering, none with a complete dependency model, so each is a boundary. Only
the integer instructions `_safe` admits move: register and immediate
operands (or the address of a frame cell), a move, binary, unary, multiply,
compare, extend, funnel shift or address operation, with no memory operand,
no fixed register or clobber, no group, no spill or frame-adjust marker and
no barrier. For these the physical register lanes and flag lanes are the
whole dependency graph.

Each block is cut at the instructions that are not safe into windows of safe
ones. A window's dependency graph (`_graph`) has an edge from an earlier to
a later instruction for read-after-write, write-after-read and
write-after-write on any lane, flags included, in source order. A window is
then list-scheduled:

```text
 mov eax, [..]   (boundary)           window: imul ebx, ecx   latency 9
 imul ebx, ecx                                add  edx, 1
 add  ebx, 3     needs imul                   add  ebx, 3     needs imul
 add  edx, 1                          ordered: imul, add edx, add ebx
```

Each step picks, among instructions whose inputs are ready by the current
clock, the one with the longest latency (`_latency`, the CPU's `latency` for
the instruction's form), ties in source order. A consumer becomes ready at
its producer's finish plus `_partial_merge_delay`: the CPU's
partial-register stall where a 32-bit read follows a 16- or 8-bit write of
the same root with no full write between.

A CPU with `pentium_pairing` uses `_pentium_ordered` instead: each cycle it
issues one instruction and, where one is ready, a second that pairs with it.
`_pair_class` gives the form's category by gcc's Pentium description: `u`
for forms with an operand or address-size prefix or a shift, `np` for
immediate forms, multiplies and funnel shifts, `uv` otherwise. A U-only or
`uv` form starts a pair when a `uv` form is also ready, and a `uv` form
fills the V slot.

## Example
On a Pentium, `shr` needs the `mov` before it and cannot pair with it; the
`lea` is independent. Moved up beside the `mov`, it fills the V slot:

```lir
═══ _fill_local@3 · LoopSlots · m16 ════════════════════════════════════════════
B2  ← B0  → B1                                                    ×0.8
    #18  bx         = lea.i16     &+eax+eax*1
    #17  cx         = mov.i16     bx
     #8  cx         = shr.i16     cx, 2:i8
     #9  bx         = and.i16     bx, 3
    #11  di         = lea.i16     &bp-16

═══ _fill_local@3 · Scheduler · m16 ════════════════════════════════════════════
B2  ← B0  → B1                                                    ×0.8
    #18  bx         = lea.i16     &+eax+eax*1
    #17  cx         = mov.i16     bx
    #11  di         = lea.i16     &bp-16
     #8  cx         = shr.i16     cx, 2:i8
     #9  bx         = and.i16     bx, 3
```

From `ISEL_DUMP=1` of `fill_local` in tests/fixtures/c/fill.c with
`-m16 -O2 -march=pentium -mtune=pentium`, stages `LoopSlots`, `Scheduler`.

## Runs
Once per body, the machine phase after `loopslots` and before `jumps`
(`flow::machine_with`), in every machine at every -O level. A body is
returned unchanged, sharing its instructions, when nothing moved, and for an
in-order profile without Pentium pairing (the 386 and 486).

## Needs
Register-lane effects from [`crate::backend::peephole`](peephole.md)
(`_register_effects`) and the CPU profile ([`crate::backend::cpu`](cpu.md)):
`latency`, `in_order`, `partial_register_stall`, `pentium_pairing`.

## See also
- gcc `-fschedule-insns2`, which runs after register allocation as this
  does:
  <https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/opts.cc#L642>
