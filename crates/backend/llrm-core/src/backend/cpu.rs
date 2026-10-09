//! Port of `qbopt/backend/cpu.py`: one immutable description of a
//! code-generation tuning target.
//!
//! Costs from `cycles::timings` are rankings; `timing` holds the smaller
//! audited subset used for legality-changing decisions.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use llrm_target::Target;

use crate::model::passes::{
    AddressForm, DEFAULT_MAX_UNROLL_ITERATIONS, DEFAULT_MAX_UNROLLED_OPERATIONS, OperationCosts,
};
use crate::support::hash::IndexMap;

#[derive(Clone, Debug)]
pub struct Profile {
    pub name: String,
    pub issue_width: i64,
    pub in_order: bool,
    pub prefix_cost: i64,
    pub partial_register_stall: i64,
    pub register_capacity: i64,
    /// The target's address spaces by role.
    pub spaces: llrm_mir::spaces::Spaces,
    /// The convention its description gives a function nothing outside the
    /// program reaches.
    pub private: Option<llrm_mir::target::PrivateConvention>,
    /// The conventions as the description states them, which decide where a
    /// call's arguments go and what removes them.
    pub calling: Option<llrm_target::calling::Stated>,
    /// The operand size an instruction has without a prefix, in bytes.
    pub operand_bytes: i64,
    pub call_register_capacity: i64,
    // Preferred forms only; the complete legal set is `address_forms`.
    pub address_scales: BTreeSet<i64>,
    pub _costs: Vec<(String, i64)>,
    pub _latencies: Vec<(String, i64)>,
    pub operations: OperationCosts,
    // P5 can issue a U/V pair only for a restricted set of forms; distinct
    // from generic issue width.
    pub pentium_pairing: bool,
    // 386 32-bit SIB addressing through an address-size prefix: legal, never
    // a native/free scale, and considered before spill/recompute.
    pub address_forms: Vec<AddressForm>,
    // GCC's target-independent complete-peel default.
    pub max_unroll_iterations: i64,
    // GCC's `max-completely-peeled-insns` default, in semantic operations.
    pub max_unrolled_operations: i64,
    // A 67h override's predecoder stall, charged apart from the prefix
    // issue cost so profitability makes the scorer's comparison.
    pub address_prefix_stall: i64,
    // -Os: where the costs tie on nothing else, the shorter encoding.
    pub size: bool,
    /// Whether the allocator tries other shapes of a body than the one it is
    /// given and keeps the cheapest, as LLVM and GCC do not.
    pub search: bool,
    /// Whether a function is made by both routes (the allocator alone and the
    /// spiller's) and the cheaper kept; else by the allocator alone.
    pub routes: bool,
    /// With `search`, whether it tries every shape (`-fallocation-search-all`,
    /// -Omax) or the one the spills suggest and the body without splitting.
    pub exhaustive: bool,
    /// How the target this profile is for builds its cost model from the CPU's
    /// prices.
    pub model: llrm_target::CostModel,
    /// The chains of shifts and adds found for constant multiplies under this
    /// profile's prices (GCC's `alg_hash`).
    pub multiplies: MultiplyChains,
}

/// What `arithmetic` found for a multiply by a constant under one profile's
/// prices, by constant and by whether a `lea` may be used: the profile owns it,
/// so it lives and is keyed with the prices it was found under.
#[derive(Default)]
pub struct MultiplyChains(
    std::sync::Mutex<crate::support::hash::HashMap<(i64, bool), Option<(Vec<(&'static str, i64)>, i64)>>>,
);

impl MultiplyChains {
    pub fn get(
        &self,
        number: i64,
        with_lea: bool,
    ) -> Option<Option<(Vec<(&'static str, i64)>, i64)>> {
        self.0.lock().expect("not poisoned").get(&(number, with_lea)).cloned()
    }

    pub fn put(
        &self,
        number: i64,
        with_lea: bool,
        found: Option<(Vec<(&'static str, i64)>, i64)>,
    ) {
        self.0.lock().expect("not poisoned").insert((number, with_lea), found);
    }
}

// A cache is not part of what a profile is: a clone starts empty and two
// profiles are equal by their prices.
impl Clone for MultiplyChains {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for MultiplyChains {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        formatter.write_str("MultiplyChains")
    }
}

impl PartialEq for MultiplyChains {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl Eq for MultiplyChains {}

impl std::hash::Hash for MultiplyChains {
    fn hash<H: std::hash::Hasher>(
        &self,
        _: &mut H,
    ) {
    }
}

impl Profile {
    /// What the CPU prices, as a target builds its cost model from.
    pub fn cpu_prices(&self) -> llrm_target::CpuPrices {
        llrm_target::CpuPrices {
            costs: self._costs.clone(),
            prefix: self.prefix_cost,
            address_stall: self.address_prefix_stall,
            registers: self.register_capacity,
            call_registers: self.call_register_capacity,
            address_forms: self.address_forms.clone(),
            operations: self.operations.clone(),
            spaces: self.spaces,
            private: self.private.clone(),
            calling: self.calling.map(|one| one.0),
        }
    }

    /// The MIR target this profile prices: its target's cost model on its CPU.
    pub fn target(&self) -> std::rc::Rc<dyn llrm_mir::target::Machine> {
        (self.model)(&self.cpu_prices())
    }

    /// The form that indexes by a dword register: where a target has one, a
    /// 16-bit target's behind the address-size prefix (`secondary`), a flat
    /// one's native.
    pub fn dword_address_form(&self) -> Option<&AddressForm> {
        self.address_forms.iter().find(|form| form.index_width == 4)
    }

    /// The dataclass constructor with every defaulted field at its default, and
    /// the registers and cost model of the target `arch`.
    pub fn new(
        arch: &dyn Target,
        name: &str,
        issue_width: i64,
        in_order: bool,
        prefix_cost: i64,
        partial_register_stall: i64,
    ) -> Self {
        Self {
            name: name.to_owned(),
            issue_width,
            in_order,
            prefix_cost,
            partial_register_stall,
            register_capacity: arch.register_capacity(),
            spaces: arch.layout().spaces.roles,
            private: private_convention(arch),
            calling: Some(llrm_target::calling::Stated(arch.calling())),
            operand_bytes: arch.operand_bytes(),
            call_register_capacity: arch.callee_saved().len() as i64,
            address_scales: BTreeSet::from([1]),
            _costs: Vec::new(),
            _latencies: Vec::new(),
            operations: OperationCosts::default(),
            pentium_pairing: false,
            address_forms: Vec::new(),
            max_unroll_iterations: DEFAULT_MAX_UNROLL_ITERATIONS,
            max_unrolled_operations: DEFAULT_MAX_UNROLLED_OPERATIONS,
            address_prefix_stall: 0,
            size: false,
            search: true,
            routes: true,
            exhaustive: false,
            model: arch.cost_model(),
            multiplies: Default::default(),
        }
    }

    /// The existing target-ranking cost for one named instruction form.
    pub fn cost(
        &self,
        operation: &str,
    ) -> Result<i64, String> {
        _listed(&self._costs, operation).ok_or_else(|| format!("{} has no cost for {operation}", self.name))
    }

    /// The cheaper way to double a register: `add r,r` or `shl r,1`, which
    /// set the same flags. The 386 and 486 take three clocks for the D1 shift.
    pub fn doubling(&self) -> Result<&'static str, String> {
        Ok(if self.cost("alu_rr")? < self.cost("shift_r1")? { "alu_rr" } else { "shift_r1" })
    }

    /// Whether `words` pops into a dead register clean a call's arguments
    /// off the stack in place of `add sp,2*words`: shorter for one or two
    /// words, and where size is not wanted, only if no slower.
    pub fn pops_arguments(
        &self,
        words: i64,
    ) -> Result<bool, String> {
        Ok(words <= 2 && (self.size || words * self.cost("pop_r")? <= self.cost("alu_rr")?))
    }

    /// Whether this profile has an explicit ranking for a form.
    pub fn prices(
        &self,
        operation: &str,
    ) -> bool {
        _listed(&self._costs, operation).is_some()
    }

    /// The existing dependency latency, distinct from occupancy cost.
    pub fn latency(
        &self,
        operation: &str,
    ) -> Result<i64, String> {
        _listed(&self._latencies, operation).ok_or_else(|| format!("{} has no latency for {operation}", self.name))
    }
}

/// What `table` lists for `operation`; the last of a name listed twice, as a
/// map built from the table says.
fn _listed(
    table: &[(String, i64)],
    operation: &str,
) -> Option<i64> {
    table.iter().rev().find(|(name, _)| name == operation).map(|(_, value)| *value)
}

/// `str | Profile`, the argument every public function here accepts.
#[derive(Clone, Copy, Debug)]
pub enum ProfileOrName<'a> {
    /// A name on 16-bit x86, which the tests of this crate are written for.
    #[cfg(test)]
    Name(&'a str),
    Profile(&'a Profile),
}

#[cfg(test)]
impl<'a> From<&'a str> for ProfileOrName<'a> {
    fn from(value: &'a str) -> Self {
        Self::Name(value)
    }
}

impl<'a> From<&'a Profile> for ProfileOrName<'a> {
    fn from(value: &'a Profile) -> Self {
        Self::Profile(value)
    }
}

/// Native medium-model addressing, then the legal secondary 67h form.
/// The target's address forms, priced by `costs` and `prefix`: the 386's
/// table has no prefix column, so its own is passed.
fn _address_forms(
    arch: &dyn Target,
    costs: &OperationCosts,
    prefix: i64,
    address_stall: i64,
) -> Vec<AddressForm> {
    arch.address_forms(&OperationCosts { prefix, ..costs.clone() }, address_stall)
}

fn _profile(
    arch: &dyn Target,
    name: &str,
) -> Result<Profile, String> {
    let table = arch.cpu_table(name).ok_or_else(|| format!("unknown CPU target: {name}"))?;
    let costs: IndexMap<&str, i64> = table.clocks.iter().map(|(form, clocks)| (form.as_str(), *clocks)).collect();
    let operations = arch.operation_costs(&|form| costs[form], table.prefix);
    Ok(Profile {
        pentium_pairing: table.pairing,
        address_forms: _address_forms(arch, &operations, table.prefix, table.lcp_stall),
        operations,
        _costs: table.clocks.clone(),
        _latencies: table.latency.clone(),
        address_prefix_stall: table.lcp_stall,
        ..Profile::new(arch, name, table.issue, table.in_order, table.prefix, table.partial_stall)
    })
}

/// The profiles made so far, by target, CPU and size: each made once, as the
/// passes hold them.
static _MADE: LazyLock<
    std::sync::Mutex<crate::support::hash::HashMap<(&'static str, String, bool, bool, bool, bool), &'static Profile>>,
> = LazyLock::new(Default::default);

/// `name`'s profile on the target `arch`, tuned for size where `size`.
pub fn tuned_for(
    arch: &dyn Target,
    name: &str,
    size: bool,
) -> Result<&'static Profile, String> {
    tuned_searching(arch, name, size, true)
}

/// `tuned_for`, trying every shape of a body where `exhaustive`.
pub fn tuned_exhaustive(
    arch: &dyn Target,
    name: &str,
    size: bool,
    exhaustive: bool,
) -> Result<&'static Profile, String> {
    tuned_with(arch, name, size, true, exhaustive)
}

/// `tuned_for`, the allocator trying other shapes of a body only where
/// `search`.
pub fn tuned_searching(
    arch: &dyn Target,
    name: &str,
    size: bool,
    search: bool,
) -> Result<&'static Profile, String> {
    tuned_with(arch, name, size, search, false)
}

/// `tuned_searching`, every shape where `exhaustive`.
pub fn tuned_with(
    arch: &dyn Target,
    name: &str,
    size: bool,
    search: bool,
    exhaustive: bool,
) -> Result<&'static Profile, String> {
    tuned_routing(arch, name, size, search, exhaustive, search)
}

/// `tuned_with`, the routes compared where `routes` whatever the search: -O1 to
/// -Os allocate once, and choose the route.
pub fn tuned_routing(
    arch: &dyn Target,
    name: &str,
    size: bool,
    search: bool,
    exhaustive: bool,
    routes: bool,
) -> Result<&'static Profile, String> {
    if !arch.cpus().contains(&name) {
        return Err(format!("unknown CPU target: {name}; {} has {}", arch.name(), arch.cpus().join(", ")));
    }
    let mut made = _MADE.lock().expect("the profiles are not poisoned");
    let key = (arch.name(), name.to_owned(), size, search, exhaustive, routes);
    if let Some(&one) = made.get(&key) {
        return Ok(one);
    }
    let one: &'static Profile = Box::leak(Box::new(Profile {
        size,
        search,
        exhaustive,
        routes,
        ..(_profile(arch, name)).expect("every listed CPU has a profile")
    }));
    made.insert(key, one);
    Ok(one)
}

/// `name`'s profile on 16-bit x86, tuned for size where `size`.
#[cfg(test)]
pub fn tuned(
    name: &str,
    size: bool,
) -> Result<&'static Profile, String> {
    tuned_for(&llrm_x86_m16::M16, name, size)
}

#[cfg(test)]
pub fn names() -> Vec<&'static str> {
    llrm_target::Target::cpus(&llrm_x86_m16::M16).to_vec()
}

pub fn profile<'a>(value: impl Into<ProfileOrName<'a>>) -> Result<&'a Profile, String> {
    match value.into() {
        ProfileOrName::Profile(value) => Ok(value),
        #[cfg(test)]
        ProfileOrName::Name(value) => named(value),
    }
}

#[cfg(test)]
pub fn named(name: &str) -> Result<&'static Profile, String> {
    tuned(name, false)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn test_every_public_cpu_name_has_one_immutable_profile() {
        assert_eq!(names().into_iter().map(|name| profile(name).unwrap().name.as_str()).collect::<Vec<_>>(), names());
        for name in names() {
            let target = profile(name).unwrap();
            assert_eq!(target.operations.add, target.cost("alu_rr").unwrap());
            assert_eq!(target.operations.address, target.cost("lea").unwrap());
            assert_eq!(target.operations.memory_update, target.cost("alu_mr").unwrap());
            assert_eq!(target.operations.prefix, target.prefix_cost);
            assert_eq!(target.operations.r#move, target.cost("mov_rr").unwrap());
            assert_eq!(target.operations.call, target.cost("call_far").unwrap());
            assert_eq!(target.operations.return_, target.cost("ret_far").unwrap());
            assert_eq!(target.operations.float_add, target.cost("x87_add").unwrap());
            assert_eq!(target.operations.float_multiply, target.cost("x87_mul").unwrap());
            assert_eq!(target.operations.float_divide, target.cost("x87_div").unwrap());
            assert_eq!(target.operations.float_load, target.cost("x87_load").unwrap());
            assert_eq!(target.operations.float_store, target.cost("x87_store").unwrap());
            assert_eq!(target.max_unroll_iterations, 16);
            assert_eq!(target.max_unrolled_operations, 200);
        }
        assert!(profile("P5").unwrap().pentium_pairing);
        assert!(!names().into_iter().any(|name| name != "P5" && profile(name).unwrap().pentium_pairing));
    }

    #[test]
    fn test_operation_costs_do_not_change_the_existing_profile_positional_shape() {
        let target = Profile {
            register_capacity: 3,
            call_register_capacity: 1,
            address_scales: BTreeSet::from([1]),
            _costs: Vec::new(),
            _latencies: Vec::new(),
            ..Profile::new(&llrm_x86_m16::M16, "test", 1, true, 0, 0)
        };

        assert_eq!(target.register_capacity, 3);
        assert_eq!(target.call_register_capacity, 1);
        assert_eq!(target.address_scales, BTreeSet::from([1]));
        assert_eq!(target.operations, OperationCosts::default());
        assert!(!target.pentium_pairing);
    }

    #[test]
    fn test_medium_model_profiles_distinguish_native_and_67h_addressing() {
        for target in names().into_iter().map(|name| profile(name).unwrap()) {
            let [native, secondary] = target.address_forms.as_slice() else { panic!("two forms") };
            assert!(target.address_scales == native.scales && native.scales == BTreeSet::from([1]));
            assert!(native.index_width == 2 && !native.secondary);
            assert!(secondary.index_width == 4 && secondary.scales == BTreeSet::from([1, 2, 4, 8]));
            assert!(secondary.secondary && secondary.extra_bytes == 1);
            assert_eq!(secondary.use_cost, target.prefix_cost + target.address_prefix_stall);
            assert_eq!(secondary.extension_cost, target.operations.extend);
        }

        assert_eq!(profile("P6").unwrap().address_prefix_stall, 6);
        assert_eq!(profile("Core").unwrap().address_prefix_stall, 3);
        assert!(
            !["386", "486", "P5", "K5", "K6", "K7"]
                .into_iter()
                .any(|name| profile(name).unwrap().address_prefix_stall != 0)
        );

        let legacy = AddressForm::new(4, BTreeSet::from([1, 2, 4, 8]), 0, 0, 0, false, Some(true)).unwrap();
        assert!(legacy.secondary && legacy.fallback == Some(true));
    }

    #[test]
    fn test_unknown_cpu_is_rejected_at_the_shared_boundary() {
        assert!(profile("pentium").unwrap_err().contains("unknown CPU target"));
    }

    /// indexed.lru_use improved on Core but regressed P5/P6 when every legal
    /// 67h form was treated as equally profitable.
    #[test]
    fn test_secondary_address_form_is_ranked_against_reload_and_spill_work() {
        let selected: BTreeSet<&str> = names()
            .into_iter()
            .filter(|name| {
                let target = profile(*name).unwrap();
                target.address_forms[1].before_spill(&target.operations)
            })
            .collect();

        assert_eq!(selected, BTreeSet::from(["386", "K5", "K6", "K7", "Core"]));
    }

    /// The target every frontend lowers to prices a call at its clocks and,
    /// tuned for size, at its bytes: `size_costs` was not forwarded, so it
    /// was `costs`, and every MIR decision made "for size" (the inliner,
    /// the callee-pop convention) weighed clocks.
    #[test]
    fn test_the_lowered_target_forwards_its_size_costs() {
        use llrm_mir::target::Machine;
        let abi = crate::abi::qb::HirAbi {
            runtime: crate::hir::model::RuntimeProfile::Freestanding,
            objects: Default::default(),
            preserved: Default::default(),
            stack_check: None,
        };
        let target = crate::abi::qb::LoweredTarget::of(profile("486").unwrap(), abi);
        assert_eq!((target.costs().call, target.size_costs().call), (18, 5));
    }

    /// Real mode indexes by a dword behind the address-size prefix; a flat
    /// target's form is native. Both are the form that takes a dword index,
    /// and the word form is not.
    #[test]
    fn test_the_dword_address_form_is_the_one_that_indexes_by_dwords() {
        let real = profile("486").unwrap();
        let form = real.dword_address_form().expect("real mode has one");
        assert!(form.secondary && form.index_width == 4 && form.scales.contains(&4));
        let mut flat = real.clone();
        flat.address_forms = vec![AddressForm::new(4, BTreeSet::from([1, 2, 4, 8]), 0, 0, 0, false, None).unwrap()];
        let form = flat.dword_address_form().expect("a flat target has one");
        assert!(!form.secondary && form.index_width == 4);
        flat.address_forms = vec![real.address_forms[0].clone()];
        assert!(flat.dword_address_form().is_none());
    }
}

/// The convention `arch`'s description gives a private function, and the ones
/// that may take it.
fn private_convention(arch: &dyn Target) -> Option<llrm_mir::target::PrivateConvention> {
    let calling = arch.calling();
    let to = llrm_target::calling::Calling::number_of(calling.private()?.cc.as_deref())?;
    let mut from: Vec<u32> = calling
        .conventions
        .iter()
        .filter(|one| calling.replaceable(one))
        .filter_map(|one| llrm_target::calling::Calling::number_of(one.cc.as_deref()))
        .collect();
    from.sort_unstable();
    from.dedup();
    Some(llrm_mir::target::PrivateConvention { to, from })
}
