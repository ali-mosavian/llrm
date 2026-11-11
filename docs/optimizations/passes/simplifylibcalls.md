> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/simplifylibcalls.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/simplifylibcalls.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/simplifylibcalls.rs sha256:ab5dba9491083c62e085022a683054390d1bc636b593c64e2555d043755cce7a -->

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
`strcmp`, `strncmp`, `memcmp` and `bcmp` of constant arrays fold to their
order, `strcmp("", p)` and `strcmp(p, "")` to the first byte,
`memcmp(p, q, 1)` to the byte difference, `memcmp(p, q, n) == 0` to one
integer compare for a legal `n * 8` and aligned pointers; `strcmp(p, "x")`
and `strncmp` of a literal are `memcmp` where the bytes are readable and the
result is only compared with zero, and `memcmp(p, q, n) == 0` is `bcmp`
(`compare.rs`). `strchr` of a constant string and character, `strpbrk`,
`strspn`, `strcspn` and `strstr` over constant strings fold to an address
into the first string, null or a length, and `memchr` of a constant array
or size folds to compares and a select (`search.rs`).

## Deviations
- D1: `puts`, `putchar`, `fwrite`, `fputc`, `fputs`, `memcmp` and `bcmp` are
  formed only where the compile states them in `Options::library`, as
  `strlen` is (loopidiom D11); the pin reads the triple. Exit:
  `Target::library` naming them.
- D2: the other arms (`sprintf`, `snprintf`, the other `str*`, `memchr`, math)
  are untranslated, and so are `iprintf`/`__small_printf`,
  `optimizeErrorReporting` and the argument annotations. The `memcpy`
  family is `memops.rs`.
- D4: the arms of `strchr`, `strpbrk`, `strcspn` and `strstr` that call a
  routine (`memchr`, `strlen`, `strncmp`, `strchr`) are untranslated, as is
  the character compare the pin makes of a `strchr` or `strstr` tested
  against its own string: such a call is left as it is.
- D5: `memchr` of a constant array and size tested against null (the pin's
  bit-field and OR-chain rewrite) is not translated.
- D3: `strcmp` of two strings whose lengths are known but whose contents are
  not (a PHI or select of literals) is not rewritten: `GetStringLength` here
  reads a constant global only.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## Example
```text
call i32 (ptr, ...) @printf(ptr @.str)   ; @.str = c"hi\0A\00", unused
```
becomes `call i32 @puts(ptr @str.0)` with `@str.0 = c"hi\00"`.

## Used by
Named by `aggressive_instcombine`, `pipeline`, `simplifylibcalls_compare`, `simplifylibcalls_search`, `simplifylibcalls_slope`.
