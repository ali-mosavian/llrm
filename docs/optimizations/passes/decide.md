> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/decide.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/decide.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/decide.rs sha256:faa2e36d1f967761a35f46e5fb9702a0af40d596b4f7495a20d4fbcd2a9bb2c1 -->

# decide

Branches whose way is known, decided: LLVM's JumpThreading and
SimplifyCFG on known conditions. Adapted from llrm-core's
`optimize/transform.rs` `Decide` (`decided`, `_threaded`,
`_executable_successors`, `_outcome`, `_switch_target`, `_signed`,
`_TAKEN`). What is known is consts' answer, carried across loops by
constant_cycles along the edges that run.

What changed with the IR:
- A branch reads an `i1`: the flags a compare left are the `icmp`
  `_comparison` finds, and a condition consts knows outright decides the
  branch too.
- A branch not taken became an inert owner falling through; here it is a
  jump to the other arm.
- Threading bypasses a block holding only a jump, for a `br` or a `switch`;
  explicit and fall-through edges are one kind here. A conditional branch
  both of whose arms go one way is a jump, as the old one was when threading
  made it so.
- consts reads the module's globals through the outer proxy.
- A compare's answer (`_signed`, `_TAKEN`) is `consts::holds`, which folds
  an `icmp`.

Dropped, no rich MIR analogue: the memory a compare's operand read
(`held`: a load is its own instruction, and its fact consts'), and the
case width checks (a switch's cases are its condition's type).

Tests, in `decide_tests.rs`:
`test_empty_jump_threading_preserves_phi_inputs_and_effects` is ported. The
other old tests of `_outcome`, `_switch_target` and `_executable_successors`
read BC objects through the raise (`raising_dispatch_tests`) or belong to
`peelsize`, whose own tests cover them.
