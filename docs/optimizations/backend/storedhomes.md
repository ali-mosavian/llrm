> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/storedhomes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/storedhomes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/storedhomes.rs sha256:450d6e067195a0a603497c0149ff398d5f127ffbd163ec50f6f814d5ac4e77d1 -->

# storedhomes

The cells the optimizer proved hold a phi's value (`!llrm.home`,
`LirBody.homes`), as the body now stands: the one fact both spillers ask. A
spilled value read from its cell needs no store of its own and no cell of
its own.

The proof is the optimizer's, over MIR. What is checked here is what a later
pass may have changed: nothing writes the cell while the value lives, and
every definition of the value is a plain copy, which spilling removes.
