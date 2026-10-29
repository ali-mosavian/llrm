//! The matcher's hooks for arm64 (`AArch64DAGToDAGISel`'s callbacks): the
//! complex patterns, the node transforms and the fold checks.
//!
//! # How it works
//! `COMPLEX_PATTERNS` names the pin's function of each number; the hooks
//! resolve it once into [`Complex`], and `check_complex_pattern` calls the
//! translation in [`super::addr`]. The node and pattern predicates are the
//! generated ones.
//!
//! DEVIATE: `SelectAddrModeWRO` (a register offset with an extend) answers
//! false, and so do the complex patterns `addr` lists as not translated.
//! Reason: the first loads and stores need none; the table has the XRO and
//! the base forms. Exit: B4.

use llrm_dagisel::aarch64_predicates::encode_logical_immediate;
use llrm_dagisel::dag::{Hooks, PatternContext, SDValue};
use llrm_dagisel::generated::aarch64::aarch64_apple_m1 as table;
use llrm_dagisel::generated::ids::mvt;
use llrm_dagisel::matcher::Vt;
use llrm_dagisel::x86_addr::AddrDag;
use llrm_dagisel::x86_hooks::find_non_imm_use;

use super::addr;

#[derive(Clone, Copy)]
enum Complex {
    ArithImmed,
    NegArithImmed,
    ArithShiftedRegister,
    LogicalShiftedRegister,
    Indexed(u32),
    Unscaled,
    Xro(u32),
    Never,
}

/// The size in bytes of `Select...<Width>` or `Select...Width`.
fn size_of_name(name: &str) -> u32 {
    let digits: String = name.chars().filter(char::is_ascii_digit).collect();
    digits.parse::<u32>().expect("a width in the name") / 8
}

/// What `SelectionDAGISel` asks the subtarget at selection time: nothing for
/// the apple-m1 table.
struct NoContext;

impl PatternContext for NoContext {
    fn should_opt_for_size(&self) -> bool {
        false
    }
    fn has_min_size(&self) -> bool {
        false
    }
    fn has_frame_pointer(&self) -> bool {
        true
    }
    fn is_legal_to_call_immediate_addr(&self) -> bool {
        false
    }
}

pub struct Aarch64Hooks {
    complex: Vec<Complex>,
}

impl Default for Aarch64Hooks {
    fn default() -> Aarch64Hooks {
        let patterns = table::COMPLEX_PATTERNS;
        let mut complex = vec![Complex::Never; patterns.iter().map(|&(n, ..)| usize::from(n) + 1).max().unwrap_or(0)];
        for &(number, function, _) in patterns {
            complex[usize::from(number)] = match function {
                "SelectArithImmed" => Complex::ArithImmed,
                "SelectNegArithImmed" => Complex::NegArithImmed,
                "SelectArithShiftedRegister" => Complex::ArithShiftedRegister,
                "SelectLogicalShiftedRegister" => Complex::LogicalShiftedRegister,
                f if f.starts_with("SelectAddrModeIndexed7S") => Complex::Never,
                f if f.starts_with("SelectAddrModeIndexed") => Complex::Indexed(size_of_name(f)),
                f if f.starts_with("SelectAddrModeUnscaled") => Complex::Unscaled,
                f if f.starts_with("SelectAddrModeXRO") => Complex::Xro(size_of_name(f)),
                f if f.starts_with("SelectAddrModeWRO") => Complex::Never,
                "SelectArithExtendedRegister"
                | "SelectArithUXTXRegister"
                | "SelectExtractHigh"
                | "SelectDupZero" => Complex::Never,
                f if f.starts_with("SelectCVTFixedPos") => Complex::Never,
                other => panic!("complex pattern `{other}` is not translated"),
            };
        }
        Aarch64Hooks { complex }
    }
}

impl<D: AddrDag> Hooks<D> for Aarch64Hooks {
    fn pointer_type(&self) -> Vt {
        mvt::i64
    }

    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool {
        table::pattern_predicate(&NoContext, predicate)
    }

    fn check_node_predicate(
        &self,
        dag: &D,
        value: SDValue,
        predicate: u16,
    ) -> bool {
        table::node_predicate(dag, self, value, predicate)
    }

    fn check_complex_pattern(
        &mut self,
        dag: &mut D,
        _root: u32,
        _parent: Option<u32>,
        value: SDValue,
        pattern: u8,
        results: &mut Vec<(SDValue, Option<u32>)>,
    ) -> bool {
        let mut push = |values: &[SDValue]| results.extend(values.iter().map(|&v| (v, None)));
        match self.complex[usize::from(pattern)] {
            Complex::ArithImmed => addr::arith_immed(dag, value).map(|v| push(&v)).is_some(),
            Complex::NegArithImmed => addr::neg_arith_immed(dag, value).map(|v| push(&v)).is_some(),
            Complex::ArithShiftedRegister => addr::shifted_register(dag, value, false).map(|v| push(&v)).is_some(),
            Complex::LogicalShiftedRegister => addr::shifted_register(dag, value, true).map(|v| push(&v)).is_some(),
            Complex::Indexed(size) => addr::addr_mode_indexed(dag, value, size).map(|v| push(&v)).is_some(),
            Complex::Unscaled => addr::addr_mode_unscaled(dag, value).map(|v| push(&v)).is_some(),
            Complex::Xro(size) => addr::addr_mode_xro(dag, value, size).map(|v| push(&v)).is_some(),
            Complex::Never => false,
        }
    }

    /// `RunSDNodeXForm`: `XFORMS` has each body.
    fn run_xform(
        &mut self,
        dag: &mut D,
        value: SDValue,
        xform: u8,
    ) -> SDValue {
        let n = || addr::zext(dag, value).expect("an xform of a constant");
        let signed = || dag.constant(value).expect("an xform of a constant");
        let (constant, vt) = match xform {
            // `V`: the operand as it is.
            2 | 29 | 31 => return value,
            0 => (i64::from(n() as u32), mvt::i32),
            1 => (signed().wrapping_neg(), dag.value_type(value)),
            9 | 10 => {
                let width = if xform == 9 { 32 } else { 64 };
                let encoded = encode_logical_immediate(n(), width).expect("a logical immediate the predicate accepted");
                (encoded as i64, mvt::i32)
            }
            19 => ((32 - n() as i64) & 0x1f, mvt::i64),
            20 => ((31 - n() as i64).min(7), mvt::i64),
            21 => ((64 - n() as i64) & 0x3f, mvt::i64),
            22 => ((63 - n() as i64).min(7), mvt::i64),
            23 => ((31 - n() as i64).min(15), mvt::i64),
            24 => ((63 - n() as i64).min(15), mvt::i64),
            25 => ((63 - n() as i64).min(31), mvt::i64),
            26 => (31 - n() as i64, mvt::i64),
            27 => (63 - n() as i64, mvt::i64),
            // `getInvertedCondCode`: the condition codes pair up on the low bit.
            28 => (n() as i64 ^ 1, mvt::i32),
            30 => ((n() | (u64::from(b'U') << 8)) as i64, mvt::i32),
            41 => (8 + n() as i64, mvt::i32),
            other => panic!("xform {other} is not translated"),
        };
        dag.target_constant(constant, vt)
    }

    /// `SelectionDAGISel::IsProfitableToFold`: always, unless unoptimized.
    fn is_profitable_to_fold(
        &self,
        _: &D,
        _: SDValue,
        _: u32,
        _: u32,
    ) -> bool {
        true
    }

    /// `SelectionDAGISel::IsLegalToFold`.
    fn is_legal_to_fold(
        &self,
        dag: &D,
        value: SDValue,
        user: u32,
        root: u32,
    ) -> bool {
        let mut root = root;
        let mut ignore_chains = true;
        while dag.value_type(SDValue::new(root, (dag.num_values(root) - 1) as u8)) == mvt::Glue {
            let Some(glued) = dag.glued_user(root) else { break };
            root = glued;
            ignore_chains = false;
        }
        !find_non_imm_use(dag, root, value.node, user, ignore_chains)
    }

    /// `AArch64DAGToDAGISel::CheckAndMask`: the table names no pattern that asks.
    fn check_and_mask(
        &self,
        _: &D,
        _: SDValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }

    fn check_or_mask(
        &self,
        _: &D,
        _: SDValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }

    fn may_load_store(
        &self,
        opcode: u16,
    ) -> (bool, bool) {
        llrm_dagisel::generated::aarch64::instrinfo::memory_flags(opcode)
    }
}
