> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/storecombine.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/storecombine.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/storecombine.rs sha256:f0c97c74c28a0324bd70b4d7f25d3f926008829e255c558e3f8c6effd40e380d -->

# storecombine

Packs two neighbouring literal word stores into one dword store.

## How it works
A store qualifies when it is `mov word [disp], imm16` to a fixed segment
address (no base, index or selector; `disp` in `0..=0xfffe`), is not
volatile and carries no clobbers, requirements, delivered or defined values,
and no relocation. When it is followed, past empty anchors, by such a store
to `disp + 2`, the pair becomes one `mov dword [disp], imm32` with the low
word first (little endian) and the second store becomes an empty anchor so
its address bookkeeping is kept.

```text
 mov word [0x40],1                 mov dword [0x40],0x00020001
 mov word [0x42],2        ==>      (anchor)
```

Anything else between the two ends the pair: a read, a call or an address
that may differ. The low store's `disp` must be at most 0xfffc.

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), once per body, after
`postrasink` and before `pushed_constants`. It has no level switch.
