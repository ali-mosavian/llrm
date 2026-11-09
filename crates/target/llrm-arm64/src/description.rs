//! arm64 as a `llrm_target::Target`: what the shared backend asks of a target.
//! Linux (AAPCS64, ELF) only; Darwin follows with its own data files.
//!
//! Hooks answered from a data file read it; the rest are stubs (STUB in the
//! doc), each owned by w15 until its stage runs on arm64
//! (docs/architecture/arm64-target-description.md).

use std::rc::Rc;
use std::sync::LazyLock;

use llrm_lir::registers::RegId;
use llrm_mir::target::{AddressForm, Machine as _, OperationCosts};
use llrm_target::calling::{Calling, Convention};
use llrm_target::{CostModel, Target};

use crate::machine::{Arm64Machine, Platform};
use crate::{REGISTER_INFO, reg};

static CALLING: LazyLock<Calling> =
    LazyLock::new(|| Calling::parse(include_str!("machines/calling.toml")).expect("calling.toml parses"));

static CONVENTIONS: LazyLock<Vec<&'static str>> = LazyLock::new(|| CALLING.names());

static LAYOUT: LazyLock<llrm_target::layout::Layout> = LazyLock::new(|| {
    llrm_target::layout::Layout::parse(Platform::Linux.layout_description()).expect("datalayout.toml parses")
});

static OBJECT: LazyLock<llrm_target::object::ObjectFormat> = LazyLock::new(|| {
    llrm_target::object::ObjectFormat::parse(Platform::Linux.object_description()).expect("object.toml parses")
});

static DIALECT: LazyLock<llrm_target::instr::Dialect> = LazyLock::new(|| {
    llrm_target::instr::Dialect::parse(include_str!("instructions/dialect.toml")).expect("dialect.toml parses")
});

/// A Linux user program: flat, no ports.
const LINUX: &str = include_str!("../../../../machine/platform/linux/flat64.toml");

const CPUS: [&str; 2] = ["generic", "apple-m1"];

/// The arm64 Linux target.
pub struct Arm64;

fn register(name: &str) -> RegId {
    REGISTER_INFO.by_name(name).unwrap_or_else(|| panic!("calling.toml names no arm64 register {name}"))
}

impl Target for Arm64 {
    fn name(&self) -> &'static str {
        "arm64"
    }

    fn objects(&self) -> &'static dyn llrm_debug::DebugObjectTarget {
        &crate::mc::Arm64Objects
    }

    fn debug_target(&self) -> llrm_debug::DebugTarget {
        crate::debug::machine().target
    }

    fn machine(&self) -> llrm_target::machine::Machine {
        llrm_target::machine::Machine::parse(LINUX, self.default_cpu())
            .expect("the Linux description parses")
            .with_layout(self.layout())
    }

    /// The pin selects arm64 through the DAG; the private driver does too.
    fn selects_by_dag(&self) -> bool {
        true
    }

    fn cpus(&self) -> &'static [&'static str] {
        &CPUS
    }

    fn layout(&self) -> llrm_target::layout::Layout {
        LAYOUT.clone()
    }

    fn address_roles(&self) -> llrm_mir::spaces::Spaces {
        LAYOUT.spaces.roles
    }

    fn stack_slot_bytes(&self) -> i64 {
        CALLING.native().slot_bytes
    }

    fn stack_alignment(&self) -> i64 {
        CALLING.native().stack_alignment.unwrap_or(CALLING.native().slot_bytes)
    }

    fn aligned_calls(&self) -> bool {
        CALLING.native().aligned_calls
    }

    fn frame_register(&self) -> RegId {
        register(&CALLING.native().frame)
    }

    fn first_argument_offset(
        &self,
        _far: bool,
    ) -> i64 {
        CALLING.native().first_argument_offset
    }

    fn return_address_bytes(
        &self,
        _far: bool,
    ) -> i64 {
        CALLING.native().return_address_bytes
    }

    fn registers(&self) -> &'static llrm_lir::registers::Info {
        &REGISTER_INFO
    }

    fn results(
        &self,
        width: u32,
    ) -> Vec<RegId> {
        self.convention_results(CALLING.native(), width)
    }

    fn stack_pointer(&self) -> RegId {
        register(&CALLING.native().stack)
    }

    fn callee_saved(&self) -> Vec<(RegId, RegId)> {
        self.convention_callee_saved(CALLING.native())
    }

    fn register_named(
        &self,
        name: &str,
    ) -> RegId {
        register(name)
    }

    fn convention_results(
        &self,
        convention: &Convention,
        width: u32,
    ) -> Vec<RegId> {
        convention
            .result_registers(i64::from(width))
            .expect("calling.toml states a result for every width")
            .iter()
            .map(|name| register(name))
            .collect()
    }

    fn convention_callee_saved(
        &self,
        convention: &Convention,
    ) -> Vec<(RegId, RegId)> {
        convention.callee_saved().into_iter().map(|kept| (register(&kept.full), register(&kept.pushed))).collect()
    }

    fn registers_text(&self) -> String {
        include_str!("registers.regs").to_owned()
    }

    /// The forms written so far (`arm64.instr`); the selector's ~250 follow.
    fn forms_text(&self) -> String {
        include_str!("instructions/arm64.instr").to_owned()
    }

    fn dialect(&self) -> &'static llrm_target::instr::Dialect {
        &DIALECT
    }

    fn operand_bytes(&self) -> i64 {
        8
    }

    /// Linux links glibc's libm.
    fn has_libm(&self) -> bool {
        true
    }

    /// The fixed-width encoding: every instruction is 4 bytes.
    fn register_op_bytes(
        &self,
        _width: i64,
    ) -> i64 {
        4
    }

    fn shift_op_bytes(
        &self,
        _count: i64,
        _width: i64,
    ) -> i64 {
        4
    }

    /// STUB: a `mov` of the constant and a `mul`; a shift-and-add chain is not
    /// priced.
    fn multiply_immediate_bytes(
        &self,
        _number: i64,
        _width: i64,
    ) -> i64 {
        8
    }

    /// Issue width and ordering from the pin's scheduling models: `generic` is
    /// Cortex-A510's, in order and three wide
    /// ([AArch64SchedA510.td#L19](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64SchedA510.td#L19)),
    /// `apple-m1` is Cyclone's, out of order and six wide
    /// ([AArch64SchedCyclone.td#L15](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64SchedCyclone.td#L15)).
    /// No clocks: arm64 prices through `cost_model` (tti.rs), so a pass that
    /// asks the profile for a form's clocks is told it has none.
    fn cpu_table(
        &self,
        name: &str,
    ) -> Option<llrm_target::timings::CpuTable> {
        let (issue, in_order) = match name {
            "generic" => (3, true),
            "apple-m1" => (6, false),
            _ => return None,
        };
        Some(llrm_target::timings::CpuTable { issue, in_order, ..Default::default() })
    }

    fn default_cpu(&self) -> &'static str {
        Platform::Linux.cpu()
    }

    /// A `nop` is one 4-byte instruction.
    fn max_nop_bytes(&self) -> u8 {
        4
    }

    /// There is no `bsf` to prefix.
    fn bsf_rep_prefix(&self) -> bool {
        false
    }

    /// The mid-end's prices, which the private driver reads too: one source.
    fn operation_costs(
        &self,
        _price: &dyn Fn(&str) -> i64,
        _prefix: i64,
    ) -> OperationCosts {
        Arm64Machine { platform: Platform::Linux }.costs()
    }

    fn register_capacity(&self) -> i64 {
        Arm64Machine { platform: Platform::Linux }.registers()
    }

    fn address_forms(
        &self,
        _: &OperationCosts,
        _: i64,
    ) -> Vec<AddressForm> {
        Arm64Machine { platform: Platform::Linux }.address_forms()
    }

    fn cost_model(&self) -> CostModel {
        |_| Rc::new(Arm64Machine { platform: Platform::Linux })
    }

    fn conventions(&self) -> &'static [&'static str] {
        &CONVENTIONS
    }

    fn calling(&self) -> &'static Calling {
        &CALLING
    }

    fn object(&self) -> llrm_target::object::ObjectFormat {
        OBJECT.clone()
    }

    fn callee_pops(&self) -> bool {
        CALLING.conventions.iter().any(|one| one.cleanup == llrm_target::calling::Cleanup::Callee)
    }

    fn frame_optional(&self) -> bool {
        CALLING.native().frame_optional
    }

    fn frame_red_zone(&self) -> i64 {
        CALLING.native().frame_red_zone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The existing data files must read under `llrm_target`'s schemas, or the
    /// shared stages would see a different arm64 than the private driver.
    #[test]
    fn the_machine_tomls_parse_under_llrm_target() {
        for text in [include_str!("machines/datalayout.toml"), include_str!("machines/datalayout-darwin.toml")] {
            llrm_target::layout::Layout::parse(text).expect("datalayout");
        }
        for text in [include_str!("machines/object.toml"), include_str!("machines/object-darwin.toml")] {
            llrm_target::object::ObjectFormat::parse(text).expect("object");
        }
        assert_eq!(Arm64.conventions(), ["aapcs64"]);
    }

    /// AAPCS64's frame is x29 over sp, x19-x28 are kept (x29 is the frame), a
    /// 16-byte stack at calls, no return address on the stack, no callee pop.
    #[test]
    fn the_target_states_aapcs64s_roles() {
        assert_eq!((Arm64.frame_register(), Arm64.stack_pointer()), (reg::X29, reg::SP));
        let (roots, pushed): (Vec<RegId>, Vec<RegId>) = Arm64.callee_saved().into_iter().unzip();
        assert_eq!(roots, pushed);
        assert_eq!(roots, (19..=28).map(|n| register(&format!("x{n}"))).collect::<Vec<_>>());
        assert_eq!((Arm64.stack_slot_bytes(), Arm64.stack_alignment()), (8, 16));
        assert_eq!((Arm64.return_address_bytes(false), Arm64.first_argument_offset(false)), (0, 16));
        assert!(Arm64.aligned_calls() && !Arm64.callee_pops());
        // A `nop` is one 4-byte instruction: padding is whole words, not x86's
        // 1-byte default.
        assert_eq!(Arm64.max_nop_bytes(), 4);
        assert_eq!(Arm64.results(8), [reg::X0]);
        assert_eq!(Arm64.results(16), [reg::X0, register("x1")]);
    }

    /// The frame the shared prologue builds from is AAPCS64's.
    #[test]
    fn the_frame_registers_are_aapcs64s() {
        let frame = Arm64.frame_registers();
        assert_eq!((frame.pointer, frame.stack, frame.slot, frame.align), (reg::X29, reg::SP, 8, 16));
        assert_eq!((frame.red_zone, frame.optional), (0, false));
    }

    /// `RegisterClasses::of` needed a 16-bit view of every base and index root
    /// (panic "a register has a word view") and named registers through x86's
    /// table ("no register x0"). arm64 addresses through whole registers.
    #[test]
    fn the_class_table_builds_for_arm64() {
        let classes = llrm_codegen::backend::classes::RegisterClasses::of(&Arm64);
        let general: std::collections::BTreeSet<RegId> =
            (0..=30).map(|n| register(&format!("x{n}"))).chain([reg::SP]).collect();
        assert_eq!(classes.addressing, general);
        assert!(classes.encodable_bases.is_subset(&general) && !classes.encodable_bases.contains(&reg::XZR));
        assert_eq!(classes.frame, reg::X29);
        let names: Vec<&str> = classes.available.iter().map(|one| REGISTER_INFO.name(*one).unwrap()).collect();
        assert_eq!(names.first(), Some(&"x0"));
        assert!(!names.contains(&"x29") && !names.contains(&"sp") && names.contains(&"x28"), "{names:?}");
        assert!(classes.xmm.is_empty());
    }

    /// A `double` took the stack (the file named no float argument registers):
    /// it must take v0-v7 beside the integer registers and return in v0.
    #[test]
    fn aapcs64_places_floats_in_v_registers_and_returns_one_in_v0() {
        use llrm_target::calling::{Kind, Place};
        let convention = Arm64.calling().named("aapcs64").unwrap();
        let mut arguments = vec![Kind::Word, Kind::Float(8), Kind::Float(8), Kind::Word];
        arguments.extend([Kind::Float(8); 7]);
        let placement = convention.place(&arguments);
        let register = |name: &str| Place::Registers(vec![name.to_owned()]);
        assert_eq!(placement.places[..4], [register("x0"), register("v0"), register("v1"), register("x1")]);
        assert_eq!(placement.places[10], Place::Stack(0));
        assert_eq!(convention.float_result_register(), Some("v0"));
    }

    /// arm64 states none of these (no `-march` table, native divide, no huge
    /// pointers, floats at their own size): the trait's defaults answer.
    #[test]
    fn the_trait_defaults_answer_what_arm64_does_not_state() {
        assert_eq!((Arm64.march("armv8"), Arm64.marches().len(), Arm64.cpu_table("generic")), (None, 0, None));
        // An unsupported default is loud: the error names the target and the
        // method.
        let refusal = |text: Result<_, String>| -> String { text.map(|_: ()| ()).unwrap_err() };
        assert_eq!(
            refusal(Arm64.divide_routine(true, true).map(drop)),
            "arm64: divide_routine: the target states no inline divide"
        );
        assert!(refusal(Arm64.huge_fill_routine(4).map(drop)).starts_with("arm64: huge_fill_routine"));
        assert!(refusal(Arm64.huge_copy_routine(false).map(drop)).starts_with("arm64: huge_copy_routine"));
        assert_eq!((Arm64.float_width(4), Arm64.float_width(8)), (4, 8));
    }

    /// `flow::machine_with` stopped at its CPU profile (no `cpu_table`:
    /// "unknown CPU target"), then at `cost("movzx")`, then at
    /// `Segments::of` asking a flat machine for a data segment. The
    /// pipeline is now built from arm64's own inputs; a profile without
    /// clocks says so by name.
    #[test]
    fn machine_with_builds_for_arm64_and_a_missing_clock_is_named() {
        use llrm_codegen::backend::{classes::RegisterClasses, cpu::tuned_for, peep::Rules, target::Segments};
        let profile = tuned_for(&Arm64, "generic", false).expect("generic has a profile");
        assert_eq!((profile.issue_width, profile.in_order), (3, true));
        assert!(profile.cost("movzx").unwrap_err().contains("has no cost for movzx"));
        let regs = llrm_lir::registers::Regs(Arm64.registers());
        let segments = Segments::of(&Arm64.machine(), regs, Arm64.operand_bytes());
        assert_eq!(segments.data, None);
        let phases = llrm_codegen::flow::machine_with(
            &Default::default(),
            None,
            None,
            None,
            false,
            profile,
            &segments,
            &std::rc::Rc::new(RegisterClasses::of(&Arm64)),
            &Rules::NONE,
            &Arm64.frame_registers(),
            llrm_codegen::backend::shadow::Selector::Old,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let names: Vec<&str> = phases.iter().map(|phase| phase.class_name()).collect();
        println!("{} phases: {}", names.len(), names.join(" "));
    }

    /// The shared passes' prices are the private driver's: one `Arm64Machine`.
    #[test]
    fn the_cost_model_is_the_private_drivers() {
        let machine = Arm64Machine { platform: Platform::Linux };
        assert_eq!(Arm64.register_capacity(), machine.registers());
        assert_eq!(Arm64.address_forms(&OperationCosts::default(), 0).len(), machine.address_forms().len());
    }
}
