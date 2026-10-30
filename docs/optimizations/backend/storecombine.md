> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/storecombine.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/storecombine.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/storecombine.rs sha256:249ba8089a72a0a0c094a954ebcbbc2caa1d209a4c38e8fe5b010774b5f13e42 -->

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

## Example
Two literal word stores to adjacent fixed cells, `x` and `x+2`, with an
empty anchor between them.

Schematic, from the fixture of test
`test_only_adjacent_unobserved_static_literals_are_packed` (not printer
output):
```lir
before:  mov word [x], -1           ; low
         (anchor)
         mov word [x+2], 2          ; high
after:   mov dword [x], 0x2ffff
         (anchor)
         (anchor)
```

The two words became one dword store, low word first, and the second store
stayed as an anchor so the instruction spans are unchanged.

From unit test `test_only_adjacent_unobserved_static_literals_are_packed` in
`storecombine.rs`.

## Runs
A sub-pass of [`crate::backend::peephole`](peephole.md), once per body, after
`postrasink` and before `pushed_constants`. It has no level switch.
