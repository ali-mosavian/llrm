> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/ipsccp.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/ipsccp.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/ipsccp.rs sha256:635982baa28934dce19db220c5e055f9637b995d7a79fa9dc875e5b36195e576 -->

# ipsccp

Interprocedural sparse conditional constant propagation. A translation of
LLVM's
[`runIPSCCP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L108)
(d1106deb71cc) without function specialisation: one solve over every body
of a module, then the rewrites. What a body's callers pass becomes its
arguments' constants and `range` attributes, what it returns reaches each
call, and a global only ever stored one value is read as that value.

## How it works
`run_ipsccp` gives each module's defined functions to
`Solver::module`. A function whose every caller the module can see (it
is private and its address is not taken) has its arguments tracked: its
entry runs only when a call reaches it, and each argument is the join of
what the calls pass. Any other function is entered from outside, with its
arguments as their attributes say. Each return whose definition is exact
is tracked, as is each private scalar global that only non-volatile loads
and stores of its type name.

After the solve, each body has its arguments that are constants replaced
(if its entry runs), its values replaced by their constants, its blocks
that never run emptied and deleted (an entry that never runs becomes
`unreachable`), and its edges that are never taken cut. Then the tracked
returns and arguments get their `range` or `nonnull` attributes
(`inferReturnAttributes`, `inferArgAttributes`). A tracked function whose
return is a constant (or never computed) and that only the module calls
returns `poison`, since every call already reads the constant; it and its
calls lose `returned` and the attributes that would make `poison`
undefined. The stores to a global whose value is known go, as do the loads
left.

```text
 @g(i32 %a): ret %a * 5           @g(i32 range(i32 1, 4) %a): ret %a * 5
 @f: call @g(1); call @g(3)  ==>  @f: call @g(1); call @g(3)
```

## DEVIATEs
1. No `FunctionSpecializer`
   ([L161](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L161),
   on at -O3 in the pin): [`crate::sccpsolver`](sccpsolver.md) DEVIATE 2. Exit: its
   translation.
2. The solve is per module of a program, and
   `canTrackArgumentsInterprocedurally`
   ([L133](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L133))
   is `hasExactDefinition` and [`crate::interprocedural`](../analyses/interprocedural.md)'s `private`: not
   exported by the program, reached by no other module, not address-taken
   and not `noipa`, where the pin asks `hasLocalLinkage` and
   `!hasAddressTaken`. For one module with no link list the two agree,
   except that a function a metadata node names is address-taken here.
3. A tracked global whose stores went stays in the module, with no users;
   the pin erases it and rewrites its debug expression
   ([L344-L361](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L344)).
   Erasing renumbers the module's globals under the program pass's
   analyses; GlobalOpt's dead-global step deletes it. MIR has no debug
   expressions.
4. `canTrackGlobalVariableInterprocedurally`'s users are the operands of the
   module's instructions and its globals' initializers (MIR constants have
   no use lists, so `removeDeadConstantUsers`
   ([L153](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L153))
   has nothing to do); a constant expression naming the global is a user
   that is neither load nor store.
5. A pointer argument replaced moves the body's argument memory effects to
   other memory with `memory::unsplit`
   ([L195](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L195));
   MIR has no errno location.
6. A dead block is deleted even when its address is taken (`hasAddressTaken`
   at
   [L256](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L256)):
   MIR has no `blockaddress`.
7. `getMRVFunctionsTracked` is visited in module order (a `SmallPtrSet` in
   the pin); which returns are zapped does not depend on it.
8. What `run_ipsccp` returns is every body it edited; the pin's
   `MadeChanges` leaves out a non-pointer argument replaced
   ([L199](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L199)).

The solver's and the rewrites' DEVIATEs are those of
[`crate::sccpsolver`](sccpsolver.md).

## Example
`@g` is called with 1 and 3 and its argument becomes `range(i32 1, 4) %a`:
unit test `test_a_callee_called_with_1_and_3_gets_the_range_1_to_4` in
`crates/opt/llrm-transforms/src/ipsccp_tests.rs`.

## Runs
Once per compile at -O1 and up, first in [`crate::walk::ModuleIpo`](walk.md)
(`run_ipsccp_with`): after the early function pipeline, before the
CGSCC walk, where the pin's `buildModuleSimplificationPipeline` runs
`IPSCCPPass`.

## See also
- Uses: [`crate::sccpsolver`](sccpsolver.md), [`crate::sccp`](sccp.md)'s rewrites,
  [`crate::predicateinfo`](predicateinfo.md)
