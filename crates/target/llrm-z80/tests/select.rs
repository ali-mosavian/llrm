//! The Z80 matcher tables (tblgen's `GenDAGISel.inc` of `z80.td`) against
//! hand-built DAGs, once per CPU.

use llrm_dagisel::dag::{LoadExt, SDValue};
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::testing::{Node, TestDag};
use llrm_z80::dagisel::hooks::{Cpu, Z80Hooks};

macro_rules! cpu_tests {
    ($cpu:ident, $table:ident, $kind:expr) => {
        mod $cpu {
            use llrm_z80::dagisel::generated::$table as table;

            use super::*;

            fn machine_name(opcode: u16) -> &'static str {
                table::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
            }

            fn select(
                dag: &mut TestDag,
                root: u32,
            ) -> Outcome {
                Selector::new(table::TABLE).select(dag, &mut Z80Hooks::new($kind, false), root)
            }

            fn name(
                dag: &TestDag,
                node: u32,
            ) -> &'static str {
                machine_name(dag.nodes[node as usize].machine.expect("not selected"))
            }

            #[test]
            fn add_of_two_bytes_is_add8r() {
                let mut dag = TestDag::new();
                let a = dag.input(1, mvt::i8);
                let b = dag.input(2, mvt::i8);
                let add = dag.node(isd::ADD, &[mvt::i8], &[a, b]);

                assert_eq!(select(&mut dag, add), Outcome::Selected);

                assert_eq!(name(&dag, add), "ADD8r");
            }

            #[test]
            fn add_of_a_byte_and_a_constant_is_add8i() {
                let mut dag = TestDag::new();
                let a = dag.input(1, mvt::i8);
                let k = dag.make_constant(7, mvt::i8);
                let add = dag.node(isd::ADD, &[mvt::i8], &[a, k]);

                assert_eq!(select(&mut dag, add), Outcome::Selected);

                assert_eq!(name(&dag, add), "ADD8i");
            }

            /// `x + 1` is `inc r`, not `add a, 1`: the constant 1 is the more
            /// specific pattern.
            #[test]
            fn a_byte_plus_one_is_inc8() {
                let mut dag = TestDag::new();
                let a = dag.input(1, mvt::i8);
                let one = dag.make_constant(1, mvt::i8);
                let add = dag.node(isd::ADD, &[mvt::i8], &[a, one]);

                assert_eq!(select(&mut dag, add), Outcome::Selected);

                assert_eq!(name(&dag, add), "INC8");
            }

            #[test]
            fn a_word_add_is_add16_on_hl() {
                let mut dag = TestDag::new();
                let a = dag.input(1, mvt::i16);
                let b = dag.input(2, mvt::i16);
                let add = dag.node(isd::ADD, &[mvt::i16], &[a, b]);

                assert_eq!(select(&mut dag, add), Outcome::Selected);

                assert_eq!(name(&dag, add), "ADD16");
            }

            #[test]
            fn a_byte_load_through_a_pointer_is_ld8rm() {
                let mut dag = TestDag::new();
                let ptr = dag.input(1, mvt::i16);
                let undef = SDValue::new(dag.node(isd::UNDEF, &[mvt::i16], &[]), 0);
                let load = dag.add(Node {
                    opcode: isd::LOAD,
                    vts: vec![mvt::i8, mvt::Other],
                    ops: vec![SDValue::new(0, 0), ptr, undef],
                    load: Some(LoadExt::NonExt),
                    memory_vt: Some(mvt::i8),
                    ..Default::default()
                });

                assert_eq!(select(&mut dag, load), Outcome::Selected);

                assert_eq!(name(&dag, load), "LD8rm");
            }

            /// A node id no LLVM header defines (`Z80ISD::RET_GLUE`, from
            /// `own_ids.txt`) reaches the table: it selects `RET`.
            #[test]
            fn ret_glue_is_ret() {
                let mut dag = TestDag::new();
                let ret =
                    dag.node(llrm_z80::dagisel::generated::ids::isd::RET_GLUE, &[mvt::Other], &[SDValue::new(0, 0)]);

                assert_eq!(select(&mut dag, ret), Outcome::Selected);

                assert_eq!(name(&dag, ret), "RET");
            }
        }
    };
}

cpu_tests!(z80, z80_z80, Cpu::Z80);
cpu_tests!(z180, z80_z180, Cpu::Z180);

/// `mlt` exists on the Z180 and later: the Z80 table must not name it, or it
/// would emit an illegal opcode.
#[test]
fn the_multiply_is_in_the_z180_table_only() {
    use llrm_z80::dagisel::generated::{z80_z80 as z80, z80_z180 as z180};
    let has = |ops: &[(u16, &str)]| ops.iter().any(|(_, n)| *n == "UMUL8X8");
    assert!(has(z180::MACHINE_OPCODES));
    assert!(!has(z80::MACHINE_OPCODES));
}
