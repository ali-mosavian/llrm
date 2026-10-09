//! Port of `qbopt/model/passes.py`: what a pass is, MIR in and MIR out.
//!
//! `transform(body) -> body` is the whole contract. Anything a pass needs to
//! know about the module it is compiling is given when the pass is made.

use crate::model::lir::LirBody;

/// A Python exception crossing a boundary: `type(error).__module__`,
/// `__name__` and `str(error)`. A caller that catches by class, as
/// `wholeseg` does, reads `kind`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Exception {
    pub module: &'static str,
    pub kind: &'static str,
    pub message: String,
}

impl Exception {
    /// A builtin: `ValueError`, `OSError`, `Exception`.
    pub fn new(
        kind: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::defined_in("builtins", kind, message)
    }

    /// A class a qbopt module defines, e.g. `qbopt.backend.masm`'s
    /// `Unprintable`.
    pub fn defined_in(
        module: &'static str,
        kind: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Exception { module, kind, message: message.into() }
    }
}

impl std::fmt::Display for Exception {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Exception {}

/// One transformation over a lowered body.
///
/// A pass over LIR: takes a body and returns one.
pub trait LIRTransform {
    /// `type(self).__name__`.
    fn class_name(&self) -> &'static str;

    fn name(&self) -> &str {
        ""
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        let _ = body;
        Err(format!("{} has no transform", self.class_name()))
    }

    /// `transform`, with the class of what it raised. A phase whose Python
    /// raises no class of its own says `Exception`, which nothing catches.
    fn transform_raising(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, Exception> {
        self.transform(body).map_err(|message| Exception::new("Exception", message))
    }
}

pub use llrm_mir::target::{AddressForm, OperationCosts};

pub const DEFAULT_MAX_UNROLL_ITERATIONS: i64 = 16;
pub const DEFAULT_MAX_UNROLLED_OPERATIONS: i64 = 200;
