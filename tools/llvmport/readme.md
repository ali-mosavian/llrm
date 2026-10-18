# llvmport

Golden files for the translations of LLVM `d1106deb71cc` (`ValueTracking`, `KnownBits`, `ConstantRange`, `DominatorTree`).
Each `*_dump.cpp` runs the LLVM operations over every input of small widths (and a fixed sample of wider ones) and prints one hash per operation;
`crates/ir/llrm-mir/src/*_tests.rs` recompute the same hashes from the Rust port, so a difference names the operation and the width.

    tools/llvmport/knownbits_dump.sh     > crates/ir/llrm-mir/src/knownbits_golden.txt
    tools/llvmport/constantrange_dump.sh > crates/ir/llrm-mir/src/constantrange_golden.txt
    tools/llvmport/domtree_dump.sh       > crates/ir/llrm-mir/src/domtree_golden.txt

They build against `$LLVM_BUILD` and `$LLVM_SRC` (checkouts at `d1106deb71cc`; the scripts say what they default to).
Regenerate a golden file only with that build; the Rust side and the C++ side must list operations in the same order.

`constfold_dump.sh FILE.ll` prints what `ConstantFoldInstruction` makes of each instruction whose operands are all constants; `cfcompare.py PATH...` runs it beside `cargo run --release -p llrm-mir --example cfdump` and names every line that differs (`--write-expected` writes the `tests/cf/fold/NAME.expected` and `.deviates` that `constantfolding_tests` reads).
