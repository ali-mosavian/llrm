> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/profit.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/profit.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/profit.rs sha256:32f76362a77fd649bbb85229f3197d2c82befd208875aa2b372822905a9a79f0 -->

# profit

Adapted from llrm-core's `optimize/profit.rs`, the port of
`qbopt/optimize/profit.py`: machine-neutral profitability shared by MIR
transforms.

Everything here prices semantic work only: opcodes, memory effects and
CFG frequency.  A kind without a price makes the answer unknown rather
than cheap.

The machine's facts come in as parameters: each operation's price
(`OperationCosts`, from the target: `costs`), how many integer values fit in
registers (`capacity`), and that floating values do not take that room --
the old MIR's x87 width, here a floating type.

What changed with the IR: only a load or store touches memory, so no
operation carries a folded memory operand and `memory_update` prices
nothing; `llvm.memset` is the old fill, its bytes the cells; `select`,
`frem` and `landingpad` had no old kind and stay unpriced. `spill_risk`
is the spill model's (`spill`) over the whole body. `trips` is induction's
proven counts (`proven_trips`); a loop it does not name gets the
conventional ten.
