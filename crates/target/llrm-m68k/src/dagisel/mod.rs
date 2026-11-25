//! The DAG selector for m68k: tables generated from `dagisel.toml` by
//! `tools/dagisel/regen-target.sh`, the address matchers and the hooks.

pub mod adda;
pub mod addr;
#[rustfmt::skip]
#[path = "generated/asm.rs"]
pub mod asm;
pub mod anyext;
pub mod bfextu;
pub mod bfffo;
pub mod btst;
pub mod call;
pub mod cmpa;
pub mod dbf;
pub mod divrem;
pub mod wordjump;
#[rustfmt::skip]
#[path = "generated/enc.rs"]
pub mod enc;
#[rustfmt::skip]
#[allow(unused_variables)]
pub mod generated;
pub mod function;
pub mod hooks;
pub mod indexed;
pub mod local;
pub mod lowering;
pub mod mulprice;
pub mod opcodes;
pub mod pea;
pub mod pipeline;
pub mod predicates;
pub mod rmw;
pub mod scaled;
pub mod short;
pub mod switchprice;
pub mod synth;
pub mod widemul;
pub mod wordmul;
pub mod zextmask;
