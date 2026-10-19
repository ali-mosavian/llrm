# llvmport

Golden files for the translations of LLVM `d1106deb71cc` (`ValueTracking`, `KnownBits`, `ConstantRange`, `DominatorTree`, `APFloat`, `FPClassTest`, `KnownFPClass`, `ConstantFPRange`).
Each `*_dump.cpp` runs the LLVM operations over every input of small widths (and a fixed sample of wider ones) and prints one hash per operation;
`crates/ir/llrm-mir/src/*_tests.rs` recompute the same hashes from the Rust port, so a difference names the operation and the width.

    tools/llvmport/knownbits_dump.sh     > crates/ir/llrm-mir/src/knownbits_golden.txt
    tools/llvmport/constantrange_dump.sh > crates/ir/llrm-mir/src/constantrange_golden.txt
    tools/llvmport/apfloat_dump.sh       > crates/ir/llrm-mir/src/apfloat_golden.txt
    tools/llvmport/fpclass_dump.sh       > crates/ir/llrm-mir/src/fpclass_golden.txt
    tools/llvmport/knownfpclass_dump.sh  > crates/ir/llrm-mir/src/knownfpclass_golden.txt
    tools/llvmport/constantfprange_dump.sh > crates/ir/llrm-mir/src/constantfprange_golden.txt
    tools/llvmport/domtree_dump.sh       > crates/ir/llrm-mir/src/domtree_golden.txt
    tools/llvmport/idf_dump.sh           > crates/ir/llrm-mir/src/idf_golden.txt
    tools/llvmport/blockfrequency_dump.sh > crates/ir/llrm-mir/src/blockfrequency/golden.txt

They build against `$LLVM_BUILD` and `$LLVM_SRC` (checkouts at `d1106deb71cc`; the scripts say what they default to).
Regenerate a golden file only with that build; the Rust side and the C++ side must list operations in the same order.

`lvi_dump.sh` and `lvicompare.py` compare LazyValueInfo's answers (`lvi_dump.cpp` against the `lvidump` example of llrm-mir) on any `.ll`:

    tools/llvmport/lvicompare.py $LLVM_SRC/llvm/test/Transforms/CorrelatedValuePropagation

A line where llrm is more precise than LLVM is MORE (a bug); less precise is LESS and must trace to a DEVIATE of `lazyvalueinfo.rs`.
`crates/ir/llrm-mir/tests/lvi/cases` holds the inputs with LLVM's answers pasted (`NAME.expected`, from `lvi_dump.sh NAME.ll`).

`constfold_dump.sh FILE.ll` prints what `ConstantFoldInstruction` makes of each instruction whose operands are all constants; `cfcompare.py PATH...` runs it beside `cargo run --release -p llrm-mir --example cfdump` and names every line that differs (`--write-expected` writes the `tests/cf/fold/NAME.expected` and `.deviates` that `constantfolding_tests` reads).
