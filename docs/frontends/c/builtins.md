# GNU C builtins in llrm-c

Audience: contributors to `llrm-c`. How a `__builtin_*` call of valid GNU C compiles, from the one table in
`crates/frontends/llrm-c/src/builtins.rs`. Open Watcom knows none of them, so a call used to be a call of an undefined
extern and the program did not link; a builtin the table lacks is now refused with its name.

## Lowerings

| builtin | lowering | how | what clang does (intent) | state |
|---|---|---|---|---|
| `expect` | Identity | at -O1 and up the `llvm.expect` intrinsic (translate.rs), below its argument; `lower-expect` (LowerExpectIntrinsic.cpp) makes branch weights of it | `llvm.expect` at -O1 and up, the argument at -O0 | translated |
| `constant_p` | Constant(0) | `#define` in the predefined header (predefined.rs) | `llvm.is.constant`, folded by LowerConstantIntrinsics after inlining | DEVIATE B1 |
| `unreachable` | Ignored | `#define` to `((void)0)` | an `unreachable` instruction | DEVIATE B2 |
| `trap` | Libc(abort) | `#define` to `abort` | `llvm.trap`, `ud2` | DEVIATE B3 |
| `prefetch` | Ignored | `#define` to `((void)0)` | `llvm.prefetch`, which a target with no prefetch instruction (i486) drops | same result |
| `abs labs memcpy memmove memset memcmp strlen strcmp strncmp strcpy strncpy strcat strchr strrchr strstr` | Libc | `#define` to the library routine | `llvm.memcpy`/`memset`/`memmove`, libcalls for the rest | DEVIATE B4 |
| `popcount clz ctz bswap* alloca` and the rest | refused | "unsupported builtin" at translation | `llvm.ctpop`/`ctlz`/`cttz`/`bswap` (in MIR already), `alloca` | MISSING |

`Constant`, `Ignored` and `Libc` are `#define`s of the predefined header, so the compiler proper never sees them; `Identity`
is translated at the call. Permalinks: LowerExpectIntrinsic.cpp and LowerConstantIntrinsics.cpp at
[d1106deb71cc](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/); clang's CGBuiltin.cpp is not in
the pin's tree, so the intent column is stated from LangRef's intrinsic semantics.

## DEVIATE rows

| row | what | why | owner | exit | review |
|---|---|---|---|---|---|
| B1 | `constant_p(x)` is 0 | GCC documents 0 for an expression it cannot show constant; clang's `llvm.is.constant` is folded after inlining, so a value inlining proves constant reads 0 here | w-1 (the `lower-constant-intrinsics` slot row) | `llvm.is.constant` in MIR, the pass (needs `replaceAndRecursivelySimplify` and `removeUnreachableBlocks` in llrm-transforms), the C front end emitting it | 11-09 |
| B2 | `unreachable()` falls through | a refinement (reaching it is undefined), but SimplifyCFG and InstCombine lose the fact | funcattrs-port | a `TerminatorKind::Unreachable` after the call in `translate.rs` (control flow after it must open a block) | 11-09 |
| B3 | `trap()` calls `abort` | the assembler and LIR have no `ud2` (the encoder refuses `hlt`); the program stops abnormally either way | funcattrs-port | `llvm.trap` in MIR, `ud2` in the encoder and LIR | 11-09 |
| B4 | `memcpy`, `memset`, `memmove` are calls | clang emits the intrinsics, which MemCpyOpt and the loop passes read | funcattrs-port | map them to `llvm.memcpy`/`llvm.memset`/`llvm.memmove` in `translate.rs` | 11-09 |

Limits: `expect` with a pointer or float first argument keeps its own type and is not the intrinsic (GCC types it `long`);
`__builtin_expect_with_probability` is refused (not in MIR).
