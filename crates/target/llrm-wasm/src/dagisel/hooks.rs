//! `WebAssemblyDAGToDAGISel` as the interpreter's [`Hooks`]: the address
//! patterns the table names by number, fold legality and the node predicates.

use llrm_dagisel::dag::{Hooks, PatternContext, SDValue};
use llrm_dagisel::generated::ids::mvt;
use llrm_dagisel::matcher::Vt;
use llrm_dagisel::x86_addr::AddrDag;
use llrm_dagisel::x86_hooks::find_non_imm_use;

use super::addr::{self, ADDR32, ADDR64};
use super::generated::{instrinfo, wasm_wasm32, wasm_wasm64};

/// The CPUs the tables are generated for; they differ in the address width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cpu {
    Wasm32,
    Wasm64,
}

/// No pattern predicate of the wasm tables is decided at selection time: the
/// features are baked into each table.
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

pub struct WasmHooks {
    cpu: Cpu,
    /// `OptLevel == CodeGenOptLevel::None`: nothing folds.
    opt_none: bool,
}

impl WasmHooks {
    pub fn new(
        cpu: Cpu,
        opt_none: bool,
    ) -> WasmHooks {
        WasmHooks { cpu, opt_none }
    }
}

impl<D: AddrDag> Hooks<D> for WasmHooks {
    fn pointer_type(&self) -> Vt {
        match self.cpu {
            Cpu::Wasm32 => mvt::i32,
            Cpu::Wasm64 => mvt::i64,
        }
    }

    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool {
        match self.cpu {
            Cpu::Wasm32 => wasm_wasm32::pattern_predicate(&NoDynamicPredicates, predicate),
            Cpu::Wasm64 => wasm_wasm64::pattern_predicate(&NoDynamicPredicates, predicate),
        }
    }

    fn check_node_predicate(
        &self,
        dag: &D,
        value: SDValue,
        predicate: u16,
    ) -> bool {
        match self.cpu {
            Cpu::Wasm32 => wasm_wasm32::node_predicate(dag, self, value, predicate),
            Cpu::Wasm64 => wasm_wasm64::node_predicate(dag, self, value, predicate),
        }
    }

    /// `SelectAddrOperands32` is pattern 2 and `SelectAddrOperands64` pattern
    /// 3; each pushes the offset, then the base.
    fn check_complex_pattern(
        &mut self,
        dag: &mut D,
        _root: u32,
        _parent: Option<u32>,
        value: SDValue,
        pattern: u8,
        results: &mut Vec<(SDValue, Option<u32>)>,
    ) -> bool {
        let width = match (self.cpu, pattern) {
            (Cpu::Wasm32, 2) => ADDR32,
            (Cpu::Wasm64, 3) => ADDR64,
            _ => panic!("a complex pattern the {:?} table does not name: {pattern}", self.cpu),
        };
        let (offset, base) = addr::select_addr_operands(dag, width, value);
        results.extend([(offset, None), (base, None)]);
        true
    }

    /// The wasm tables run no node transform.
    fn run_xform(
        &mut self,
        _dag: &mut D,
        _value: SDValue,
        xform: u8,
    ) -> SDValue {
        panic!("the wasm tables have no xform, asked for {xform}")
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
