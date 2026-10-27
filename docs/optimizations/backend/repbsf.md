> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/repbsf.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/repbsf.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/repbsf.rs sha256:60785564e75e6a40578dd92b39693c00780495fb83cb41e45363ecff758d8835 -->

# repbsf

`bsf` gets the `rep` prefix (the TZCNT encoding, which processors without
`tzcnt` run as the same `bsf`) unless the function is optimised for size or
the instruction's flags are read.

LLVM's `X86MCInstLower` `BSF*` case sets `IP_HAS_REPEAT` when
`!hasOptSize() && EFLAGS is dead`
([L516](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86MCInstLower.cpp#L516)).
`Profile::size` stands for `hasOptSize`.

## Example
With the flags dead and the function built for speed:

```text
 before                after
 bsf eax, ecx          rep bsf eax, ecx     (tzcnt's encoding)
 cmp esi, 0            cmp esi, 0
```

Built for size, or with the flags read (`adc` next), the `bsf` is left as it
is. From unit test
`test_bsf_gets_the_rep_prefix_only_where_the_pin_gives_it`
in `peephole_tests.rs`, which runs the three cases.

## Used by
Named by `peephole`.
