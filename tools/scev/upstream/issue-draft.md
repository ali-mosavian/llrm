# [SCEV] getAddExpr/getMulExpr crash when an operand is an AddRec held with a use flag

`ScalarEvolution::getAddExpr` and `getMulExpr` fold loop-invariant operands into an
`AddRec`, then find the `AddRec` again in `Ops` to replace it:

```cpp
for (unsigned i = 0;; ++i)
  if (Ops[i] == AddRec) {   // ScalarEvolution.cpp, getAddExpr and getMulExpr
```

`Ops[i]` is a `SCEVUse`; `operator==(const SCEV *)` compares `getOpaqueValue()`, which
includes the use-flag bits. When the `AddRec` operand carries a use flag (`SCEVUse(AR, FlagNUW)`),
the comparison is never true and the loop reads past the end of `Ops`.

## Reproducer

`repro.cpp` (attached): builds `{%b,+,%a}<L>`, holds it as `SCEVUse(AR, FlagNUW)`, and calls
`getAddExpr(X, getUDivExpr(X, ARnuw), ARnuw)` with `loop.ll`. At current main
(29eff2031) a Release build segfaults in `getAddExpr`. `getMulExpr` crashes the same way.
A pinned d1106deb71cc build crashes identically.

```
$ clang++ -std=c++17 -fno-rtti repro.cpp $(llvm-config --cxxflags --ldflags --libs analysis irreader passes) -o repro
$ ./repro loop.ll
Segmentation fault
```

`opt -passes='print<scalar-evolution>' loop.ll` does not show it: IR has no use flags, so
no pass reaches this path today. It is reachable by any caller that passes a flagged
`SCEVUse` (for example one built with `SCEVUse(S, Flags)`).

## Expected

The `AddRec` is found whatever its use bits. Comparing `Ops[i].getPointer() == AddRec`
fixes both lookups.

Seen while porting ScalarEvolution to another compiler.
