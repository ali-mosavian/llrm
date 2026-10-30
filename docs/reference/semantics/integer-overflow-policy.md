# Integer overflow, divide and wrap policy

For users choosing arithmetic policy and for frontend authors. Each policy is a gcc option; a frontend marks the semantics it implies.

| Option | Meaning | Where it lands |
|---|---|---|
| `-fwrapv` | signed arithmetic wraps; the frontend states no no-overflow promise | C: `Unit::wrapv` ([`translate.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/frontends/llrm-c/src/translate.rs#L2562)) decides whether a signed add or multiply of 16 bits or more carries `nsw` |
| `-ftrapv`, `-fsanitize=signed-integer-overflow` | signed overflow raises | QB: `checked_overflow`, BC's `/D` check |
| `-fsanitize=integer-divide-by-zero` | division by zero raises in code | QB: `checked_division` |
| `-fsanitize=stack` | procedure entry compares SP with the runtime's limit | `checked_stack` |
| `-fsanitize=undefined` | bounds, divide and overflow together | not `stack` |

`-fsanitize` is parsed in [`flags.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/backend/llrm-core/src/driver/flags.rs#L96). With `checked_overflow`, qbfront emits the error-6 raise for INTEGER and LONG `+ - *`, negation, and LONG to INTEGER narrowing ([`semantic.rs`](https://github.com/ali-mosavian/llrm/blob/c341bd6e3de058142c6652d399b8dfeb444eff9b/crates/frontends/qb/src/semantic.rs#L7998)); without it the result wraps at its width. `checked_division` raises error 11 in code rather than through the processor's divide trap, whose statement the frontend cannot name.

A C frontend that sees no `-fwrapv` marks signed operations `nsw`, and `ranges`, `indvars` and `algebraic` may assume they do not wrap. A checked or wrapping operation carries no such mark, so no pass assumes more than the width allows.
