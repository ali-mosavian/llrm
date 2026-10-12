# Semantics

How a language's semantics reach the optimizer and what the user selects. For anyone running the compilers or writing a frontend.

| Doc | Holds |
|---|---|
| [switches](switches.md) | Multiway branches: MIR `switch` and BASIC's `ON GOTO` dispatch |
| [bounds-checks](bounds-checks.md) | Subscript checks: `-fsanitize=bounds` and the `B$HARY` call |
| [integer-overflow-policy](integer-overflow-policy.md) | Overflow, divide and wrap policy: `-ftrapv`, `-fwrapv`, `-fsanitize` |
| [floating-environment](floating-environment.md) | What the compiler assumes of the floating-point environment |
| [native-fpu-waits](native-fpu-waits.md) | x87 `wait` around integer conversions in BASIC modules |
| [aggregate-arguments](aggregate-arguments.md) | Aggregates passed by value |
