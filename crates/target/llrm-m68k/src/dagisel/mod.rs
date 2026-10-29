//! The DAG selector for m68k: tables generated from `dagisel.toml` by
//! `tools/dagisel/regen-target.sh`, the address matchers and the hooks.

pub mod addr;
#[rustfmt::skip]
#[path = "generated/asm.rs"]
pub mod asm;
pub mod call;
#[rustfmt::skip]
#[allow(unused_variables)]
pub mod generated;
pub mod function;
pub mod hooks;
pub mod lowering;
pub mod pipeline;
pub mod predicates;
