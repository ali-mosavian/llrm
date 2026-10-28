//! `M68kDAGToDAGISel` as the interpreter's [`Hooks`]: the complex patterns the
//! table names by number, fold legality and the node predicates.

use llrm_dagisel::dag::{Hooks, PatternContext, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::matcher::Vt;
use llrm_dagisel::x86_addr::AddrDag;
use llrm_dagisel::x86_hooks::find_non_imm_use;

use super::addr;
use super::generated::ids::isd as m68k_isd;
use super::generated::{instrinfo, m68k_m68000, m68k_m68020};

/// The CPUs the tables are generated for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cpu {
    M68000,
    M68020,
}

#[derive(Clone, Copy, Debug)]
enum Complex {
    Ari,
    Arid,
    Arii,
    Pcd,
    Pci,
    Pcibd,
    Al,
    /// `SelectARIPI` and `SelectARIPD`: not implemented in the pin either.
    Never,
}

/// No pattern predicate of the m68k tables is decided at selection time.
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

pub struct M68kHooks {
    cpu: Cpu,
    /// `OptLevel == CodeGenOptLevel::None`: nothing folds.
    opt_none: bool,
    /// The complex pattern of each number the table uses.
    complex: Vec<Option<Complex>>,
}

impl M68kHooks {
    pub fn new(
        cpu: Cpu,
        opt_none: bool,
    ) -> M68kHooks {
        let patterns = match cpu {
            Cpu::M68000 => m68k_m68000::COMPLEX_PATTERNS,
            Cpu::M68020 => m68k_m68020::COMPLEX_PATTERNS,
        };
        let mut complex = vec![None; patterns.iter().map(|&(n, ..)| usize::from(n) + 1).max().unwrap_or(0)];
        for &(number, function, _) in patterns {
            complex[usize::from(number)] = Some(match function {
                "SelectARI" => Complex::Ari,
                "SelectARID" => Complex::Arid,
                "SelectARII" => Complex::Arii,
                "SelectPCD" => Complex::Pcd,
                "SelectPCI" => Complex::Pci,
                "SelectPCIBD" => Complex::Pcibd,
                "SelectAL" => Complex::Al,
                "SelectARIPI" | "SelectARIPD" => Complex::Never,
                other => panic!("complex pattern `{other}` is not translated"),
            });
        }
        M68kHooks { cpu, opt_none, complex }
    }
}

impl<D: AddrDag> Hooks<D> for M68kHooks {
    fn pointer_type(&self) -> Vt {
        mvt::i32
    }

    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool {
        match self.cpu {
            Cpu::M68000 => m68k_m68000::pattern_predicate(&NoDynamicPredicates, predicate),
            Cpu::M68020 => m68k_m68020::pattern_predicate(&NoDynamicPredicates, predicate),
        }
    }

    fn check_node_predicate(
        &self,
        dag: &D,
        value: SDValue,
        predicate: u16,
    ) -> bool {
        match self.cpu {
            Cpu::M68000 => m68k_m68000::node_predicate(dag, self, value, predicate),
            Cpu::M68020 => m68k_m68020::node_predicate(dag, self, value, predicate),
        }
    }

    fn check_complex_pattern(
        &mut self,
        dag: &mut D,
        _root: u32,
        parent: Option<u32>,
        value: SDValue,
        pattern: u8,
        results: &mut Vec<(SDValue, Option<u32>)>,
    ) -> bool {
        let kind = self.complex[usize::from(pattern)].expect("a complex pattern the table does not name");
        let mut push = |values: &[SDValue]| results.extend(values.iter().map(|&v| (v, None)));
        match kind {
            Complex::Ari => addr::select_ari(dag, value).map(|v| push(&[v])).is_some(),
            Complex::Arid => addr::select_arid(dag, value).map(|v| push(&v)).is_some(),
            Complex::Arii => addr::select_arii(dag, parent, value).map(|v| push(&v)).is_some(),
            Complex::Pcd => addr::select_pcd(dag, value).map(|v| push(&[v])).is_some(),
            Complex::Pci => addr::select_pci(dag, value).map(|v| push(&v)).is_some(),
            Complex::Pcibd => addr::select_pcibd(dag, value).map(|v| push(&v)).is_some(),
            Complex::Al => addr::select_al(dag, value).map(|v| push(&[v])).is_some(),
            Complex::Never => false,
        }
    }

    fn run_xform(
        &mut self,
        dag: &mut D,
        value: SDValue,
        _xform: u8,
    ) -> SDValue {
        // The only transform: `getTargetConstant(N->getSExtValue(), SDLoc(N),
        // MVT::i16)`.
        let constant = dag.constant(value).expect("an xform of a constant");
        dag.target_constant(constant, mvt::i16)
    }

    /// `M68kDAGToDAGISel::IsProfitableToFold`.
    fn is_profitable_to_fold(
        &self,
        dag: &D,
        _value: SDValue,
        user: u32,
        root: u32,
    ) -> bool {
        if self.opt_none {
            return false;
        }
        if user == root {
            let opcode = dag.opcode(user);
            // Prefer NEG when zero subtracts a value.
            if (opcode == isd::SUB || opcode == m68k_isd::SUB) && dag.constant(dag.operand(user, 0)) == Some(0) {
                return false;
            }
        }
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
        if self.opt_none {
            return false;
        }
        let mut root = root;
        // A glued root: its user is the root of the matched code.
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

    fn has_sse_unaligned_mem(&self) -> bool {
        false
    }

    fn should_avoid_immediate_inst_forms_for_size(
        &self,
        _: &D,
        _: u32,
    ) -> bool {
        false
    }

    fn is_unneeded_shift_mask(
        &self,
        _: &D,
        _: u32,
        _: u32,
    ) -> bool {
        false
    }

    fn is_sext_absolute_symbol_ref(
        &self,
        _: &D,
        _: u32,
        _: u32,
    ) -> bool {
        false
    }

    fn has_no_carry_flag_uses(
        &self,
        _: &D,
        _: SDValue,
    ) -> bool {
        false
    }

    fn is_def32(
        &self,
        _: &D,
        _: u32,
    ) -> bool {
        false
    }

    fn check_tc_ret_enough_regs(
        &self,
        _: &D,
        _: u32,
    ) -> bool {
        true
    }

    fn may_load_store(
        &self,
        opcode: u16,
    ) -> (bool, bool) {
        instrinfo::memory_flags(opcode)
    }
}
