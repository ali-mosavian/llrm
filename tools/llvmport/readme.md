# llvmport

Golden files for the translations of LLVM `d1106deb71cc` (`ValueTracking`, `KnownBits`, `ConstantRange`).
Each `*_dump.cpp` runs the LLVM operations over every input of small widths (and a fixed sample of wider ones) and prints one hash per operation;
`crates/ir/llrm-mir/src/*_tests.rs` recompute the same hashes from the Rust port, so a difference names the operation and the width.

    tools/llvmport/knownbits_dump.sh     > crates/ir/llrm-mir/src/knownbits_golden.txt
    tools/llvmport/constantrange_dump.sh > crates/ir/llrm-mir/src/constantrange_golden.txt

They build against `$LLVM_BUILD` (default `~/scratch/llvm-build`) and `$LLVM_SRC` (default `~/scratch/llvm-src`), both at `d1106deb71cc`.
Regenerate a golden file only with that build; the Rust side and the C++ side must list operations in the same order.
