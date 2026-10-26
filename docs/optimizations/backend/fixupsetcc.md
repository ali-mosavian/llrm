> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/fixupsetcc.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/fixupsetcc.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/fixupsetcc.rs sha256:0fa6b132fdac8365d8a15760f26f183d00c37fbb5ea46ba6ba2921c09e7af062 -->

# fixupsetcc

`setcc` into a register zeroed before the compare, not a `movzx` of a byte
after it: `X86FixupSetCC`. Translated from
[X86FixupSetCC.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupSetCC.cpp#L68)
(`fixupSetCC`).

## How it works
In each block the latest instruction that writes the flags is remembered.
A `setcc` after one, whose byte is read by one `movzx` and by nothing else,
becomes: a zero made just before the flag-writing instruction (a `mov` of 0,
`xor` after allocation), and the `setcc` writing the low byte of that
register; the `movzx` goes and its readers read the register. The pin
refuses when the flag-writing instruction also reads the flags (the zero
would have to clobber them there, which the `xor` does).

`xor eax,eax; cmp; setl al` is shorter than `cmp; setl al; movzx eax,al`
and has no partial-register write to stall on.

DEVIATE F1 (no number yet): the pin keeps the `setcc` result and inserts it
into the zeroed register as a sub-register (`INSERT_SUBREG`), which its
coalescer then joins. LIR has no sub-register insert: the `setcc` here
names the register's own value as written and read (one value defined
twice, as phi elimination leaves them), so the allocator gives both the
same register. Only a byte read once by the `movzx` is rewritten. Exit:
sub-register values in LIR.

## Example
`int f(int a, int b) { return a < b; }` at `-m32`, `ISEL_DUMP=1`.
Before:

```lir
v1 := mov [[bp+0x8]]
cmp v1, [[bp+0xc]]
v3 := setl
v4 := movzx v3
```

After:

```lir
v5 := mov 0
v1 := mov [[bp+0x8]]
cmp v1, [[bp+0xc]]
v5 := setl          ; the low byte of v5
```

## Runs
After `MachineScheduler`, before the float phases. Optimizing levels only.
