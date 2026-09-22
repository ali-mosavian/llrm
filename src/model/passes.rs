//! Port of `qbopt/model/passes.py`: what a pass is, MIR in and MIR out.
//!
//! `transform(body) -> body` is the whole contract. Anything a pass needs to
//! know about the module it is compiling is given when the pass is made.

use std::any::Any;
use std::collections::BTreeSet;
use std::sync::Arc;

use indexmap::IndexMap;

use crate::model::ir::Space;
use crate::model::lir::LirBody;
use crate::model::mir::MirBody;

/// One transformation over a body.
///
/// Implementors override `transform` and nothing else. `name` is what the
/// pipeline lists it by and what `--only` matches.
pub trait MIRTransform {
    /// `type(self).__name__`.
    fn class_name(&self) -> &'static str;

    fn name(&self) -> &str {
        ""
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        let _ = body;
        Err(format!("{} has no transform", self.class_name()))
    }

    /// `__repr__`.
    fn repr(&self) -> String {
        format!(
            "<{}>",
            if self.name().is_empty() {
                self.class_name()
            } else {
                self.name()
            }
        )
    }
}

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

    /// `__repr__`.
    fn repr(&self) -> String {
        format!(
            "<{}>",
            if self.name().is_empty() {
                self.class_name()
            } else {
                self.name()
            }
        )
    }
}

/// One legal indexed-address family and its costs above the native form.
///
/// Machine-neutral: MIR may know an address can use a four-byte index with
/// scales 1/2/4/8, never how x86 spells it.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AddressForm {
    pub index_width: i64,
    pub scales: BTreeSet<i64>,
    pub extra_bytes: i64,
    pub use_cost: i64,
    pub extension_cost: i64,
    pub secondary: bool,
    // Compatibility name for `secondary`; both views stay identical.
    pub fallback: Option<bool>,
}

impl AddressForm {
    /// The dataclass constructor and `__post_init__`.
    pub fn new(
        index_width: i64,
        scales: BTreeSet<i64>,
        extra_bytes: i64,
        use_cost: i64,
        extension_cost: i64,
        secondary: bool,
        fallback: Option<bool>,
    ) -> Result<Self, String> {
        if fallback.is_some() && secondary && fallback != Some(secondary) {
            return Err("an address form cannot disagree about whether it is secondary".to_owned());
        }
        let selected = fallback.unwrap_or(secondary);
        Ok(Self {
            index_width,
            scales,
            extra_bytes,
            use_cost,
            extension_cost,
            secondary: selected,
            fallback: Some(selected),
        })
    }

    /// Whether this form is cheap enough to try before a frame spill.
    pub fn before_spill(&self, costs: &OperationCosts) -> bool {
        let direct = self.extension_cost + self.use_cost <= costs.load;
        let amortized = self.extension_cost <= costs.r#move
            && self.extension_cost + self.use_cost
                <= costs.shift + costs.address + costs.r#move + costs.store;
        !self.secondary || direct || amortized
    }
}

/// Machine-neutral costs a MIR profitability decision may compare.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OperationCosts {
    pub add: i64,
    pub multiply: i64,
    pub divide: i64,
    pub shift: i64,
    pub address: i64,
    pub load: i64,
    pub store: i64,
    pub memory_update: i64,
    pub branch: i64,
    pub prefix: i64,
    pub r#move: i64,
    pub call: i64,
    pub return_: i64,
    pub float_add: i64,
    pub float_multiply: i64,
    pub float_divide: i64,
    pub float_load: i64,
    pub float_store: i64,
    pub extend: i64,
}

impl Default for OperationCosts {
    fn default() -> Self {
        Self {
            add: 1,
            multiply: 1,
            divide: 1,
            shift: 1,
            address: 1,
            load: 1,
            store: 1,
            memory_update: 1,
            branch: 1,
            prefix: 0,
            r#move: 1,
            call: 1,
            return_: 1,
            float_add: 1,
            float_multiply: 1,
            float_divide: 1,
            float_load: 1,
            float_store: 1,
            extend: 1,
        }
    }
}

// Target-independent complete-peel safeguards. A selected target may
// override them; an omitted selection receives the default 386 policy.
pub const DEFAULT_MAX_UNROLL_ITERATIONS: i64 = 16;
pub const DEFAULT_MAX_UNROLLED_OPERATIONS: i64 = 200;

/// What a pass may be told about the module it is compiling.
///
/// Handed to a pass when it is made, never reachable from `transform`.
#[derive(Clone, Debug)]
pub struct Where {
    pub dgroup: BTreeSet<i64>,
    pub calls: Option<IndexMap<i64, String>>,
    pub bounds: Option<IndexMap<(Space, i64), Vec<i64>>>,
    pub blocks: Option<Vec<Arc<dyn Any + Send + Sync>>>,
    pub found: Option<Arc<dyn Any + Send + Sync>>,
    pub registers: i64,
    // Values the target can keep live across an ordinary call.
    pub call_registers: i64,
    // The multipliers an address may apply to an index register.
    pub index_scales: BTreeSet<i64>,
    // Complete legal indexed-address families.
    pub address_forms: Vec<AddressForm>,
    // Semantic work only.
    pub costs: OperationCosts,
    pub options: Options,
}

impl Default for Where {
    fn default() -> Self {
        Self {
            dgroup: BTreeSet::new(),
            calls: None,
            bounds: None,
            blocks: None,
            found: None,
            registers: 0,
            call_registers: 0,
            index_scales: BTreeSet::new(),
            address_forms: Vec::new(),
            costs: OperationCosts::default(),
            options: Options::default(),
        }
    }
}

impl Where {
    /// `self.calls or {}`.
    pub fn named(&self) -> IndexMap<i64, String> {
        match &self.calls {
            Some(calls) if !calls.is_empty() => calls.clone(),
            _ => IndexMap::new(),
        }
    }
}

// ---- early port (agent B) ----

/// What GCC's command line says about optimization, as one value.
///
/// `-O` picks the defaults, `--param` the copy budgets, `-f` each pass. They
/// are independent of the CPU, as in GCC. `grows=false` is -Os's
/// `UL_NO_GROWTH` (tree-ssa-loop-ivcanon.cc): a copy is taken only when it
/// is no larger.
#[derive(Clone, Debug, Eq, PartialEq)]
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

#[allow(non_snake_case)]
pub fn LEVELS() -> IndexMap<&'static str, Options> {
    IndexMap::from([
        ("O2", Options::default()),
        ("Os", Options { level: "Os".to_owned(), grows: false, ..Options::default() }),
    ])
}

#[allow(non_snake_case)]
pub fn O2() -> Options {
    Options::default()
}
