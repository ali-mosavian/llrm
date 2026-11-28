> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/fixupsetcc.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/fixupsetcc.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/fixupsetcc.rs sha256:f684c8d43134d72896ceba25073eee9e56c07246a15e2d94fd930b72449b726e -->

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

DEVIATE F2 (owner x87, exit 2026-11-29): at -O1, -Os and -Oz the pair after
an x87 compare stays when its result is required in the register the status
word takes (`fnstsw ax`: a return value at m32). `fnstsw ax` holds EAX, so
the zero lands in another register and a copy follows: `xor ecx,ecx; fcomp;
fnstsw ax; sahf; seta cl; mov eax,ecx` (7 B) for `seta al; movzx eax,al` (6
B), and the register it took is clobbered for the callers of a
private-convention function: tests/run/c/fcmp_all -Os m32 5436 -> 5564 B
(+128) since the compare writes the flags (#8200); -O1 grew 5883 -> 6011 B
alike. Any other result keeps the pin's form (`xor; seta`, 1 B less). -O2
and -O3, which price bytes against clocks, keep the pin's form.

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
