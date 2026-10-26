> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/globalsaa.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/globalsaa.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/globalsaa.rs sha256:e0f47e21f4b4c32383a2d4359fb0a63c6fcbea379d02a39364dfa0f61abbc7c9 -->

# globalsaa

Which globals no code outside the program reaches but by name: LLVM's
GlobalsAA, GCC's ipa-reference.

## How it works
A global variable is tracked when three things hold. No body in any module
lets its address escape: each body's pointer solve (`alias::points_to`)
lists the globals that escape. No live initializer holds its address (an
initializer is read only through its own global). And the program does not
export it, or the program's runtime module lists it in `!llrm.named`: the
runtime names it but never hands its address out.

```text
 module A  @g = internal global ...      tracked: no body leaks &@g
           f(): store 1, @g              reached only by its name,
           h(): call @ext()              so a declared @ext cannot touch @g
```

A tracked global is uncaptured (`memory::global_object`), so no nonlocal
reach meets it. What each body does to one, callees included, is in
[`crate::alias`](alias.md)'s summaries. A callee no summary describes reaches the
tracked globals as `Globals::unsummarized` says:

- a body of the module (or of another module that defines this declaration):
  all of them;
- otherwise the named ones it writes (those its `!llrm.writes` node in the
  runtime module lists, or all without one) and, unless it is `nocallback`
  (`calls_back`), what the module's entry points do, as it may call back
  into them (`entries`: bodies not internal, or address-taken).

`!llrm.named = !{!0}` with `!0 = !{ptr @g, ...}`; `!llrm.writes = !{!1,
...}` with `!1 = !{ptr @routine, ptr @g, ...}`: of the named globals,
`@routine` writes by name only those listed. The runtime module's globals
stand for each module's of the same names.

Across modules, `ProgramGlobals` finds, per module, the globals another
module lets escape or holds in an initializer (`Elsewhere.escaped`), its
declarations another module defines and the functions another takes the
address of, comparing globals by the definition they resolve to. A global
passed to a callee that is `noretain` is not named for this purpose. Each
body's contribution is kept while its history is unchanged, and `covers`
says whether a held result still claims no more than a fresh one would.

## Example
`@g` is internal and no body leaks its address, so a call that cannot name
it keeps the 7:

```mir
@g = internal global i16 0

declare void @outside(ptr) nocallback

define i16 @f() {
b0:
  store i16 7, ptr @g
  call void @outside(ptr null)
  %r = load i16, ptr @g
  ret i16 %r
}
```

`%r` is `Known { n: 7, width: 16 }`. Passing `@g` to `@outside`, storing
`@g` anywhere, or holding it in another global's initializer makes `%r`
unknown.

From unit tests `a_private_global_is_kept_across_a_call_that_cannot_name_it`
and `a_private_global_whose_address_leaves_is_forgotten` in
`globalsaa_tests.rs`.

## Provides
`Globals` (`tracked`, `unsummarized`, `entries`, `covers`), `analysis`,
`ProgramGlobals`, and the helpers `embedded` and `calls_back`. The manager
entry is [`crate::manager::GlobalsAA`](manager.md). Consumers: [`crate::alias`](alias.md),
[`crate::memory`](memory.md), and `globalopt` and `globaldce` in `llrm-transforms`
(`embedded`).

## Needs
[`crate::alias`](alias.md) (`points_to`), [`crate::cfg::Shape`](cfg.md), `llrm_mir`'s program
and module analyses.

## See also
LLVM `GlobalsAA`; GCC `ipa-reference.cc`.
