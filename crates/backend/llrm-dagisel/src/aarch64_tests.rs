//! The interpreter against the generated AArch64 table, on hand-built DAGs, with hooks that name no x86
//! concept: the table and `Hooks` are the target's, `X86Predicates` is not asked for.

use crate::dag::{Dag, Hooks, SDValue};
use crate::generated::aarch64::apple_m1 as table;
use crate::generated::ids::{isd, mvt};
use crate::interp::{Outcome, Selector};
use crate::matcher::Vt;
use crate::testing::TestDag;

/// Every hook says no; a pattern that needs one is not selected.
struct Bare;

impl Hooks<TestDag> for Bare {
    fn pointer_type(&self) -> Vt {
        mvt::i64
    }
    fn check_pattern_predicate(
        &self,
        _: u16,
    ) -> bool {
        false
    }
    fn check_node_predicate(
        &self,
        _: &TestDag,
        _: SDValue,
        _: u16,
    ) -> bool {
        false
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
    fn may_load_store(
        &self,
        _: u16,
    ) -> (bool, bool) {
        (false, false)
    }
}

fn name(opcode: u16) -> &'static str {
    table::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
}

/// `(add i32, i32)` is `ADDWrr`, and an `i64` pair `ADDXrr`: the table is the pin's AArch64 one, selected by
/// the generic interpreter.
#[test]
fn an_add_of_two_registers_is_addwrr_and_addxrr() {
    for (vt, expected) in [(mvt::i32, "ADDWrr"), (mvt::i64, "ADDXrr")] {
        let mut dag = TestDag::new();
        let a = dag.input(1, vt);
        let b = dag.input(2, vt);
        let add = dag.node(isd::ADD, &[vt], &[a, b]);
        let selector = Selector::new(table::TABLE);
        assert_eq!(selector.select(&mut dag, &mut Bare, add), Outcome::Selected);
        assert_eq!(name(dag.nodes[add as usize].machine.expect("selected")), expected);
    }
}

/// The scalar table keeps no SVE instruction: `features_aarch64.toml` turns SVE, SME and NEON off. NEON
/// forms of scalar operations (`FCVTZSv1i64`) stay.
#[test]
fn the_scalar_table_emits_no_sve_instruction() {
    let sve: Vec<_> = table::MACHINE_OPCODES
        .iter()
        .filter(|(_, n)| n.contains("_ZZZ") || n.contains("_ZPmZ") || n.contains("ZPZ") || n.starts_with("SVE"))
        .collect();
    assert!(sve.is_empty(), "{sve:?}");
    assert!(table::MACHINE_OPCODES.iter().any(|(_, n)| *n == "MADDWrrr"));
}
