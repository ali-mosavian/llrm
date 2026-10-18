> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/needs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/needs.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/needs.rs sha256:2fee09db16b15adf02c5283adc54b59e3c0ba149323d613574d8d745b961ebfe -->

# needs

What a body contains that a peephole sub-pass could act on, so that
`peephole` skips a sub-pass whose subject is absent.

## How it works
Each instruction has a set of bits, and a body has the union of its
instructions'. A sub-pass names the bits it needs; `gated` runs it only if
the body has all of them. The shape is that of the MIR passes' `Depends`,
for the machine passes.

```text
  COPY        register-to-register mov       regthrash, machinecp
  INSERTED    inserted instruction defining  secondary_bases
  UNHELD      cell read via an unheld reg32  upperzero
  EXACT_CELL  a cell lowering proved exact   exactaddress
  BASED       a cell addressed via a value   secondary_bases
```

A bit is the sub-pass's own candidate test (`regthrash::_plain_copy`,
`upperzero::unheld`, `exactaddress::cell_of`), so "can this pass match" has
one definition. `Contents` keeps the bits of each block for as long as the
block is the same list of instructions (`Insns::same_as`), so a pass that
changes one block recomputes one block.

`LLRM_CHECK_NEEDS` runs a skipped pass anyway and asserts it changed
nothing.

## Example
No honest example: the bits steer which sub-pass of `peephole` runs and
change no instruction. `LLRM_CHECK_NEEDS` checks a skipped pass against its
bits.

## Used by
`peephole`.
