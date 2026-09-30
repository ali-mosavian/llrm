//! Port of `qbopt/cfront`: C through Open Watcom's front end.

pub mod compile;
pub mod debug;
pub mod hir;
pub mod raise_hir;
pub mod libfunc;
pub mod stream;
pub mod translate;
#[cfg(test)]
mod callconv_tests;
#[cfg(test)]
mod debug_tests;
