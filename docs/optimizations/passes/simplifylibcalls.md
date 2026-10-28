> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/simplifylibcalls.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/simplifylibcalls.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/simplifylibcalls.rs sha256:7f0cc85ff49fb330e267c13409a2d3434014556f9bbc9c3796a5228bc7476643 -->

# simplifylibcalls

`LibCallSimplifier`: a call of a C library function that a cheaper one
does as well becomes that one, as `printf("hi\n")` becomes `puts("hi")`.

## How it works
A translation of the `printf`, `fprintf`, `puts`, `fputs` and `fwrite` arms
of LLVM's
[`SimplifyLibCalls.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/SimplifyLibCalls.cpp)
at d1106deb71cc: `optimizePrintFString` (:3466), `optimizeFPrintFString`
(:3887), `optimizeFWrite` (:3923), `optimizeFPuts` (:3950) and
`optimizePuts` (:3978).
A direct call whose callee is a library function by name and prototype
(`TargetLibraryInfo::get_lib_func_of`) is rewritten when its format is a
constant string: an empty format is dropped, a one-character one becomes
`putchar`, `"%s"` of a constant, `"%c"`, `"%s\n"` and a format ending in
`\n` without `%` become `putchar` or `puts` (the new string goes in a
private `str.N` table). `fprintf` of a format without `%` is `fwrite`,
of `"%c"` `fputc`, of `"%s"` `fputs`; `fputs` of a constant string is
`fwrite` (not at `optsize`); `fwrite` of one byte is `fputc`, of none is
0. All but the empty format and the empty `fwrite` need the result unused.

## Deviations
- D1: `puts`, `putchar`, `fwrite`, `fputc` and `fputs` are formed only where the compile states them
  in `Options::library`, as `strlen` is (loopidiom D11); the pin reads the
  triple. Exit: `Target::library` naming them.
- D2: the other arms (`sprintf`, `snprintf`, `str*`, `mem*`, math) are
  untranslated, and so are `iprintf`/`__small_printf`, `optimizeErrorReporting`
  and the argument annotations.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## Example
```text
call i32 (ptr, ...) @printf(ptr @.str)   ; @.str = c"hi\0A\00", unused
```
becomes `call i32 @puts(ptr @str.0)` with `@str.0 = c"hi\00"`.

## Used by
Named by `pipeline`.
