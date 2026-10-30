//! The pin's DAG instruction selector: the matcher table tblgen writes for the
//! target and the interpreter that walks it.
//!
//! # How it works
//! `llvm-tblgen -gen-dag-isel X86.td` writes a byte table that
//! `SelectCodeCommon` interprets. `llrm-dagisel-gen` decodes that table once,
//! drops what the target's CPU cannot run and checks in the result under
//! `generated/`; [`interp::Selector`] is the interpreter. The DAG is
//! reached through the [`dag::Dag`] trait and the target's hooks through
//! [`dag::Hooks`]. Regenerate with `tools/dagisel/regen.sh`; a test fails when
//! the checked-in tables are stale.
//!
//! # Used by
//! `X86DAGToDAG` (piece C1) and the shadow harness (D0).
//!
//! # See also
//! `docs/working/selectiondag-plan.md`, section 3.

pub mod dag;
#[rustfmt::skip]
pub mod generated;
pub mod instr;
pub mod interp;
pub mod matcher;
pub mod predicates;
pub mod x86_addr;
pub mod x86_classes;
pub mod x86_divrem;
pub mod x86_fcmp;
pub mod x86_fplogic;
pub mod x86_hooks;
pub mod x86_llrm;
pub mod x86_mul;
pub mod x86_rmw;
pub mod x86_select;
pub mod x86_shift;
pub mod x86_test;

/// A small DAG and hooks for the tests and for `examples/price.rs`.
pub mod testing;
#[cfg(test)]
mod tests;
