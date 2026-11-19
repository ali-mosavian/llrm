> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/maskbyand.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/maskbyand.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/maskbyand.rs sha256:6dbc582792ddb4219b627bfa86bf6e8e155a20b0fc741cdf2806d9e09cc10f55 -->

# maskbyand

Byte `movzx d, s.low` as `d := and s, mask`, where the CPU prices the
extension above the `and` and the block runs often enough to pay the extra
bytes.

## How it works
The pin's table selects `(and x, 0xff)` as `MOVZX32rr8` with no CPU gate
(X86InstrCompiler.td#L1773-L1777): its CPUs extend in one clock. The 486
and the Pentium take three clocks for it and one for `and r,imm`, which is
two to three bytes longer. Before allocation the block frequency is known
and the `and` is three-address, so TwoAddress and the coalescer place it
without a `mov` where the source dies. The rewrite is made where `frequency
x clocks saved` is at least `rate x bytes added` (the target's speed rate;
none at -O1 and for size).

Only byte sources: a halfword `and` (imm32, 6 B) after a `div` moved the
allocation in nbody_fixed -O3 (+25024 clocks, a loop counter spilled).

The source must have no other reader: the `and` overwrites its register,
and a live source costs a `mov` that eats the clocks saved.

The `and` writes the flags: the rewrite needs a later instruction of the
block that writes them all before any reads one, or the block's end
(flags are not carried across blocks).

DEVIATE MA1 (owner x86-m32; exit: the rule moves into the generated table
when it takes a block-frequency predicate; review 2026-12-10). The
instruction selector cannot see how often a block runs, so the choice
is made here, where cold sites keep the shorter `movzx`.

## Example
`movzx v1, v0.b` in a loop body of frequency 8: `v1 := and v0, 255`.

## Runs
After `IvZero`, at -O2 and -O3.
