//! The DAG selector for wasm: tables generated from `dagisel.toml` by
//! `tools/dagisel/regen-target.sh`, the address matchers and the hooks.

pub mod addr;
#[rustfmt::skip]
#[allow(unused_variables)]
pub mod generated;
pub mod hooks;
pub mod predicates;
