> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/access.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/access.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/access.rs sha256:b395cf1493b17d06e0bbb1b226af8f1ab9f3a2d287e994caf0e86caed5e3cf85 -->

# access

Each memory access of a loop as an affine function of its counters, in the
form [`crate::dependence`](dependence.md) tests.

## How it works
`references(unit, counters, variant, scanned, roots)` reads every load and
store of the blocks `scanned` and writes its address as `root + constant +
outer * i + inner * j + sum of invariant terms`: `root` is the pointer no
`getelementptr` computes, `i` and `j` are the iteration numbers of the
`counters` loops (outermost first; one loop is `inner`), and a term is an
invariant value with its coefficient. A counter is a header phi stepped by
a constant from one start (`start + step * iteration`).

A value defined in a block of `variant` (the blocks whose values change
with the counters) is expanded through `add`, `sub`, `mul` and `shl` by a
constant and `sext`, to depth 12; any other operation, or a root that
`variant` defines, makes the whole listing `None`, as does a volatile
access, a call, an `invoke` or an `alloca` in `scanned`. A value defined
anywhere else is an invariant term.

`Apart` says which references through different roots may meet. `Roots`
numbers the distinct roots, so two listings made with one `Roots`
(two loops, say) agree on which accesses share an object. A `Reference`
says whether its root is an object's own address (`memory::object_of`: an
alloca or a global): two such roots that differ are different objects, and
`dependence` calls their accesses independent. Roots that are not
identified (a pointer parameter) are the caller's to relate.

```text
  %o = mul i32 %j, 4
  %p = getelementptr i8, ptr @a, i32 %o        root @a, constant 0,
  store i32 %v, ptr %p                         inner 4, width 4, write
```

## Example
The body of `for (j = 0; j < n; j++) a[j + 2] = a[j]` with `a` an `i32`
array, listed with the loop as `inner`:

```text
load  a[j]       constant 0, inner 4, width 4, read
store a[j + 2]   constant 8, inner 4, width 4, write
```

From unit test `an_array_walked_by_a_counter_is_an_affine_access` in
`access_tests.rs`.

## Provides
`references`, `Reference`, `Roots`, `Apart`. Consumers: `unroll_jam`
(`crates/opt/llrm-transforms`).

## See also
LLVM ScalarEvolution add recurrences read by `DependenceAnalysis`; gcc's
`dr_analyze_innermost` and `access_fn` of `tree-data-ref.cc`.
