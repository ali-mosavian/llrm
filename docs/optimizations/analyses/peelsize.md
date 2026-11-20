> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/peelsize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/peelsize.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/peelsize.rs sha256:e7cf028afa69399df60311d547733a5ed97da8cfa0fac9b91b0c7a6d78df902e -->

# peelsize

The budgets the mid-end's passes read: `Limits`.

The GCC-model `admitted` (is a complete copy of a counted loop worth it) was
deleted with `Options::peel`, `Limits::grows` and the trip and operation caps
only it read: nothing called it outside its tests, and the translated
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
