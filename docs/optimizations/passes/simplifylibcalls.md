> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/simplifylibcalls.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/simplifylibcalls.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/simplifylibcalls.rs sha256:a4516b8b1d66d4d4a3740155e4754b2f21500f125cf44830ac92e5a9637b1008 -->

# simplifylibcalls

`LibCallSimplifier`: a call of a C library function that a cheaper one
does as well becomes that one, as `printf("hi\n")` becomes `puts("hi")`.

## How it works
A translation of the `printf` and `puts` arms of LLVM's
[`SimplifyLibCalls.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/SimplifyLibCalls.cpp)
at d1106deb71cc: `optimizePrintFString` (:3466) and `optimizePuts` (:3978).
A direct call whose callee is a library function by name and prototype
(`TargetLibraryInfo::get_lib_func_of`) is rewritten when its format is a
constant string: an empty format is dropped, a one-character one becomes
`putchar`, `"%s"` of a constant, `"%c"`, `"%s\n"` and a format ending in
`\n` without `%` become `putchar` or `puts` (the new string goes in a
private `str.N` table). All but the empty format need the result unused.

## Deviations
- D1: `puts` and `putchar` are formed only where the compile states them
  in `Options::library`, as `strlen` is (loopidiom D11); the pin reads the
  triple. Exit: `Target::library` naming them.
- D2: the other arms (`fprintf`, `fputs`, `fwrite`, `sprintf`, `str*`,
  `mem*`, math) are untranslated, and so are `iprintf`/`__small_printf`.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## Example
```text
call i32 (ptr, ...) @printf(ptr @.str)   ; @.str = c"hi\0A\00", unused
```
becomes `call i32 @puts(ptr @str.0)` with `@str.0 = c"hi\00"`.
