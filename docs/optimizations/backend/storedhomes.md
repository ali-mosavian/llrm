> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/storedhomes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/storedhomes.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/storedhomes.rs sha256:bc79ee64461f5b4605c44e0af7b472eea93c7849d53b4e992a37bb4749cfacc1 -->

# storedhomes

The cells the optimizer proved hold a phi's value (`!llrm.home`,
`LirBody.homes`), as the body now stands: the one fact both spillers ask. A
spilled value read from its cell needs no store of its own and no cell of
its own.

The proof is the optimizer's, over MIR. What is checked here is what a later
pass may have changed: nothing writes the cell while the value lives, and
every definition of the value is a plain copy, which spilling removes.
