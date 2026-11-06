//! The Z80's DAG-to-DAG hooks as the interpreter's [`Hooks`]: fold legality and
//! the node predicates. The tables name no complex pattern yet.

use llrm_dagisel::dag::{Hooks, PatternContext, SDValue};
use llrm_dagisel::generated::ids::mvt;
use llrm_dagisel::matcher::Vt;
use llrm_dagisel::x86_addr::AddrDag;
use llrm_dagisel::x86_hooks::find_non_imm_use;

use super::generated::{instrinfo, z80_z80, z80_z180};

/// The CPUs the tables are generated for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cpu {
    Z80,
    Z180,
}

/// No pattern predicate of the Z80 tables is decided at selection time.
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

pub struct Z80Hooks {
    cpu: Cpu,
    /// `OptLevel == CodeGenOptLevel::None`: nothing folds.
    opt_none: bool,
}

impl Z80Hooks {
    pub fn new(
        cpu: Cpu,
        opt_none: bool,
    ) -> Z80Hooks {
        Z80Hooks { cpu, opt_none }
    }
}

impl<D: AddrDag> Hooks<D> for Z80Hooks {
    fn pointer_type(&self) -> Vt {
        mvt::i16
    }

    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool {
        match self.cpu {
            Cpu::Z80 => z80_z80::pattern_predicate(&NoDynamicPredicates, predicate),
            Cpu::Z180 => z80_z180::pattern_predicate(&NoDynamicPredicates, predicate),
        }
    }

    fn check_node_predicate(
        &self,
        dag: &D,
        value: SDValue,
        predicate: u16,
    ) -> bool {
        match self.cpu {
            Cpu::Z80 => z80_z80::node_predicate(dag, self, value, predicate),
            Cpu::Z180 => z80_z180::node_predicate(dag, self, value, predicate),
        }
    }

    fn check_complex_pattern(
        &mut self,
        _dag: &mut D,
        _root: u32,
        _parent: Option<u32>,
        _value: SDValue,
        pattern: u8,
        _results: &mut Vec<(SDValue, Option<u32>)>,
    ) -> bool {
        unreachable!("the Z80 tables name no complex pattern, got {pattern}")
    }

    /// The one transform of the tables is `to_tframeindex`.
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
