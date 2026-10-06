//! Port of `qbopt/backend/cpu.py`: one immutable description of a
//! code-generation tuning target.
//!
//! Costs from `cycles::timings` are rankings; `timing` holds the smaller
//! audited subset used for legality-changing decisions.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::support::hash::IndexMap;

use llrm_target::Target;

use llrm_x86_code16::timings;
use crate::model::passes::{
    AddressForm, DEFAULT_MAX_UNROLL_ITERATIONS, DEFAULT_MAX_UNROLLED_OPERATIONS, OperationCosts,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Profile {
    pub name: String,
    pub issue_width: i64,
    pub in_order: bool,
    pub prefix_cost: i64,
    pub partial_register_stall: i64,
    pub register_capacity: i64,
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
    /// How the target this profile is for builds its cost model from the CPU's prices.
    pub model: llrm_target::CostModel,
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
        }
    }

    /// The MIR target this profile prices: its target's cost model on its CPU.
    pub fn target(&self) -> std::rc::Rc<dyn llrm_mir::target::Machine> {
        (self.model)(&self.cpu_prices())
    }

    /// The form that indexes by a dword register: where a target has one, a
    /// 16-bit target's behind the address-size prefix (`secondary`), a flat one's
    /// native.
    pub fn dword_address_form(&self) -> Option<&AddressForm> {
        self.address_forms.iter().find(|form| form.index_width == 4)
    }

    /// The dataclass constructor with every defaulted field at its default.
    pub fn new(
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
            register_capacity: llrm_x86_code16::GENERAL.len() as i64,
            call_register_capacity: llrm_x86_code16::PRESERVED.len() as i64,
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
            model: llrm_target::Target::cost_model(&llrm_x86_code16::Code16),
        }
    }

    /// The existing target-ranking cost for one named instruction form.
    pub fn cost(&self, operation: &str) -> Result<i64, String> {
        let costs: IndexMap<&str, i64> = self
            ._costs
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect();
        costs
            .get(operation)
            .copied()
            .ok_or_else(|| format!("{} has no cost for {operation}", self.name))
    }

    /// The cheaper way to double a register: `add r,r` or `shl r,1`, which
    /// set the same flags. The 386 and 486 take three clocks for the D1 shift.
    pub fn doubling(&self) -> Result<&'static str, String> {
        Ok(if self.cost("alu_rr")? < self.cost("shift_r1")? { "alu_rr" } else { "shift_r1" })
    }

    /// Whether `words` pops into a dead register clean a call's arguments
    /// off the stack in place of `add sp,2*words`: shorter for one or two
    /// words, and where size is not wanted, only if no slower.
    pub fn pops_arguments(&self, words: i64) -> Result<bool, String> {
        Ok(words <= 2 && (self.size || words * self.cost("pop_r")? <= self.cost("alu_rr")?))
    }

    /// Whether this profile has an explicit ranking for a form.
    pub fn prices(&self, operation: &str) -> bool {
        let costs: IndexMap<&str, i64> = self
            ._costs
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect();
        costs.contains_key(operation)
    }

    /// The existing dependency latency, distinct from occupancy cost.
    pub fn latency(&self, operation: &str) -> Result<i64, String> {
        let latencies: IndexMap<&str, i64> = self
            ._latencies
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect();
        latencies
            .get(operation)
            .copied()
            .ok_or_else(|| format!("{} has no latency for {operation}", self.name))
    }
}

/// `str | Profile`, the argument every public function here accepts.
#[derive(Clone, Copy, Debug)]
pub enum ProfileOrName<'a> {
    Name(&'a str),
    Profile(&'a Profile),
}

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

static _I386_COSTS: LazyLock<IndexMap<&'static str, i64>> = LazyLock::new(|| {
    IndexMap::from_iter([
        ("alu_rr", 2),
        ("alu_ri", 2),
        ("alu_rm", 6),
        ("alu_mr", 8),
        ("mov_rr", 2),
        ("mov_rm", 4),
        ("mov_mr", 2),
        ("mov_ri", 2),
        ("shift_ri", 3),
        ("shift_r1", 3),
        ("movzx", 4),
        ("imul_r32", 22),
        ("imul_m32", 26),
        ("mul_r16", 22),
        ("mul_r32", 38),
        ("div_r16", 27),
        ("idiv_r32", 43),
        ("idiv_m32", 47),
        ("cdq", 2),
        ("push_r", 2),
        ("push_m", 6),
        ("push_i", 2),
        ("pop_r", 4),
        // Intel's 80386 table: POP m16/m32 is five clocks, not the
        // four-unit register form.
        ("pop_m", 5),
        ("pop_seg", 8),
        ("mov_seg_r", 8),
        ("les", 8),
        ("nop", 3),
        ("jmp_short", 7),
        ("jcc", 7),
        // Intel's 80386 table: Jcc is 7+m taken, 3 not.
        ("jcc_not_taken", 3),
        ("call_far", 37),
        ("ret_far", 18),
        ("ret_pop", 18),
        ("lahf", 2),
        ("sahf", 3),
        ("lea", 2),
        ("leave", 6),
        ("rep_stos", 5),
        ("rep_stos_cell", 5),
        // Intel's 80386 table: REP MOVS is 8+4n.
        ("rep_movs", 8),
        ("rep_movs_cell", 4),
        // GCC's i386 table: x87 loads/stores eight units, arithmetic
        // 23/27/88. Memory arithmetic includes both components.
        ("x87_load", 8),
        // 80387 register exchange is eighteen clocks.
        ("x87_exchange", 18),
        ("x87_store", 8),
        ("x87_convert_store", 35),
        ("x87_add", 23),
        ("x87_add_m", 31),
        ("x87_mul", 27),
        ("x87_mul_m", 35),
        ("x87_div", 88),
        ("x87_div_m", 96),
        ("x87_control_load", 8),
        ("x87_control_store", 8),
    ])
});

/// Translate backend instruction forms into MIR's semantic vocabulary.
fn _operation_costs(costs: &IndexMap<&str, i64>, prefix: i64) -> OperationCosts {
    OperationCosts {
        add: costs["alu_rr"],
        multiply: costs["mul_r16"],
        divide: costs["div_r16"],
        shift: costs["shift_ri"],
        address: costs["lea"],
        carry: llrm_mir::target::carry_cost(costs["movzx"], costs["alu_rr"], costs["shift_ri"], costs["mov_rr"]),
        carry_step: llrm_mir::target::step_cost(costs["alu_rr"]),
        load: costs["mov_rm"],
        store: costs["mov_mr"],
        memory_update: costs["alu_mr"],
        branch: costs["jcc"],
        prefix,
        r#move: costs["mov_rr"],
        call: costs["call_far"],
        return_: costs["ret_far"],
        argument: costs["mov_mr"] + costs["mov_rm"],
        pop: costs["pop_r"],
        adjust: costs["alu_ri"],
        return_pops: costs["ret_pop"] - costs["ret_far"],
        float_add: costs["x87_add"],
        float_multiply: costs["x87_mul"],
        float_divide: costs["x87_div"],
        float_load: costs["x87_load"],
        float_store: costs["x87_store"],
        extend: costs["movzx"],
        // The expansion saves ES, loads it from the cells' segment, and sets
        // the value and count before `rep stos`; then restores ES.
        fill: costs["rep_stos"] + 2 * costs["push_r"] + 2 * costs["pop_seg"] + 2 * costs["mov_ri"],
        fill_cell: costs["rep_stos_cell"],
        copy: costs["rep_movs"] + 2 * costs["push_r"] + 2 * costs["pop_seg"] + 3 * costs["mov_ri"],
        copy_cell: costs["rep_movs_cell"],
        direction: 2 * costs["alu_rr"],
    }
}

/// Native medium-model addressing, then the legal secondary 67h form.
/// The target's address forms, priced by `costs` and `prefix`: the 386's
/// table has no prefix column, so its own is passed.
fn _address_forms(arch: &dyn Target, costs: &OperationCosts, prefix: i64, address_stall: i64) -> Vec<AddressForm> {
    arch.address_forms(&OperationCosts { prefix, ..costs.clone() }, address_stall)
}

/// What the target states of its registers and prices, over the CPU's own.
fn _of_target(arch: &dyn Target, profile: Profile) -> Profile {
    Profile { register_capacity: arch.register_capacity(), call_register_capacity: arch.callee_saved().len() as i64, model: arch.cost_model(), ..profile }
}

fn _profile(arch: &dyn Target, name: &str) -> Result<Profile, String> {
    if name == "386" {
        let costs = _I386_COSTS.clone();
        let operations = _operation_costs(&costs, 0);
        return Ok(_of_target(arch, Profile {
            address_forms: _address_forms(arch, &operations, 0, 0),
            operations,
            _costs: costs
                .iter()
                .map(|(key, value)| ((*key).to_owned(), *value))
                .collect(),
            _latencies: costs
                .iter()
                .map(|(key, value)| ((*key).to_owned(), *value))
                .collect(),
            ..Profile::new(name, 1, true, 0, 0)
        }));
    }
    let at = timings::ARCHS
        .iter()
        .position(|one| *one == name)
        .ok_or_else(|| "tuple.index(x): x not in tuple".to_owned())?;
    let costs: IndexMap<&str, i64> = timings::COST
        .iter()
        .map(|(operation, values)| (*operation, values[at]))
        .collect();
    let operations = _operation_costs(&costs, timings::PREFIX[at]);
    Ok(_of_target(arch, Profile {
        pentium_pairing: name == "P5",
        address_forms: _address_forms(arch, &operations, timings::PREFIX[at], timings::LCP_STALL[at]),
        operations,
        _costs: costs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), *value))
            .collect(),
        _latencies: timings::LATENCY
            .iter()
            .map(|(operation, values)| ((*operation).to_owned(), values[at]))
            .collect(),
        address_prefix_stall: timings::LCP_STALL[at],
        ..Profile::new(
            name,
            timings::ISSUE[at],
            timings::INORDER[at] != 0,
            timings::PREFIX[at],
            timings::PARTIAL_STALL[at],
        )
    }))
}

/// The profiles made so far, by target, CPU and size: each made once, as the passes hold them.
static _MADE: LazyLock<std::sync::Mutex<std::collections::HashMap<(&'static str, String, bool), &'static Profile>>> = LazyLock::new(Default::default);

/// `name`'s profile on the target `arch`, tuned for size where `size`.
pub fn tuned_for(arch: &dyn Target, name: &str, size: bool) -> Result<&'static Profile, String> {
    if !arch.cpus().contains(&name) {
        return Err(format!("unknown CPU target: {name}"));
    }
    let mut made = _MADE.lock().expect("the profiles are not poisoned");
    let key = (arch.name(), name.to_owned(), size);
    if let Some(&one) = made.get(&key) {
        return Ok(one);
    }
    let one: &'static Profile = Box::leak(Box::new(Profile { size, ..(_profile(arch, name)).expect("every listed CPU has a profile") }));
    made.insert(key, one);
    Ok(one)
}

/// `name`'s profile on 16-bit x86, tuned for size where `size`.
pub fn tuned(name: &str, size: bool) -> Result<&'static Profile, String> {
    tuned_for(&llrm_x86_code16::Code16, name, size)
}

pub fn names() -> Vec<&'static str> {
    llrm_target::Target::cpus(&llrm_x86_code16::Code16).to_vec()
}

pub fn profile<'a>(value: impl Into<ProfileOrName<'a>>) -> Result<&'a Profile, String> {
    match value.into() {
        ProfileOrName::Profile(value) => Ok(value),
        ProfileOrName::Name(value) => named(value),
    }
}

pub fn named(name: &str) -> Result<&'static Profile, String> {
    tuned(name, false)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn test_every_public_cpu_name_has_one_immutable_profile() {
        assert_eq!(
            names()
                .into_iter()
                .map(|name| profile(name).unwrap().name.as_str())
                .collect::<Vec<_>>(),
            names()
        );
        for name in names() {
            let target = profile(name).unwrap();
            assert_eq!(target.operations.add, target.cost("alu_rr").unwrap());
            assert_eq!(target.operations.address, target.cost("lea").unwrap());
            assert_eq!(
                target.operations.memory_update,
                target.cost("alu_mr").unwrap()
            );
            assert_eq!(target.operations.prefix, target.prefix_cost);
            assert_eq!(target.operations.r#move, target.cost("mov_rr").unwrap());
            assert_eq!(target.operations.call, target.cost("call_far").unwrap());
            assert_eq!(target.operations.return_, target.cost("ret_far").unwrap());
            assert_eq!(target.operations.float_add, target.cost("x87_add").unwrap());
            assert_eq!(
                target.operations.float_multiply,
                target.cost("x87_mul").unwrap()
            );
            assert_eq!(
                target.operations.float_divide,
                target.cost("x87_div").unwrap()
            );
            assert_eq!(
                target.operations.float_load,
                target.cost("x87_load").unwrap()
            );
            assert_eq!(
                target.operations.float_store,
                target.cost("x87_store").unwrap()
            );
            assert_eq!(target.max_unroll_iterations, 16);
            assert_eq!(target.max_unrolled_operations, 200);
        }
        assert!(profile("P5").unwrap().pentium_pairing);
        assert!(
            !names()
                .into_iter()
                .any(|name| name != "P5" && profile(name).unwrap().pentium_pairing)
        );
    }

    #[test]
    fn test_operation_costs_do_not_change_the_existing_profile_positional_shape() {
        let target = Profile {
            register_capacity: 3,
            call_register_capacity: 1,
            address_scales: BTreeSet::from([1]),
            _costs: Vec::new(),
            _latencies: Vec::new(),
            ..Profile::new("test", 1, true, 0, 0)
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
            let [native, secondary] = target.address_forms.as_slice() else {
                panic!("two forms")
            };
            assert!(target.address_scales == native.scales && native.scales == BTreeSet::from([1]));
            assert!(native.index_width == 2 && !native.secondary);
            assert!(secondary.index_width == 4 && secondary.scales == BTreeSet::from([1, 2, 4, 8]));
            assert!(secondary.secondary && secondary.extra_bytes == 1);
            assert_eq!(
                secondary.use_cost,
                target.prefix_cost + target.address_prefix_stall
            );
            assert_eq!(secondary.extension_cost, target.operations.extend);
        }

        assert_eq!(profile("P6").unwrap().address_prefix_stall, 6);
        assert_eq!(profile("Core").unwrap().address_prefix_stall, 3);
        assert!(
            !["386", "486", "P5", "K5", "K6", "K7"]
                .into_iter()
                .any(|name| profile(name).unwrap().address_prefix_stall != 0)
        );

        let legacy =
            AddressForm::new(4, BTreeSet::from([1, 2, 4, 8]), 0, 0, 0, false, Some(true)).unwrap();
        assert!(legacy.secondary && legacy.fallback == Some(true));
    }

    #[test]
    fn test_unknown_cpu_is_rejected_at_the_shared_boundary() {
        assert!(
            profile("pentium")
                .unwrap_err()
                .contains("unknown CPU target")
        );
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

    /// The target every frontend lowers to prices a call at its clocks and, tuned for size, at its
    /// bytes: `size_costs` was not forwarded, so it was `costs`, and every MIR decision made "for
    /// size" (the inliner, the callee-pop convention) weighed clocks.
    #[test]
    fn test_the_lowered_target_forwards_its_size_costs() {
        use llrm_mir::target::Machine;
        let abi = crate::abi::qb::HirAbi { runtime: crate::hir::model::RuntimeProfile::Freestanding, objects: Default::default(), preserved: Default::default(), stack_check: None };
        let target = crate::abi::qb::LoweredTarget::of(profile("486").unwrap(), abi);
        assert_eq!((target.costs().call, target.size_costs().call), (18, 5));
    }

    /// Real mode indexes by a dword behind the address-size prefix; a flat target's form is
    /// native. Both are the form that takes a dword index, and the word form is not.
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
