//! The 6502 matcher tables (tblgen's `GenDAGISel.inc` of `mos6502.td`) against
//! hand-built DAGs, once per CPU.

use llrm_6502::dagisel::hooks::{Cpu, MosHooks};
use llrm_dagisel::dag::SDValue;
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::testing::TestDag;

macro_rules! cpu_tests {
    ($cpu:ident, $table:ident, $kind:expr) => {
        mod $cpu {
            use super::*;
            use llrm_6502::dagisel::generated::$table as table;

            fn machine_name(opcode: u16) -> &'static str {
                table::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
            }

            fn select(dag: &mut TestDag, root: u32) -> Outcome {
                Selector::new(table::TABLE).select(dag, &mut MosHooks::new($kind, false), root)
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

            /// The type legalizer splits an `i32` add into `UADDO` on the low words and `UADDO_CARRY`
            /// on the high ones; the carry is an `i1` in `C`. They select `ADDO16` and `ADDOC16`.
            #[test]
            fn word_add_with_carry_out_and_in_select_addo16_and_addoc16() {
                let mut dag = TestDag::new();
                let a = dag.input(1, mvt::i16);
                let b = dag.input(2, mvt::i16);
                let carry = dag.input(3, mvt::i1);
                let lo = dag.node(isd::UADDO, &[mvt::i16, mvt::i1], &[a, b]);
                let hi = dag.node(isd::UADDO_CARRY, &[mvt::i16, mvt::i1], &[a, b, carry]);

                assert_eq!(select(&mut dag, lo), Outcome::Selected);
                assert_eq!(select(&mut dag, hi), Outcome::Selected);

                let name = |node: u32| machine_name(dag.nodes[node as usize].machine.expect("not selected"));
                assert_eq!((name(lo), name(hi)), ("ADDO16", "ADDOC16"));
            }

            /// A load through `ptr + 3` is one `(zp),Y` load: the pair and the offset 3, not an add.
            #[test]
            fn a_byte_load_through_pointer_plus_3_is_ldp8_with_offset_3() {
                let mut dag = TestDag::new();
                let ptr = dag.input(1, mvt::i16);
                let three = dag.make_constant(3, mvt::i16);
                let address = SDValue::new(dag.node(isd::ADD, &[mvt::i16], &[ptr, three]), 0);
                let undef = SDValue::new(dag.node(isd::UNDEF, &[mvt::i16], &[]), 0);
                let load = dag.add(llrm_dagisel::testing::Node {
                    opcode: isd::LOAD,
                    vts: vec![mvt::i8, mvt::Other],
                    ops: vec![SDValue::new(0, 0), address, undef],
                    load: Some(llrm_dagisel::dag::LoadExt::NonExt),
                    memory_vt: Some(mvt::i8),
                    ..Default::default()
                });

                assert_eq!(select(&mut dag, load), Outcome::Selected);

                let node = &dag.nodes[load as usize];
                assert_eq!(machine_name(node.machine.expect("not selected")), "LDP8");
                assert_eq!(node.ops[0], ptr);
                assert_eq!(dag.nodes[node.ops[1].node as usize].constant, Some(3));
            }

            /// `x << 3` on a byte is one shift instruction with the count 3.
            #[test]
            fn a_byte_shift_by_3_is_shl8() {
                let mut dag = TestDag::new();
                let x = dag.input(1, mvt::i8);
                let three = dag.make_constant(3, mvt::i8);
                let shl = dag.node(isd::SHL, &[mvt::i8], &[x, three]);

                assert_eq!(select(&mut dag, shl), Outcome::Selected);

                assert_eq!(machine_name(dag.nodes[shl as usize].machine.expect("not selected")), "SHL8");
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

cpu_tests!(mos6502, mos_mos6502, Cpu::Mos6502);
cpu_tests!(w65c02, mos_w65c02, Cpu::W65c02);

/// STZ and BRA exist on the 65C02 only: the NMOS table must not name them, or it would emit an illegal opcode.
#[test]
fn cmos_instructions_are_in_the_65c02_table_only() {
    use llrm_6502::dagisel::generated::{mos_mos6502 as nmos, mos_w65c02 as cmos};
    let has = |ops: &[(u16, &str)], name: &str| ops.iter().any(|(_, n)| *n == name);
    for name in ["STZ8", "STZP", "BRA"] {
        assert!(has(cmos::MACHINE_OPCODES, name), "{name} missing from the 65C02 table");
        assert!(!has(nmos::MACHINE_OPCODES, name), "{name} leaked into the NMOS table");
    }
}
