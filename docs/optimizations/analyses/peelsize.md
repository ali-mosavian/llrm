> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/peelsize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/peelsize.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/peelsize.rs sha256:f88379226067acc68e7d43b15333d27fa5763f2d4b85287fa61eb1f2c175efa9 -->

# peelsize

The budgets the mid-end's passes read: `Limits`.

The GCC-model `admitted` (is a complete copy of a counted loop worth it) was
deleted with `Options::peel`, `Limits::grows` and the trip and operation
caps only it read: nothing called it outside its tests, and the translated
`LoopUnrollPass` decides the copies.

## Example
`Limits::default()` is the budget the passes read when no option sets one.

```text
Limits::default()
    => { target_percent: 0, milliclocks_per_byte: 16_000 }
       (an inline that grows the code must save 16 clocks per byte)
```

## Used by
Named by `pipeline`, `target`.
