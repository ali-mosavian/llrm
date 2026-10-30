//! One immutable description of a code-generation tuning target: a CPU's
//! prices and a target's registers, address forms and cost model.
//!
//! # How it works
//! [`Profile`] joins two sources. The target (`llrm_target::Target`) gives
//! the register capacity, the general registers, address spaces, calling
//! conventions and cost model. A row of the target's timing table
//! (`timings.times`) gives the CPU's clocks per instruction form, latencies,
//! issue width, in-order flag, prefix cost, partial-register stall, pairing
//! and address-size-prefix stall. Instruction forms are named by strings
//! such as `alu_rr` or `movzx`.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   tab["<b>Target::cpu_table(name)</b>"]:::src
//!   tgt["<b>Target</b><br/><small>registers, spaces,<br/>calling, cost model</small>"]:::src
//!   drv["<b>driver</b><br/><small>size, search,<br/>exhaustive</small>"]:::src
//!   prof["<b>Profile</b>"]:::opt
//!   q["<b>cost(alu_rr), latency(..)</b><br/><small>doubling()<br/>clears_before_byte_load()<br/>pops_arguments(n), ...</small>"]:::mid
//!   mir["<b>target()</b><br/><small>the MIR cost model</small>"]:::mid
//!   tab --> prof
//!   tgt --> prof
//!   drv --> prof
//!   prof --> q
//!   prof --> mir
//! ```
//!
//! *Where a CPU profile comes from and what reads it*
//!
//! The four driver flags select the allocator's effort: `size` (-Os, ties
//! broken by the shorter encoding), `search` (the allocator tries other
//! shapes of a body), `exhaustive` (every shape) and `routes` (both the
//! allocator alone and the spiller route are made and the cheaper kept).
//! They are part of the profile so that every phase reads one value.
//! [`tuned_optimizing`] builds a profile once per (target, CPU, size, search,
//! exhaustive, routes) and leaks it, so a profile is a `&'static` reference
//! that passes hold freely. An unknown CPU is an error that lists the
//! target's CPUs.
//!
//! Costs are rankings, not guarantees. [`Profile::cost`] and
//! [`Profile::latency`] err when a form has no entry; [`Profile::prices`]
//! asks first. The smaller audited subset of multiply and divide clocks
//! that may change legality is read through `timing.rs`. Helper decisions
//! that depend on the prices live here: [`Profile::doubling`] (`add r,r`
//! or `shl r,1`), [`Profile::clears_before_byte_load`] (`xor r,r; mov rl,m`
//! or `movzx`) and [`Profile::pops_arguments`] (pops against `add sp`).
//! `MultiplyChains` caches the constant-multiply chains found under this
//! profile's prices (GCC's `alg_hash`); a clone starts with an empty cache.
//!
//! # Example
//! Facts the profiles state (from the target's timing tables):
//!
//! ```text
//! address_prefix_stall            P6 6, Core 3, 386 486 P5 K5 K6 K7 0
//! 67h form beats a spill (before_spill)   386 K5 K6 K7 Core
//! call / size call on a 486       18 / 5
//! ```
//!
//! From unit tests
//! `test_medium_model_profiles_distinguish_native_and_67h_addressing`,
//! `test_secondary_address_form_is_ranked_against_reload_and_spill_work` and
//! `test_the_lowered_target_forwards_its_size_costs` in `cpu.rs`.
//!
//! # Used by
//! The driver (`driver/mod.rs`, `Options::cpu`) builds it from the machine's
//! CPU name. Consumers by grep: `allocate`, `schedule`, `peephole`,
//! `isel`, `jumps`, `executed`, `division`, `arithmetic`, `timing`,
//! `exactaddress`, `affine`, `phielim` and `parcopy`.
//!
//! # See also
//! Arithmetic by CPU: `docs/machine/arithmetic-targets.md`.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use llrm_lir::registers::RegId;
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
    /// The registers a value may be placed in, from the description's `gpr`
    /// class.
    pub general: Vec<RegId>,
    /// The target's address spaces by role.
    pub spaces: llrm_mir::spaces::Spaces,
    /// The convention its description gives a function nothing outside the
    /// program reaches.
    pub private: Option<llrm_mir::target::PrivateConvention>,
    /// The conventions as the description states them, which decide where a
    /// call's arguments go and what removes them.
    pub calling: Option<llrm_target::calling::Stated>,
    /// The platform's linear ranges no program data occupies, for a flat
    /// target.
    pub foreign: llrm_target::Foreign,
    /// The platform's ports and what an access to each may do to memory.
    pub ports: llrm_target::PortTable,
    /// The operand size an instruction has without a prefix, in bytes.
    pub operand_bytes: i64,
    /// `getPrefLoopAlignment` as a power of two; 0: loops are not aligned.
    pub loop_alignment: u8,
    pub call_register_capacity: i64,
    /// The floating values it holds in registers at once.
    pub float_register_capacity: i64,
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
    /// Whether the backend's optional phases run: off at -O0, where like gcc's
    /// and clang's nothing is improved after selection and allocation
    /// (`flow::machine_with`).
    pub optimized: bool,
    /// `CodeGenOptLevel::Aggressive` (-O3): tail duplication in placement
    /// copies blocks of four instructions, not two.
    pub aggressive: bool,
    /// `-fexcess-precision=standard` (`-ffloat-store`): a float conversion
    /// rounds to its type, where otherwise x87 keeps the wider value in its
    /// register, as gcc's `-fexcess-precision=fast` does.
    pub float_store: bool,
    /// A spill of a float value rounds to its type, as the pin's RFP32 and
    /// RFP64 spills do (`C`): the spill cell is as wide as the value's type.
    /// Off, the cell keeps x87's wider format and a spill loses no precision
    /// the language allows to stay (Nib and BASIC: only a store the program
    /// writes rounds).
    pub rounds_float_spills: bool,
    /// A load of a cell nothing writes while the value lives is made again
    /// where it is read (DEVIATE D-I: the pin remats only invariant loads).
    /// Off by default (`-fllrm-stable-loads` turns it on): it cost 6.6 M Ir
    /// and saved QCport 1.4% bytes (docs/working/stable-loads.md).
    pub stable_loads: bool,
    /// -Oz (`Function::hasMinSize` on every function, as clang marks them): the
    /// machine passes that ask it take size over speed.
    pub min_size: bool,
    /// A spill of an x87 value keeps the register's 80 bits (`fstp tbyte`),
    /// as real BC keeps a temporary (BASIC). DEVIATE D-RFP5, exit: none, the
    /// pin rounds; QB follows real BC text.
    pub spills_extended: bool,
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
            float_registers: self.float_register_capacity,
            address_forms: self.address_forms.clone(),
            operations: self.operations.clone(),
            spaces: self.spaces,
            private: self.private.clone(),
            calling: self.calling.map(|one| one.0),
            foreign: self.foreign,
            ports: self.ports,
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
            general: general_registers(arch),
            spaces: arch.layout().spaces.roles,
            private: private_convention(arch, llrm_support::env_flag!("LLRM_PRIVATE_WATCALL")),
            calling: Some(llrm_target::calling::Stated(arch.calling())),
            foreign: arch.flat_foreign(),
            ports: arch.flat_ports(),
            operand_bytes: arch.operand_bytes(),
            loop_alignment: arch.pref_loop_alignment(),
            call_register_capacity: arch.callee_saved().len() as i64,
            float_register_capacity: arch.float_stack() as i64,
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
            optimized: true,
            aggressive: false,
            float_store: false,
            rounds_float_spills: false,
            stable_loads: true,
            min_size: false,
            spills_extended: false,
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

    /// Clocks a byte of code is worth, in thousandths, where code is tuned
    /// for size (the target's, none elsewhere).
    pub fn size_milliclocks_per_byte(&self) -> i64 {
        if self.size || self.min_size { self.target().size_milliclocks_per_byte() } else { 0 }
    }

    /// `TuningSlow3OpsLEA`: a three-operand LEA takes three cycles, so
    /// X86FixupLEAs splits it. Of the nine `-march`es only `x86-64`, tuned as
    /// clang and llc tune it by default, `generic` (X86.td L1903): not
    /// `TuningSlowIncDec`, which only `X86_64V1Tuning` carries.
    pub fn slow_3ops_lea(&self) -> bool {
        self.name == "x86-64"
    }

    /// The cheaper way to double a register: `add r,r` or `shl r,1`, which
    /// set the same flags. The 386 and 486 take three clocks for the D1 shift.
    pub fn doubling(&self) -> Result<&'static str, String> {
        Ok(if self.cost("alu_rr")? < self.cost("shift_r1")? { "alu_rr" } else { "shift_r1" })
    }

    /// Whether clearing a register and loading its low byte (`xor r,r; mov
    /// rl,m`) is faster than `movzx r,m8`: the 486 takes three clocks for
    /// the extension and two for the pair; a CPU that stalls on the partial
    /// register, or that extends in one clock, does not.
    pub fn clears_before_byte_load(&self) -> Result<bool, String> {
        Ok(!self.size
            && self.cost("alu_rr")? + self.cost("mov_rm")? + self.partial_register_stall < self.cost("movzx")?)
    }

    /// Whether `and r,255` and `and r,65535` are better as the `movzx` of the
    /// low byte or word (X86InstrCompiler.td): shorter, so for size; for
    /// clocks only where the extension is no slower than the `and`. The 486
    /// and the Pentium take three clocks for it and one for the `and`.
    pub fn masks_by_extension(&self) -> Result<bool, String> {
        Ok(self.size || self.cost("movzx")? <= self.cost("alu_rr")?)
    }

    /// Whether `words` pops into a dead register clean a call's arguments
    /// off the stack in place of `add sp,2*words`: shorter for one or two
    /// words, and where size is not wanted, only if no slower.
    ///
    /// DEVIATE PO1 (owner w10, 2026-10-30): the pin pops only at `minsize`
    /// (`X86FrameLowering::eliminateCallFramePseudoInstr`,
    /// X86FrameLowering.cpp:4116, `F.hasMinSize() && adjustStackWithPops`; pin
    /// llc -mcpu=i486 keeps `add esp,8` at `optsize`). Here -Os pops too: one
    /// byte shorter per two-word site, two memory operands more (qmove bas
    /// -Os memops 32 against 29, bytes equal). Main 65098fd950 has this same
    /// function, so no row moves against main. Exit: -Os is at or below main
    /// without it, then this is `self.min_size` only.
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
    let mut operations = arch.operation_costs(&|form| costs[form], table.prefix);
    operations.pieces_set = table.set_ratio.unwrap_or(0);
    operations.pieces_clear = table.clear_ratio.unwrap_or(0);
    operations.pieces_set_size = table.set_ratio_size.unwrap_or(0);
    operations.pieces_clear_size = table.clear_ratio_size.unwrap_or(0);
    operations.jump_table = table.jump_table.unwrap_or(0);
    operations.cache_line = table.cache_line.unwrap_or(0);
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
    std::sync::Mutex<
        crate::support::hash::HashMap<
            (&'static str, String, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool),
            &'static Profile,
        >,
    >,
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
    tuned_optimizing(arch, name, size, search, exhaustive, true, false, false, false, false, true, false)
}

/// `tuned_with`, the backend's optional phases left out where not
/// `optimized` (-O0).
pub fn tuned_optimizing(
    arch: &dyn Target,
    name: &str,
    size: bool,
    search: bool,
    exhaustive: bool,
    optimized: bool,
    float_store: bool,
    aggressive: bool,
    rounds_float_spills: bool,
    spills_extended: bool,
    stable_loads: bool,
    min_size: bool,
) -> Result<&'static Profile, String> {
    if !arch.cpus().contains(&name) {
        return Err(format!("unknown CPU target: {name}; {} has {}", arch.name(), arch.cpus().join(", ")));
    }
    let mut made = _MADE.lock().expect("the profiles are not poisoned");
    let key = (
        arch.name(),
        name.to_owned(),
        size,
        search,
        exhaustive,
        optimized,
        float_store,
        aggressive,
        rounds_float_spills,
        spills_extended,
        stable_loads,
        min_size,
    );
    if let Some(&one) = made.get(&key) {
        return Ok(one);
    }
    let one: &'static Profile = Box::leak(Box::new(Profile {
        size,
        search,
        exhaustive,
        optimized,
        float_store,
        aggressive,
        rounds_float_spills,
        spills_extended,
        stable_loads,
        min_size,
        ..(_profile(arch, name)).expect("every listed CPU has a profile")
    }));
    made.insert(key, one);
    Ok(one)
}

/// The registers a value may be placed in: the description's `gpr` class.
fn general_registers(arch: &dyn Target) -> Vec<RegId> {
    let file = llrm_target::registers::parse(&arch.registers_text()).expect("the target's registers parse");
    llrm_target::registers::of_class(&file, "gpr")
        .into_iter()
        .map(|name| {
            RegId::values()
                .find(|one| format!("{one:?}").eq_ignore_ascii_case(name))
                .unwrap_or_else(|| panic!("no register {name}"))
        })
        .collect()
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

    /// The passes' view of the target (`Profile::target`, then the QB layer's
    /// wrapper) dropped the depth of the floating stack: hoist saw no class and
    /// held 16 matrix values across a loop on a stack of 8.
    #[test]
    fn test_a_profiles_target_states_the_depth_of_its_floating_stack() {
        for name in names() {
            let target = profile(name).unwrap();
            assert_eq!(target.target().float_registers(), 8, "{name}");
            assert_eq!(
                target.operations.float_load_folded,
                target.cost("x87_add_m").unwrap() - target.cost("x87_add").unwrap(),
                "{name}"
            );
        }
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
            hosted: false,
        };
        let target = crate::abi::qb::LoweredTarget::of(profile("486").unwrap(), abi);
        assert_eq!((target.costs().call, target.size_costs().call), (18, 5));
    }

    /// The target every frontend lowers to states its LLVM cost model: the
    /// flat 32-bit target's and real mode's. `tti` was not forwarded, so
    /// `LLRM_EXP_INLINEDECIDE` priced with the generic rules whatever the
    /// target said (the rows with and without the model were the same run).
    #[test]
    fn test_the_lowered_target_forwards_its_cost_model() {
        use llrm_mir::target::Machine;
        let abi = || crate::abi::qb::HirAbi {
            runtime: crate::hir::model::RuntimeProfile::Freestanding,
            objects: Default::default(),
            preserved: Default::default(),
            stack_check: None,
            hosted: false,
        };
        let flat: &'static Profile = Box::leak(Box::new(Profile::new(&llrm_x86_m32::M32, "486", 1, true, 1, 0)));
        assert!(crate::abi::qb::LoweredTarget::of(flat, abi()).tti().is_some());
        assert!(crate::abi::qb::LoweredTarget::of(profile("486").unwrap(), abi()).tti().is_some());
    }

    /// The target every frontend lowers to states its C library: real mode's
    /// `_sin@3`. `library` was not forwarded, so no declaration was a library
    /// function's and viewfx called `sin` 13 times where the pin calls 4.
    #[test]
    fn test_the_lowered_target_forwards_its_library() {
        use llrm_mir::target::Machine;
        let abi = crate::abi::qb::HirAbi {
            runtime: crate::hir::model::RuntimeProfile::Freestanding,
            objects: Default::default(),
            preserved: Default::default(),
            stack_check: None,
            hosted: false,
        };
        let library = crate::abi::qb::LoweredTarget::of(profile("486").unwrap(), abi).library().expect("states one");
        assert_eq!(library.standard("_sin@3"), "sin");
    }

    /// The target every frontend lowers to states X86's `TargetLowering`
    /// facts on both word sizes. `lowering` was not forwarded (and `mask_test`
    /// priced the sink of a masked compare): the sink never ran on QB's
    /// target once the price was gone.
    #[test]
    fn test_the_lowered_target_forwards_its_lowering_facts() {
        use llrm_mir::target::Machine;
        let abi = || crate::abi::qb::HirAbi {
            runtime: crate::hir::model::RuntimeProfile::Freestanding,
            objects: Default::default(),
            preserved: Default::default(),
            stack_check: None,
            hosted: false,
        };
        let flat: &'static Profile = Box::leak(Box::new(Profile::new(&llrm_x86_m32::M32, "486", 1, true, 1, 0)));
        for cpu in [flat, profile("486").unwrap()] {
            let lowering = crate::abi::qb::LoweredTarget::of(cpu, abi()).lowering().expect("x86 states them");
            assert!(lowering.is_mask_and_cmp0_folding_beneficial());
        }
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
/// that may take it. `instrument` (`LLRM_PRIVATE_WATCALL`, the CC1 comparison)
/// gives it the description's `private_instrument` instead, where it states
/// one.
fn private_convention(
    arch: &dyn Target,
    instrument: bool,
) -> Option<llrm_mir::target::PrivateConvention> {
    let calling = arch.calling();
    let private = calling.private()?;
    let private = calling.private_instrument().filter(|_| instrument).unwrap_or(private);
    let to = llrm_target::calling::Calling::number_of(private.cc.as_deref())?;
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

#[cfg(test)]
mod private_convention_tests {
    use super::*;

    /// CC1: a private m32 function takes the pin's `fastcc` (ECX, EDX; the
    /// caller pops); `LLRM_PRIVATE_WATCALL` (the comparison instrument) gives
    /// it `watcall32` instead. A default of `watcall32` priced the rows against
    /// the pin's convention and lost -O1..-O3 clocks (`rectwo` -9.6%).
    #[test]
    fn test_a_private_function_takes_the_pin_fastcc_and_the_instrument_watcall32() {
        use llrm_mir::opcode::{FAST, WATCALL};
        let ours = private_convention(&llrm_x86_m32::M32, false).expect("m32 states one");
        let old = private_convention(&llrm_x86_m32::M32, true).expect("m32 states one");
        assert_eq!((ours.to, old.to), (FAST, WATCALL));
        let found = llrm_x86_m32::M32.calling().by_number(FAST).expect("fastcc is stated");
        assert_eq!(found.argument_registers, ["ecx", "edx"]);
        assert_eq!(found.cleanup, llrm_target::calling::Cleanup::Caller);
    }
}

#[cfg(test)]
mod zeroed_load_tests {
    use super::*;

    /// The 486 and Pentium take three clocks for `movzx r,m8` and two for the
    /// pair; the 386 (loads cost four), the Pentium Pro and later (one-clock
    /// extension, a partial-register stall) do not. Priced from the CPU's
    /// description, not named.
    #[test]
    fn test_the_pair_is_chosen_where_the_description_prices_it_lower() {
        for (name, pair) in [("386", false), ("486", true), ("P5", true), ("P6", false), ("Core", false)] {
            let cpu = profile(name).expect("a cpu");
            assert_eq!(cpu.clears_before_byte_load(), Ok(pair), "{name}");
        }
    }
}

#[cfg(test)]
mod mask_tests {
    use super::*;

    /// `and r,255` as `movzx r,al` is three clocks against one on the 386,
    /// 486 and Pentium, so it is chosen there only for size; a one-clock
    /// extension (Pentium Pro and later) makes it free. Priced from the
    /// description, not named.
    #[test]
    fn test_the_extension_replaces_a_mask_for_size_or_where_it_is_no_slower() {
        for (name, fast) in [("386", false), ("486", false), ("P5", false), ("P6", true)] {
            let mut cpu = profile(name).expect("a cpu").clone();
            assert_eq!(cpu.masks_by_extension(), Ok(fast), "{name}");
            cpu.size = true;
            assert_eq!(cpu.masks_by_extension(), Ok(true), "{name} for size");
        }
    }
}
