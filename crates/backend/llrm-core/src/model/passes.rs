//! Port of `qbopt/model/passes.py`: what a pass is, MIR in and MIR out.
//!
//! `transform(body) -> body` is the whole contract. Anything a pass needs to
//! know about the module it is compiling is given when the pass is made.

use crate::support::hash::IndexMap;

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
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self::defined_in("builtins", kind, message)
    }

    /// A class a qbopt module defines, e.g. `qbopt.backend.masm`'s `Unprintable`.
    pub fn defined_in(module: &'static str, kind: &'static str, message: impl Into<String>) -> Self {
        Exception { module, kind, message: message.into() }
    }
}

impl std::fmt::Display for Exception {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Exception {}

/// One transformation over a lowered body.
///
/// The same contract as MIRTransform, one form down.
pub trait LIRTransform {
    /// `type(self).__name__`.
    fn class_name(&self) -> &'static str;

    fn name(&self) -> &str {
        ""
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        let _ = body;
        Err(format!("{} has no transform", self.class_name()))
    }

    /// `transform`, with the class of what it raised. A phase whose Python
    /// raises no class of its own says `Exception`, which nothing catches.
    fn transform_raising(&mut self, body: LirBody) -> Result<LirBody, Exception> {
        self.transform(body).map_err(|message| Exception::new("Exception", message))
    }
}

pub use llrm_mir::target::{AddressForm, OperationCosts};

pub const DEFAULT_MAX_UNROLL_ITERATIONS: i64 = 16;
pub const DEFAULT_MAX_UNROLLED_OPERATIONS: i64 = 200;

/// What GCC's command line says about optimization, as one value.
///
/// `-O` picks the defaults, `--param` the copy budgets, `-f` each pass. They
/// are independent of the CPU, as in GCC. `grows=false` is -Os's
/// `UL_NO_GROWTH` (tree-ssa-loop-ivcanon.cc): a copy is taken only when it
/// is no larger.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Options {
    pub level: String,
    // --param max-completely-peel-times
    pub max_unroll_iterations: i64,
    // --param max-completely-peeled-insns
    pub max_unrolled_operations: i64,
    pub grows: bool,
    pub lcssa: bool,
    pub floatloop: bool,
    pub fold: bool,
    pub decide: bool,
    pub dead: bool,
    pub hoist: bool,
    pub forward: bool,
    pub drop_loads: bool,
    pub drop_stores: bool,
    pub promote: bool,
    pub strength: bool,
    pub unroll: bool,
    pub peel: bool,
    pub fill: bool,
    pub unswitch: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            level: "O2".to_owned(),
            max_unroll_iterations: DEFAULT_MAX_UNROLL_ITERATIONS,
            max_unrolled_operations: DEFAULT_MAX_UNROLLED_OPERATIONS,
            grows: true,
            lcssa: true,
            floatloop: true,
            fold: true,
            decide: true,
            dead: true,
            hoist: true,
            forward: true,
            drop_loads: true,
            drop_stores: true,
            promote: true,
            strength: true,
            unroll: true,
            peel: true,
            fill: true,
            unswitch: false,
        }
    }
}

/// Python `LEVELS`: each `-O` level's options, in declaration order.
#[allow(non_snake_case)]
pub fn LEVELS() -> IndexMap<&'static str, Options> {
    IndexMap::from_iter([
        ("O2", Options::default()),
        ("Os", Options { level: "Os".to_owned(), grows: false, ..Options::default() }),
    ])
}

/// Python `O2`.
#[allow(non_snake_case)]
pub fn O2() -> Options {
    LEVELS()["O2"].clone()
}
