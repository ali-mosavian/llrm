> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/noreturn.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/noreturn.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/noreturn.rs sha256:5bce366bca1b937c2e97a9dd64856291a9a7c35405771d5f4eb5c35f6f54f6b8 -->

# noreturn

Finds the bodies and blocks that cannot reach a normal return, and cuts a
block after a call that cannot return, so the code after it and the paths
into it fold away.

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

## Example
`after_terminal_calls` at the call to `@exit`:

Before:

```mir
define i16 @f() {
b0:
  call void @exit()
  %x = add i16 1, 2
  br label %b1

b1:
  store i16 %x, ptr @g
  ret i16 %x
}
```

After:

```mir
define i16 @f() {
b0:
  call void @exit()
  unreachable
}
```

The tail of `b0` is gone, so `b1` is unreachable and removed; the call stays
and still runs first.

From unit test `test_terminal_call_removes_its_newly_unreachable_successor`
in `noreturn_tests.rs`.

## Provides
`terminal_sites`, `inferred`, `inferred_in`, `cold`, `stranded`,
`after_terminal_calls`. Consumers: [`crate::interprocedural`](interprocedural.md)
(`noreturn_procedures`, `terminal_calls`), [`crate::branchprob`](branchprob.md) (`cold`),
[`crate::induction`](induction.md) (`stranded`) and the `hoist` pass in llrm-transforms.

## Needs
[`crate::effects`](../backend/effects.md) (`callee`, `states` for the `noreturn` and `cold`
attributes), [`crate::cfg`](cfg.md).
