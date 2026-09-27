clang 20's `-O1` output for msp430 from `bench/parity/*.c`, less `target
triple` and `dso_local`, which MIR's subset omits: real LLVM text for the
parser and printer. Regenerate with `tools/mir-clang-fixtures.sh`.
