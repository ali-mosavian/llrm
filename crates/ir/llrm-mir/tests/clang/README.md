# Clang fixtures

Audience: contributors to `llrm-mir`. Real LLVM text for the MIR parser and
printer tests: clang's `-O1` output for msp430 from `bench/parity/*/*.c`, less
`target triple` and `dso_local`, which MIR's subset omits. Regenerate with
`tools/mir-clang-fixtures.sh`, which uses the `clang` on `PATH`.
