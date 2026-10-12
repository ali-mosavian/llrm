> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/upperzero.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/upperzero.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/upperzero.rs sha256:50f82e5e58d0788d6e9b9facecf113649a068f26fdd06db05ac6cfa7e229713c -->

# upperzero

Which 32-bit register roots hold zero in their upper half, and the `movzx`
that makes it so where a 32-bit address needs it.

## How it works
In 16-bit code a 32-bit effective address through a register is the prefixed
form of the 16-bit one. It names the same byte only if the register's upper
half is zero. A forward dataflow over the allocated body keeps, before each
instruction, a set of roots (one bit each) known to be zero above bit 15.

```text
 entry            nothing known (empty set)
 meet             intersection over predecessors; unreached blocks are top
 movzx r32,r/m    sets r       xor r,r / sub r,r        sets r
 mov r32,imm      sets r when imm is 0..0xFFFF and has no relocation
 and r32,imm      sets r when imm is 0..0xFFFF and the source is r
 mov a,b          a is zero when b is
 xchg a,b         swaps the two bits
 word/byte write  keeps the bit; any write of lanes 2 or 3 clears it
 unknown effects  clear every bit (jumps and branches write nothing)
```

`established` makes the facts true where the code needs them. A cell that
reads a root through a register holding no value (the frame pointer) needs
that root's upper half zero. Where the analysis does not prove it, the pass
inserts `movzx r32,r16`: in the preheader of the outermost loop around the
cell in which nothing disturbs the root, when the loop's entry is one block
with one successor, or else before the first such cell in the block. It
repeats until no cell lacks the fact.

## Example
`mov ecx, 2; xchg esi, ecx; nop` in one block, nothing known on entry:

```lir
mov ecx, 2          ; ecx is zero above bit 15
xchg esi, ecx       ; the two bits swap: esi zero, ecx not
nop                 ; before here: esi in the set, ecx not
```

The exchange moves the zero upper half from `ecx` to `esi`, so a `movzx` of
`esi` before it indexes would be redundant.

From unit test `test_an_exchange_swaps_the_zero_upper_halves` in
`upperzero.rs`.

## Provides
`before`, the zero roots before each instruction, keyed by instruction
identity. `preheaded` places `movzx` in loop preheaders for roots a caller
asks for.

## Runs
`established` is the last sub-pass of [`crate::backend::peephole`](peephole.md), run when
the body has such a cell (`needs::UNHELD`) and not on a native target, whose
addresses read whole registers.

## Used by
[`crate::backend::exactaddress`](exactaddress.md) (`before`, `preheaded`), and the rewrite
guards of the peephole (`upper_zero`).
