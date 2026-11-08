pub mod fastisel;
#[rustfmt::skip]
pub mod generated;
pub mod x86_classes;
pub mod x86_divrem;
pub mod x86_fcmp;
pub mod x86_fplogic;
pub mod x86_hooks;
pub mod x86_llrm;
pub mod x86_memop128;
pub mod x86_mul;
pub mod x86_rmw;
pub mod x86_select;
pub mod x86_shift;
pub mod x86_test;
/// Hooks for the x86-32 tables over a small test DAG.
pub mod x86_testing;

#[cfg(test)]
mod tests;
