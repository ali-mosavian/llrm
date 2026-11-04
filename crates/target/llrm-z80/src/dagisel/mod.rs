//! The DAG selector's tables for the Z80: generated from `dagisel.toml` by
//! `tools/dagisel/regen-target.sh`.

#[rustfmt::skip]
#[allow(unused_variables)]
pub mod generated;
pub mod call;
pub mod function;
pub mod hooks;
pub mod lowering;
pub mod pipeline;
pub mod predicates;
