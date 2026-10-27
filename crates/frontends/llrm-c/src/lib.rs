//! C through Open Watcom's front end.

// Without the feature every test that compiles a `.c` file fails with
// `Unsupported("llrm was built without the toolchain feature")`: one error
// here instead. The root package enables it, so a workspace build has it.
#[cfg(all(test, not(feature = "toolchain")))]
compile_error!("llrm-c's lib tests need --features toolchain: cargo test -p llrm-c --features toolchain");

pub mod builtins;
#[cfg(test)]
mod callconv_tests;
pub mod compile;
pub mod debug;
#[cfg(test)]
mod debug_tests;
pub mod hir;
pub mod hwreg;
pub mod libfunc;
pub mod ow_facts;
pub mod predefined;
pub mod raise_hir;
pub mod stream;
#[cfg(test)]
mod toolchain_tests;
pub mod translate;
