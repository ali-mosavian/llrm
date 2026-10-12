> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/needs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/needs.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/needs.rs sha256:5aa9dbeec07ca73c6541b0808c6b907ab816b0cbbb1148e198a61120cdede8b0 -->

# needs

What a body contains that a peephole sub-pass could act on, so that the
driver skips a pass whose subject is absent: the shape of the MIR passes'
`Depends`, for the machine passes.

A bit is the pass's own candidate test (`regthrash::_plain_copy`,
`upperzero::unheld`, `exactaddress::cell_of`), so the question "can this
pass match" has one definition. The bits of a block are kept for as long as
the block is the same list of instructions (`Insns::same_as`), as its other
facts are.
`LLRM_CHECK_NEEDS` runs a skipped pass anyway and asserts it changed
nothing.
