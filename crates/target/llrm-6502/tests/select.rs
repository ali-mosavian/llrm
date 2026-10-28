//! The 6502 matcher tables (tblgen's `GenDAGISel.inc` of `mos6502.td`) against
//! hand-built DAGs, once per CPU.

use llrm_dagisel::dag::{Hooks, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::matcher::Vt;
use llrm_dagisel::testing::{OptSize, TestDag};

macro_rules! cpu_tests {
    ($cpu:ident, $table:ident) => {
        mod $cpu {
            use super::*;
            use llrm_6502::dagisel::generated::$table as table;

            /// `SelectIndirect` takes the value as it is: a pointer already in an `Imag16` pair.
            struct Indirect;

            impl Hooks<TestDag> for Indirect {
                fn pointer_type(&self) -> Vt {
                    mvt::i16
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
                    value: SDValue,
                    _: u8,
                    results: &mut Vec<(SDValue, Option<u32>)>,
                ) -> bool {
                    results.push((value, None));
                    true
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
                    llrm_6502::dagisel::generated::instrinfo::memory_flags(opcode)
                }
            }


            fn machine_name(opcode: u16) -> &'static str {
                table::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
            }

            fn select(dag: &mut TestDag, root: u32) -> Outcome {
                Selector::new(table::TABLE).select(dag, &mut Indirect, root)
            }

            /// Before the 6502 had a table, an i8 DAG had nothing to select it: `add` of two
            /// imaginary registers is `ADD8`.
            #[test]
            fn add_of_two_registers_is_add8() {
                let mut dag = TestDag::new();
                let a = dag.input(1, mvt::i8);
                let b = dag.input(2, mvt::i8);
                let add = dag.node(isd::ADD, &[mvt::i8], &[a, b]);

                assert_eq!(select(&mut dag, add), Outcome::Selected);

                assert_eq!(machine_name(dag.nodes[add as usize].machine.expect("not selected")), "ADD8");
            }

            /// A node id no LLVM header defines (`MOSISD::RET_GLUE`, from `own_ids.txt`) reaches
            /// the table: it selects `RET`.
            #[test]
            fn ret_glue_is_ret() {
                let mut dag = TestDag::new();
                let ret = dag.node(llrm_6502::dagisel::generated::ids::isd::RET_GLUE, &[mvt::Other], &[SDValue::new(0, 0)]);

                assert_eq!(select(&mut dag, ret), Outcome::Selected);

                assert_eq!(machine_name(dag.nodes[ret as usize].machine.expect("not selected")), "RET");
            }
        }
    };
}

cpu_tests!(mos6502, mos_mos6502);
cpu_tests!(w65c02, mos_w65c02);

/// STZ exists on the 65C02 only: the NMOS table must not name it, or it would emit an illegal opcode.
#[test]
fn stz_is_in_the_65c02_table_only() {
    use llrm_6502::dagisel::generated::{mos_mos6502 as nmos, mos_w65c02 as cmos};
    let has = |ops: &[(u16, &str)]| ops.iter().any(|(_, n)| *n == "STZP");
    assert!(has(cmos::MACHINE_OPCODES));
    assert!(!has(nmos::MACHINE_OPCODES));
}
