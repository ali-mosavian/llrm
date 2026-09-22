# Rust port vs Python: compile time

TL;DR: matmul `--opt` went from 1502 to 907 ms CPU, 4.3x to 7.1x Python. nbody
`--opt` went from 1320 to 515 ms, 6.6x to 16.9x. Every object file is
byte-identical to Python's before and after.

## Method

- Inputs are `bench/c/*.c`. Release build with debug symbols.
- Each cell is the minimum CPU time (user+sys) over 5 runs, with the stages
  interleaved so they share the host's load. Python is the minimum of 2.
- The host was loaded, so CPU ms still drifts by up to 2x between sessions.
  Retired instructions (`/usr/bin/time -l`) do not drift, and every delta
  below was checked against them.
- The ratio is Python CPU ms over Rust CPU ms.

## Stages

| stage | commit | change |
|---|---|---|
| base | 531e5b79 | |
| 1 | febcfd78 | `avail`: update holders in place |
| 4 | a505b94e | Fx hasher for internal maps |
| 5 | 5a42b54d | `symbolic_ref`/`covering` borrow |
| 6 | 643dc5e0 | register lanes as a bitset |
| 4b | 258e408b | `consts._kills` kills cells in place |
| 2a | b7614e7a | passes move or borrow ops they keep |
| 2b | c13382ac | `with_blocks`/`with_ops`/`with_insns` |

## CPU ms (ratio to Python)

| bench | base | 1 | 4 | 5 | 6 | 4b | 2a | 2b | Python |
|---|---|---|---|---|---|---|---|---|---|
| matmul | 45 (6.8x) | 45 | 43 | 43 | 25 | 25 | 24 | 24 (12.5x) | 306 |
| matmul `--opt` | 1502 (4.3x) | 1252 | 1163 | 1138 | 1058 | 1063 | 959 | 907 (7.1x) | 6478 |
| crc | 16 (9.8x) | 16 | 15 | 15 | 11 | 11 | 11 | 11 (14.0x) | 155 |
| crc `--opt` | 60 (5.7x) | 60 | 55 | 54 | 44 | 42 | 41 | 39 (8.7x) | 339 |
| nbody | 52 (6.2x) | 52 | 51 | 51 | 27 | 27 | 27 | 27 (12.2x) | 325 |
| nbody `--opt` | 1320 (6.6x) | 1312 | 944 | 785 | 740 | 532 | 525 | 515 (16.9x) | 8719 |
| sieve | 22 (8.2x) | 22 | 21 | 21 | 14 | 14 | 14 | 14 (13.2x) | 183 |
| sieve `--opt` | 50 (5.9x) | 50 | 45 | 45 | 36 | 36 | 35 | 35 (8.5x) | 295 |

## G instructions retired

| bench | base | 1 | 4 | 5 | 6 | 4b | 2a | 2b | Python |
|---|---|---|---|---|---|---|---|---|---|
| matmul `--opt` | 24.23 | 19.36 | 18.17 | 17.73 | 16.10 | 15.95 | 14.78 | 13.99 | 122.2 |
| nbody `--opt` | 22.05 | 21.81 | 15.87 | 12.98 | 12.40 | 8.72 | 8.56 | 8.33 | 171.0 |
| matmul | 0.73 | 0.73 | 0.71 | 0.71 | 0.36 | 0.36 | 0.36 | 0.36 | 4.3 |

## What remains

Clone, drop and malloc are still about 40% of matmul `--opt` samples.

- The largest single share is whole-body copies behind identity: the
  `consts.known` and `transform.halves` caches, `_Transaction.fixed`, and
  passes that return `body.clone()` unchanged. The `Rc<MirBody>` work
  covers these; it was not touched here.
- After that comes `Op` and `MemRef` sharing (`Rc<Op>` in blocks). It changes
  every pass, so it belongs after the `Rc<MirBody>` merge.
- Strength, Hoist and Gvn are about half of what is left, mostly real work.
  Strength alone is about 27%.
