//! The 6502's DAG-to-DAG hooks as the interpreter's [`Hooks`]: the one complex pattern the table
//! names, fold legality and the node predicates.
//!
//! # How it works
//! `SelectIndirect` is the `(zp),Y` address: a pointer in an `Imag16` pair and a byte offset for Y.
//! `(add ptr, c)` with `0 <= c < 256` gives `ptr` and `c`; any other address is its own pointer at
//! offset 0.

use llrm_dagisel::dag::{Dag, Hooks, PatternContext, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::matcher::Vt;
use llrm_dagisel::x86_addr::AddrDag;
use llrm_dagisel::x86_hooks::find_non_imm_use;

use super::generated::{ids::isd as own, instrinfo, mos_mos6502, mos_w65c02};

/// The CPUs the tables are generated for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cpu {
    Mos6502,
    W65c02,
}

/// No pattern predicate of the 6502 tables is decided at selection time.
struct NoDynamicPredicates;

impl PatternContext for NoDynamicPredicates {
    fn should_opt_for_size(&self) -> bool {
        false
    }
    fn has_min_size(&self) -> bool {
        false
    }
    fn has_frame_pointer(&self) -> bool {
        false
    }
    fn is_legal_to_call_immediate_addr(&self) -> bool {
        false
    }
}

pub struct MosHooks {
    cpu: Cpu,
    /// `OptLevel == CodeGenOptLevel::None`: nothing folds.
    opt_none: bool,
}

impl MosHooks {
    pub fn new(
        cpu: Cpu,
        opt_none: bool,
    ) -> MosHooks {
        MosHooks { cpu, opt_none }
    }
}

/// `SelectIndirect`: the pointer and the byte offset.
fn select_indirect<D: Dag>(
    dag: &mut D,
    value: SDValue,
    results: &mut Vec<(SDValue, Option<u32>)>,
) {
    let (base, offset) = match (dag.opcode(value.node) == isd::ADD).then(|| dag.constant(dag.operand(value.node, 1))) {
        Some(Some(c)) if (0..256).contains(&c) => (dag.operand(value.node, 0), c),
        _ => (value, 0),
    };
    let offset = dag.target_constant(offset, mvt::i8);
    results.push((base, None));
    results.push((offset, None));
}

/// `SelectAbsX`: `(add (wrapper global), (zext i8 index))` as the symbol and the byte index, either
/// operand order.
fn select_absx<D: Dag>(
    dag: &mut D,
    value: SDValue,
    results: &mut Vec<(SDValue, Option<u32>)>,
) -> bool {
    if dag.opcode(value.node) != isd::ADD {
        return false;
    }
    let operands = [dag.operand(value.node, 0), dag.operand(value.node, 1)];
    for (wrapper, extend) in [(operands[0], operands[1]), (operands[1], operands[0])] {
        if dag.opcode(wrapper.node) != own::WRAPPER || dag.opcode(extend.node) != isd::ZERO_EXTEND {
            continue;
        }
        let symbol = dag.operand(wrapper.node, 0);
        let index = dag.operand(extend.node, 0);
        if dag.opcode(symbol.node) == isd::TargetGlobalAddress && dag.value_type(index) == mvt::i8 {
            results.push((symbol, None));
            results.push((index, None));
            return true;
        }
    }
    false
}

impl<D: AddrDag> Hooks<D> for MosHooks {
    fn pointer_type(&self) -> Vt {
        mvt::i16
    }

    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool {
        match self.cpu {
            Cpu::Mos6502 => mos_mos6502::pattern_predicate(&NoDynamicPredicates, predicate),
            Cpu::W65c02 => mos_w65c02::pattern_predicate(&NoDynamicPredicates, predicate),
        }
    }

    fn check_node_predicate(
        &self,
        dag: &D,
        value: SDValue,
        predicate: u16,
    ) -> bool {
        match self.cpu {
            Cpu::Mos6502 => mos_mos6502::node_predicate(dag, self, value, predicate),
            Cpu::W65c02 => mos_w65c02::node_predicate(dag, self, value, predicate),
        }
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
        match pattern {
            0 => {
                select_indirect(dag, value, results);
                true
            }
            1 => select_absx(dag, value, results),
            _ => unreachable!("the table has two complex patterns"),
        }
    }

    /// The one transform of the table is `to_tframeindex`.
    fn run_xform(
        &mut self,
        dag: &mut D,
        value: SDValue,
        _xform: u8,
    ) -> SDValue {
        let index = dag.frame_index(value).expect("to_tframeindex takes a frame index");
        dag.target_frame_index(index, mvt::i16)
    }

    fn is_profitable_to_fold(
        &self,
        _dag: &D,
        _value: SDValue,
        _user: u32,
        _root: u32,
    ) -> bool {
        !self.opt_none
    }

    /// `SelectionDAGISel::IsLegalToFold`.
    fn is_legal_to_fold(
        &self,
        dag: &D,
        value: SDValue,
        user: u32,
        root: u32,
    ) -> bool {
        if self.opt_none {
            return false;
        }
        let mut root = root;
        let mut ignore_chains = true;
        while dag.value_type(SDValue::new(root, (dag.num_values(root) - 1) as u8)) == mvt::Glue {
            let Some(glued) = dag.glued_user(root) else { break };
            root = glued;
            ignore_chains = false;
        }
        !find_non_imm_use(dag, root, value.node, user, ignore_chains)
    }

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
        instrinfo::memory_flags(opcode)
    }
}
