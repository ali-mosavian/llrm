//! The wasm32 matcher table (tblgen's `WebAssemblyGenDAGISel.inc`) against
//! hand-built DAGs.

use llrm_dagisel::dag::{Hooks, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::matcher::Vt;
use llrm_dagisel::testing::{OptSize, TestDag};
use llrm_wasm::dagisel::generated::wasm_wasm32 as table;

/// Hooks with no complex pattern: `SelectAddrOperands*` match nothing.
struct NoAddressModes;

impl Hooks<TestDag> for NoAddressModes {
    fn pointer_type(&self) -> Vt {
        mvt::i32
    }
    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool {
        table::pattern_predicate(&OptSize, predicate)
    }
    fn check_node_predicate(
        &self,
        dag: &TestDag,
        value: SDValue,
        predicate: u16,
    ) -> bool {
        table::node_predicate(dag, self, value, predicate)
    }
    fn check_complex_pattern(
        &mut self,
        _: &mut TestDag,
        _: u32,
        _: Option<u32>,
        _: SDValue,
        _: u8,
        _: &mut Vec<(SDValue, Option<u32>)>,
    ) -> bool {
        false
    }
    fn run_xform(
        &mut self,
        _: &mut TestDag,
        value: SDValue,
        _: u8,
    ) -> SDValue {
        value
    }
    fn is_profitable_to_fold(
        &self,
        _: &TestDag,
        _: SDValue,
        _: u32,
        _: u32,
    ) -> bool {
        true
    }
    fn is_legal_to_fold(
        &self,
        _: &TestDag,
        _: SDValue,
        _: u32,
        _: u32,
    ) -> bool {
        true
    }
    fn check_and_mask(
        &self,
        _: &TestDag,
        _: SDValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }
    fn check_or_mask(
        &self,
        _: &TestDag,
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
        _: &TestDag,
        _: u32,
    ) -> bool {
        false
    }
    fn is_unneeded_shift_mask(
        &self,
        _: &TestDag,
        _: u32,
        _: u32,
    ) -> bool {
        false
    }
    fn is_sext_absolute_symbol_ref(
        &self,
        _: &TestDag,
        _: u32,
        _: u32,
    ) -> bool {
        false
    }
    fn has_no_carry_flag_uses(
        &self,
        _: &TestDag,
        _: SDValue,
    ) -> bool {
        false
    }
    fn is_def32(
        &self,
        _: &TestDag,
        _: u32,
    ) -> bool {
        false
    }
    fn check_tc_ret_enough_regs(
        &self,
        _: &TestDag,
        _: u32,
    ) -> bool {
        true
    }
    fn may_load_store(
        &self,
        opcode: u16,
    ) -> (bool, bool) {
        llrm_wasm::dagisel::generated::instrinfo::memory_flags(opcode)
    }
}

fn machine_name(opcode: u16) -> &'static str {
    table::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
}

/// The pin's tblgen table, decoded: `i32 add` of two registers is `ADD_I32` and
/// `i64 add` is `ADD_I64` (the pin's llc: `-print-after=finalize-isel`). Before
/// the decoder took this table, wasm had none at all.
#[test]
fn add_of_two_registers_is_add_of_its_width() {
    for (vt, name) in [(mvt::i32, "ADD_I32"), (mvt::i64, "ADD_I64")] {
        let mut dag = TestDag::new();
        let a = dag.input(1, vt);
        let b = dag.input(2, vt);
        let add = dag.node(isd::ADD, &[vt], &[a, b]);
        let selector = Selector::new(table::TABLE);

        assert_eq!(selector.select(&mut dag, &mut NoAddressModes, add), Outcome::Selected);

        assert_eq!(machine_name(dag.nodes[add as usize].machine.expect("not selected")), name);
    }
}
