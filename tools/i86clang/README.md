# i86clang

Clang's C and C++ front end for llrm's 16-bit x86 target (`m16`), with Borland's memory models. The unoptimised LLVM IR goes to
stdout; `llrm-llc` optimises and selects it.

```
tools/i86clang/build.sh                       # needs LLVM with libclang-cpp (llvm-config on PATH or LLVM_BIN)
clang -E -P -x c --target=msp430 f.c | python3 -c 'import borland,sys; print(borland.rewrite(sys.stdin.read()))' > f.i.c
tools/i86clang/build/i86clang -model=medium f.i.c -- --target=msp430 -std=gnu89 | llrm-llc -O2 -o f.obj /dev/stdin
python3 -m unittest discover -s tools/i86clang/tests
```

`borland.py` turns Borland's `far`, `near`, `huge`, `pascal` and inline assembly into what clang parses.

## Models

| `-model=` | code | data |
|---|---|---|
| `small` | near | near |
| `medium` (default) | far | near |
| `compact` | near | far |
| `large`, `huge` | far | far |

The model sets the default; `near`, `far` and `huge` on a data pointer override it. A function pointer follows the code distance unless
marked; `far` is always far, `near` always near (there is no `huge` function pointer). Names and argument passing come from the function's calling convention (`calling.toml`), not from this tool.

## How

- Far code is the datalayout's program address space (`P1`): clang puts every function and function pointer in address space 1.
- A default pointer is as wide as code is far, so data pointers are spelled with an address space (near 0, far 1, huge 3) by a
  first parse that writes the model's default before each unmarked `*` and splits declaration lists around them.
- Far arithmetic changes the offset, `<` compares offsets, `==` both words, a huge pointer carries into its segment and a huge
  difference is `llrm.ia16.ptrdiff`; all of that is llrm's, from the address space.
- Pascal is clang's `stdcall`, remapped to llrm's convention 1000.

## Limits

- `near` functions inside a far-code program are far.
- Models are m16 only; in m32 and m64 every pointer is native size and `near`/`far`/`huge` mean nothing.
- A far-data model keeps objects in DGROUP; only pointers are far.
- `ptrdiff_t` is `int` below compact and `long` from compact up; Borland also types `far - far` as `long` below that.
- `-wide-size` makes `size_t` 32 bits, which a `huge` array over 64K needs; Borland's stays 16.
- `char const *a, *b` as a list loses the trailing `const` when split.
