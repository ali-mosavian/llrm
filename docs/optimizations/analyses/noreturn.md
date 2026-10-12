> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/noreturn.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/noreturn.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/noreturn.rs sha256:fe06059a71a46215da6c56ac458188dcd934c3ae4cd6da29b5296368757061d6 -->

# noreturn

Which bodies and blocks cannot reach a normal return, and the edit that cuts
a block after a call that cannot return.

## How it works
A call is terminal when its callee is in the given `noreturn` set or the
call or callee states `noreturn` (`terminal_sites`; an `invoke` may still
unwind, so only a `call` counts). `_cannot_return` walks the CFG from the
entry, stopping at a block holding a terminal call; if it reaches a block
ending in `ret`, the body can return.

`inferred` and `inferred_in` (over a program, so a declaration of a proven
body stops as the body does) find the greatest set of bodies none of which
returns. Every body starts in the set; a body that can return, given the
calls to bodies still in the set, leaves it, and only its callers are looked
at again, as gcc's ipa-pure-const propagates. Starting full lets a closed
recursive SCC prove terminal.

`cold` finds blocks from which every path ends in a terminal call, an
`unreachable`, or a block with a call that states `cold`. It is a least
fixed point, so a loop that never exits is not cold, and when the entry is
cold the result is empty. `stranded(function, header)` finds blocks from
which no path reaches a `ret` or the header: a loop that exits into one
stops the program.

`after_terminal_calls` is the edit. In each block, everything after the
first terminal call is erased (uses replaced by `poison`), the call is kept
as the observable action, and the block ends in `unreachable`.
`cfg::_unreachable` then removes blocks the entry no longer reaches.

```text
   bb:                         bb:
     %a = call @exit(1)          %a = call @exit(1)   ; kept
     %b = add %a, 1      -->     unreachable
     store %b, %p
     br %next                  %next loses bb as a predecessor
```

## Provides
`TerminalSites` (analysis of the function with no body proven noreturn),
`terminal_sites`, `inferred`, `inferred_in`, `cold`, `stranded`,
`after_terminal_calls`. Consumers: [`crate::interprocedural`](interprocedural.md)
(`noreturn_procedures`, `terminal_calls`), [`crate::branchprob`](branchprob.md) (`cold`),
[`crate::induction`](induction.md) (`stranded`) and the `hoist` pass in llrm-transforms.

## Needs
[`crate::effects`](../backend/effects.md) (`callee`, `states` for the `noreturn` and `cold`
attributes), [`crate::cfg`](cfg.md).
