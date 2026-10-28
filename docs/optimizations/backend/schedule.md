> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/schedule.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/schedule.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/schedule.rs sha256:4f9f0fce3d7060dffa3ab92e58f77c9753b871c999eb2f105cb6b521c17a0b9c -->

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
`imul` has a long latency and `add` needs its result; the `mov` is
independent. The addresses on the left are the original positions:

```lir
=== before
    block 0x0000
      0x0000  EAX := imul EAX, ECX
      0x0001  EAX := add EAX, EDX
      0x0002  EBX := mov ESI
=== after
    block 0x0000
      0x0000  EAX := imul EAX, ECX
      0x0002  EBX := mov ESI
      0x0001  EAX := add EAX, EDX
```

The `mov` now runs while the multiply finishes.

From unit test `test_out_of_order_profile_fills_an_imul_dependency_gap` in
`schedule_tests.rs` (P6 profile).

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
