//! Decodes the pin's `X86GenDAGISel.inc` into `llrm-dagisel`'s matcher table.
//!
//! # How it works
//! [`inc`] reads the text of the `.inc` into its byte tables, [`decode`] turns
//! the bytes into a tree of `Matcher` items, the CPU's features prune it and
//! [`emit`] writes Rust.
//!
//! # Used by
//! `tools/dagisel/regen.sh`.

pub mod decode;
pub mod emit;
pub mod features;
pub mod flatten;
pub mod ids_rs;
pub mod inc;
pub mod predicate;
pub mod prune;
pub mod target;
