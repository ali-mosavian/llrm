//! The interpreter against the generated i486 table, on hand-built DAGs.

use crate::dag::{Dag, SDValue};
use crate::generated::ids::{isd, mvt};
use crate::generated::x86_i486 as table;
use crate::interp::{Outcome, Selector};
use crate::testing::{Node, TestDag, TestHooks};

fn machine_name(opcode: u16) -> &'static str {
    table::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
}

/// The machine opcode of the node `value` now is.
fn selected(
    dag: &TestDag,
    node: u32,
) -> &'static str {
    machine_name(dag.nodes[node as usize].machine.expect("not selected"))
}

#[test]
fn add_of_two_registers_is_add32rr() {
    let mut dag = TestDag::new();
    let a = dag.input(1, mvt::i32);
    let b = dag.input(2, mvt::i32);
    let add = dag.node(isd::ADD, &[mvt::i32], &[a, b]);
    let selector = Selector::new(table::TABLE);
    let mut hooks = TestHooks { address: None };

    assert_eq!(selector.select(&mut dag, &mut hooks, add), Outcome::Selected);

    assert_eq!(selected(&dag, add), "ADD32rr");
    assert_eq!(dag.nodes[add as usize].ops, [a, b]);
}

/// Each selection built its own node stack and match scopes (five allocations
/// and their growth a node, 5,336 nodes in d_faces -O2); the buffers are kept
/// for the next selection now.
#[test]
fn the_buffers_of_a_selection_are_kept_for_the_next() {
    let mut dag = TestDag::new();
    let a = dag.input(1, mvt::i32);
    let b = dag.input(2, mvt::i32);
    let add = dag.node(isd::ADD, &[mvt::i32], &[a, b]);
    let selector = Selector::new(table::TABLE);
    let mut hooks = TestHooks { address: None };
    assert_eq!(selector.select(&mut dag, &mut hooks, add), Outcome::Selected);
    assert!(crate::interp::kept_capacity() > 0, "nothing was kept");
}

#[test]
fn a_load_through_an_address_pattern_is_mov32rm_with_five_address_operands() {
    let mut dag = TestDag::new();
    let base = dag.input(1, mvt::i32);
    let index = dag.input(2, mvt::i32);
    let address = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[base, index]), 0);
    let load = dag.add(Node {
        opcode: isd::LOAD,
        vts: vec![mvt::i32, mvt::Other],
        ops: vec![SDValue::new(0, 0), address, SDValue::new(0, 0)],
        load: Some(crate::dag::LoadExt::NonExt),
        memory_vt: Some(mvt::i32),
        ..Node::default()
    });
    let selector = Selector::new(table::TABLE);
    let mut hooks = TestHooks { address: Some((base, index, 4, 8)) };

    assert_eq!(selector.select(&mut dag, &mut hooks, load), Outcome::Selected);

    assert_eq!(selected(&dag, load), "MOV32rm");
    let ops = &dag.nodes[load as usize].ops;
    assert_eq!(ops.len(), 6, "base, scale, index, disp, segment, chain");
    assert_eq!(ops[0], base);
    assert_eq!(ops[2], index);
    assert_eq!(dag.nodes[ops[1].node as usize].constant, Some(4));
    assert_eq!(dag.nodes[ops[3].node as usize].constant, Some(8));
}

#[test]
fn a_node_no_pattern_covers_cannot_be_selected() {
    let mut dag = TestDag::new();
    let a = dag.input(1, mvt::i32);
    let odd = dag.node(9999, &[mvt::i32], &[a]);
    let selector = Selector::new(table::TABLE);
    let mut hooks = TestHooks { address: None };

    assert_eq!(selector.select(&mut dag, &mut hooks, odd), Outcome::CannotSelect);
}

fn emits(
    opcodes: &[(u16, &str)],
    prefix: &str,
) -> bool {
    opcodes.iter().any(|(_, name)| name.split('|').any(|n| n.starts_with(prefix)))
}

#[test]
fn neither_cpu_emits_cmov_and_only_p5_emits_cmpxchg8b() {
    use crate::generated::x86_p5;
    // `CMOV_GR32` and kin are the pseudos that become a branch; `CMOV32rr` is
    // the instruction.
    for (cpu, opcodes) in [("i486", table::MACHINE_OPCODES), ("P5", x86_p5::MACHINE_OPCODES)] {
        assert!(emits(opcodes, "CMOV_GR32"), "{cpu}: select is the branch pseudo");
        assert!(!emits(opcodes, "CMOV32") && !emits(opcodes, "CMOV16"), "{cpu} has no CMOV instruction");
    }
    assert!(!emits(table::MACHINE_OPCODES, "LCMPXCHG8B"), "i486 has no cx8");
    assert!(emits(x86_p5::MACHINE_OPCODES, "LCMPXCHG8B"), "P5 has cx8");
    assert!(emits(table::MACHINE_OPCODES, "ADD32rr"), "the table must not be empty of the basics");
}

mod x86_addr {
    use super::*;
    use crate::dag::LoadExt;
    use crate::testing::{Node, TestDag};
    use crate::x86_addr::{AddressMode, AddressOperands, BaseType, Matcher, SymbolKind};
    use crate::x86_hooks::{Cpu, Options, X86Hooks};

    fn hooks() -> X86Hooks {
        X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        })
    }

    fn constant_of(
        dag: &TestDag,
        v: SDValue,
    ) -> Option<i64> {
        dag.nodes[v.node as usize].constant
    }

    /// `a[i + 2]` for `int a[]`: the address is `a + ((i + 2) << 2)`.
    fn indexed_address(dag: &mut TestDag) -> (SDValue, SDValue, SDValue) {
        let base = dag.input(1, mvt::i32);
        let i = dag.input(2, mvt::i32);
        let two = dag.make_constant(2, mvt::i32);
        let shift = dag.make_constant(2, mvt::i8);
        let i_plus_2 = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[i, two]), 0);
        let scaled = SDValue::new(dag.node(isd::SHL, &[mvt::i32], &[i_plus_2, shift]), 0);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[base, scaled]), 0);
        (address, base, i)
    }

    fn matcher(dag: &mut TestDag) -> Matcher<'_, TestDag> {
        Matcher { dag, pointer_type: mvt::i32, mode: crate::x86_addr::AddrMode::Flat32, size: false }
    }

    #[test]
    fn a_index_plus_two_scaled_by_four_is_one_base_index_disp_operand() {
        let mut dag = TestDag::new();
        let (address, base, i) = indexed_address(&mut dag);

        let AddressOperands { base: b, scale, index, disp, .. } = matcher(&mut dag).select_addr(address).unwrap();

        assert_eq!((b, index), (base, i));
        assert_eq!(constant_of(&dag, scale), Some(4));
        assert_eq!(constant_of(&dag, disp), Some(8), "the +2 folds into the displacement, scaled");
    }

    #[test]
    fn a_load_of_that_address_is_mov32rm_with_the_folded_operands() {
        let mut dag = TestDag::new();
        let (address, base, i) = indexed_address(&mut dag);
        let load = dag.add(Node {
            opcode: isd::LOAD,
            vts: vec![mvt::i32, mvt::Other],
            ops: vec![SDValue::new(0, 0), address, SDValue::new(0, 0)],
            load: Some(LoadExt::NonExt),
            memory_vt: Some(mvt::i32),
            ..Node::default()
        });
        let mut hooks = hooks();

        assert_eq!(Selector::new(table::TABLE).select(&mut dag, &mut hooks, load), Outcome::Selected);

        assert_eq!(selected(&dag, load), "MOV32rm");
        let ops = &dag.nodes[load as usize].ops;
        assert_eq!((ops[0], ops[2]), (base, i));
        assert_eq!(constant_of(&dag, ops[1]), Some(4));
        assert_eq!(constant_of(&dag, ops[3]), Some(8));
    }

    #[test]
    fn a_frame_slot_plus_a_constant_is_a_frame_base_with_a_displacement() {
        let mut dag = TestDag::new();
        let slot = dag.frame_index(3, mvt::i32);
        let four = dag.make_constant(4, mvt::i32);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[slot, four]), 0);

        let a = matcher(&mut dag).select_addr(address).unwrap();

        assert_eq!(dag.nodes[a.base.node as usize].opcode, isd::TargetFrameIndex);
        assert_eq!(constant_of(&dag, a.disp), Some(4));
    }

    #[test]
    fn a_wrapped_global_plus_a_constant_keeps_the_symbol_and_folds_the_offset() {
        let mut dag = TestDag::new();
        let global = dag.wrapped_global(7, 0, mvt::i32);
        let twelve = dag.make_constant(12, mvt::i32);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[global, twelve]), 0);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag).match_address(address, &mut am));

        let symbol = am.symbol.unwrap();
        assert_eq!((symbol.kind, symbol.id, am.disp), (SymbolKind::Global, 7, 12));
        assert!(am.base_reg.is_none() && am.index_reg.is_none() && am.base_type == BaseType::Reg);
    }

    #[test]
    fn lea_wants_more_than_a_plain_add() {
        let mut dag = TestDag::new();
        let (address, ..) = indexed_address(&mut dag);
        let a = dag.input(3, mvt::i32);
        let b = dag.input(4, mvt::i32);
        let plain = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[a, b]), 0);

        assert!(matcher(&mut dag).select_lea_addr(address).is_some(), "base + index*4 + 8 is worth an lea");
        assert!(matcher(&mut dag).select_lea_addr(plain).is_none(), "a + b is an add");
    }

    #[test]
    fn two_times_an_index_becomes_base_plus_index() {
        let mut dag = TestDag::new();
        let x = dag.input(1, mvt::i32);
        let shift = dag.make_constant(1, mvt::i8);
        let address = SDValue::new(dag.node(isd::SHL, &[mvt::i32], &[x, shift]), 0);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag).match_address(address, &mut am));

        assert_eq!((am.base_reg, am.index_reg, am.scale), (Some(x), Some(x), 1), "lea (x,x), not (,x,2)");
    }

    #[test]
    fn a_displacement_near_the_limit_is_not_folded_next_to_a_register() {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i32);
        let huge = dag.make_constant(0x4000_0000, mvt::i32);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[p, huge]), 0);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag).match_address(address, &mut am));

        assert_eq!(am.disp, 0, "isInt<31> fails: the constant stays a register operand");
        assert!(am.base_reg.is_some() && am.index_reg.is_some());
    }
}

mod rmw {
    use super::*;
    use crate::dag::LoadExt;
    use crate::generated::ids::x86isd;
    use crate::testing::{Node, TestDag};
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_rmw::fold_load_store_into_mem_operand;

    fn hooks() -> X86Hooks {
        X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        })
    }

    /// `*p += rhs` as the DAG makes it: load, X86ISD::ADD, store. Returns the
    /// store, the load and the arithmetic node.
    fn add_to_memory(
        dag: &mut TestDag,
        rhs: SDValue,
        p: SDValue,
    ) -> (u32, u32, u32) {
        let off = SDValue::new(dag.node(isd::UNDEF, &[mvt::i32], &[]), 0);
        let entry = SDValue::new(0, 0);
        let load = dag.add(Node {
            opcode: isd::LOAD,
            vts: vec![mvt::i32, mvt::Other],
            ops: vec![entry, p, off],
            load: Some(LoadExt::NonExt),
            memory_vt: Some(mvt::i32),
            ..Node::default()
        });
        let add = dag.node(x86isd::ADD, &[mvt::i32, mvt::i32], &[SDValue::new(load, 0), rhs]);
        let store = dag.add(Node {
            opcode: isd::STORE,
            vts: vec![mvt::Other],
            ops: vec![SDValue::new(load, 1), SDValue::new(add, 0), p, off],
            memory_vt: Some(mvt::i32),
            ..Node::default()
        });
        (store, load, add)
    }

    fn machine_nodes(dag: &TestDag) -> Vec<&'static str> {
        dag.live().filter_map(|(_, n)| n.machine.map(machine_name)).collect()
    }

    #[test]
    fn add_a_register_to_memory_is_one_add32mr() {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i32);
        let r = dag.input(2, mvt::i32);
        let (store, load, _) = add_to_memory(&mut dag, r, p);

        assert!(fold_load_store_into_mem_operand(&mut dag, &hooks(), store));

        assert_eq!(machine_nodes(&dag), ["ADD32mr"]);
        let (_, node) = dag.live().find(|(_, n)| n.machine.is_some()).unwrap();
        assert_eq!(node.ops[0], p, "base");
        assert_eq!(node.ops[5], r, "the register operand");
        assert_eq!(node.ops[6], SDValue::new(0, 0), "the chain the load had");
        assert!(dag.nodes[store as usize].dead, "the store is gone");
        let _ = load;
    }

    #[test]
    fn adding_one_to_memory_is_inc32m() {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i32);
        let one = dag.make_constant(1, mvt::i32);
        let (store, ..) = add_to_memory(&mut dag, one, p);

        assert!(fold_load_store_into_mem_operand(&mut dag, &hooks(), store));

        assert_eq!(machine_nodes(&dag), ["INC32m"]);
    }

    #[test]
    fn a_load_with_another_user_is_not_fused() {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i32);
        let r = dag.input(2, mvt::i32);
        let (store, load, _) = add_to_memory(&mut dag, r, p);
        // The old value is also used elsewhere: it must stay a load.
        dag.node(isd::ADD, &[mvt::i32], &[SDValue::new(load, 0), r]);

        assert!(!fold_load_store_into_mem_operand(&mut dag, &hooks(), store));
        assert!(machine_nodes(&dag).is_empty());
    }

    #[test]
    fn a_small_immediate_adds_directly() {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i32);
        let imm = dag.make_constant(300, mvt::i32);
        let (store, ..) = add_to_memory(&mut dag, imm, p);

        assert!(fold_load_store_into_mem_operand(&mut dag, &hooks(), store));

        assert_eq!(machine_nodes(&dag), ["ADD32mi"]);
    }
}

mod fold_load {
    use super::*;
    use crate::dag::LoadExt;
    use crate::testing::{Node, TestDag};
    use crate::x86_hooks::{Cpu, Options, X86Hooks};

    fn hooks(opt_none: bool) -> X86Hooks {
        X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        })
    }

    /// `add (load p), r` where the load has no other user.
    fn load_add(dag: &mut TestDag) -> (u32, SDValue) {
        let p = dag.input(1, mvt::i32);
        let r = dag.input(2, mvt::i32);
        let off = SDValue::new(dag.node(isd::UNDEF, &[mvt::i32], &[]), 0);
        let load = dag.add(Node {
            opcode: isd::LOAD,
            vts: vec![mvt::i32, mvt::Other],
            ops: vec![SDValue::new(0, 0), p, off],
            load: Some(LoadExt::NonExt),
            memory_vt: Some(mvt::i32),
            ..Node::default()
        });
        let add = dag.node(isd::ADD, &[mvt::i32], &[r, SDValue::new(load, 0)]);
        (add, SDValue::new(load, 0))
    }

    #[test]
    fn a_single_use_load_folds_into_its_add() {
        let mut dag = TestDag::new();
        let (add, load) = load_add(&mut dag);

        assert!(hooks(false).try_fold_load(&mut dag, add, add, load).is_some());
    }

    #[test]
    fn nothing_folds_when_optimization_is_off() {
        let mut dag = TestDag::new();
        let (add, load) = load_add(&mut dag);

        assert!(hooks(true).try_fold_load(&mut dag, add, add, load).is_none());
    }
}

mod address_folds {
    use super::*;
    use crate::testing::TestDag;
    use crate::x86_addr::{AddressMode, Matcher};

    fn matcher(dag: &mut TestDag) -> Matcher<'_, TestDag> {
        Matcher { dag, pointer_type: mvt::i32, mode: crate::x86_addr::AddrMode::Flat32, size: false }
    }

    fn masked_shift(
        dag: &mut TestDag,
        amount: i64,
        mask: i64,
    ) -> (SDValue, SDValue) {
        let x = dag.input(1, mvt::i32);
        let count = dag.make_constant(amount, mvt::i8);
        let shifted = SDValue::new(dag.node(isd::SRL, &[mvt::i32], &[x, count]), 0);
        let mask = dag.make_constant(mask, mvt::i32);
        (SDValue::new(dag.node(isd::AND, &[mvt::i32], &[shifted, mask]), 0), x)
    }

    #[test]
    fn a_byte_extracted_by_shift_and_mask_becomes_a_scaled_index() {
        // `(x >> 6) & 0x3fc` is `((x >> 8) & 0xff) << 2`.
        let mut dag = TestDag::new();
        let (and, x) = masked_shift(&mut dag, 6, 0x3fc);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag).match_address(and, &mut am));

        assert_eq!(am.scale, 4);
        let index = am.index_reg.unwrap();
        assert_eq!(dag.nodes[index.node as usize].opcode, isd::AND, "(x >> 8) & 0xff");
        let srl = dag.nodes[index.node as usize].ops[0];
        assert_eq!(dag.nodes[srl.node as usize].ops[0], x);
        assert_eq!(constant_of_node(&dag, dag.nodes[srl.node as usize].ops[1]), Some(8));
    }

    fn constant_of_node(
        dag: &TestDag,
        v: SDValue,
    ) -> Option<i64> {
        dag.nodes[v.node as usize].constant
    }

    #[test]
    fn a_masked_shift_left_becomes_a_masked_value_scaled() {
        // `(x << 2) & 0xfc` is `(x & 0x3f) << 2`.
        let mut dag = TestDag::new();
        let x = dag.input(1, mvt::i32);
        let count = dag.make_constant(2, mvt::i8);
        let shl = SDValue::new(dag.node(isd::SHL, &[mvt::i32], &[x, count]), 0);
        let mask = dag.make_constant(0xfc, mvt::i32);
        let and = SDValue::new(dag.node(isd::AND, &[mvt::i32], &[shl, mask]), 0);
        // The fold wants the AND used once, by the address it is part of.
        let base = dag.input(2, mvt::i32);
        dag.node(isd::ADD, &[mvt::i32], &[base, and]);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag).match_address(and, &mut am));

        assert_eq!(am.scale, 4);
        let index = am.index_reg.unwrap();
        assert_eq!(dag.nodes[index.node as usize].ops[0], x);
        assert_eq!(constant_of_node(&dag, dag.nodes[index.node as usize].ops[1]), Some(0x3f));
    }

    #[test]
    fn a_shift_and_mask_that_drops_unknown_bits_is_left_alone() {
        // `(x >> 1) & 0x3fc` with nothing known about x: the high bits of x may
        // be set.
        let mut dag = TestDag::new();
        let (and, _) = masked_shift(&mut dag, 1, 0x3fc);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag).match_address(and, &mut am));

        assert_eq!((am.scale, am.base_reg), (1, Some(and)), "the whole value stays a register");
    }

    #[test]
    fn the_same_shift_and_mask_scales_when_the_high_bits_are_known_zero() {
        // x = y & 0xff: `(x >> 1) & 0x3fc` is `(x >> 3) << 2`.
        let mut dag = TestDag::new();
        let y = dag.input(1, mvt::i32);
        let byte = dag.make_constant(0xff, mvt::i32);
        let x = SDValue::new(dag.node(isd::AND, &[mvt::i32], &[y, byte]), 0);
        let count = dag.make_constant(1, mvt::i8);
        let shifted = SDValue::new(dag.node(isd::SRL, &[mvt::i32], &[x, count]), 0);
        let mask = dag.make_constant(0x3fc, mvt::i32);
        let and = SDValue::new(dag.node(isd::AND, &[mvt::i32], &[shifted, mask]), 0);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag).match_address(and, &mut am));

        assert_eq!(am.scale, 4);
        let index = am.index_reg.unwrap();
        assert_eq!(dag.nodes[index.node as usize].opcode, isd::SRL);
        assert_eq!(constant_of_node(&dag, dag.nodes[index.node as usize].ops[1]), Some(3));
    }
}

mod select_without_cmov {
    use super::*;
    use crate::dag::Dag;
    use crate::generated::ids::x86isd;
    use crate::testing::TestDag;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};

    /// `X86ISD::CMOV f, t, cc, (CMP a, b)` for a cpu without the instruction.
    fn select(
        cpu: Cpu,
        vt: crate::matcher::Vt,
    ) -> (TestDag, u32) {
        let mut dag = TestDag::new();
        let a = dag.input(1, vt);
        let b = dag.input(2, vt);
        let (t, f) = (dag.input(3, vt), dag.input(4, vt));
        let flags = SDValue::new(dag.node(x86isd::CMP, &[mvt::i32], &[a, b]), 0);
        let cc = dag.target_constant(4, mvt::i8);
        let cmov = dag.node(x86isd::CMOV, &[vt, mvt::i32], &[f, t, cc, flags]);
        let mut hooks = X86Hooks::new(Options {
            cpu,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        });
        assert_eq!(Selector::new(table::TABLE).select(&mut dag, &mut hooks, cmov), Outcome::Selected);
        (dag, cmov)
    }

    #[test]
    fn a_select_on_flags_is_the_branch_pseudo_not_the_instruction() {
        for (vt, pseudo) in [(mvt::i32, "CMOV_GR32"), (mvt::i16, "CMOV_GR16"), (mvt::i8, "CMOV_GR8")] {
            let (dag, cmov) = select(Cpu::I486, vt);
            assert_eq!(selected(&dag, cmov), pseudo);
        }
    }
}

mod real_mode_addresses {
    use super::*;
    use crate::testing::TestDag;
    use crate::x86_addr::{AddrMode, AddressMode, Matcher};

    fn matcher(
        dag: &mut TestDag,
        mode: AddrMode,
    ) -> Matcher<'_, TestDag> {
        Matcher { dag, pointer_type: mvt::i16, mode, size: false }
    }

    #[test]
    fn a_field_of_a_struct_through_a_pointer_is_base_plus_displacement() {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i16);
        let four = dag.make_constant(4, mvt::i16);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i16], &[p, four]), 0);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag, AddrMode::Real16).match_address(address, &mut am));

        assert_eq!((am.base_reg, am.index_reg, am.scale, am.disp), (Some(p), None, 1, 4));
    }

    fn word_element(dag: &mut TestDag) -> (SDValue, SDValue, SDValue, SDValue) {
        let a = dag.input(1, mvt::i16);
        let i = dag.input(2, mvt::i16);
        let one = dag.make_constant(1, mvt::i8);
        let scaled = SDValue::new(dag.node(isd::SHL, &[mvt::i16], &[i, one]), 0);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i16], &[a, scaled]), 0);
        (address, a, i, scaled)
    }

    /// `frame - (i << 2) + 24` took the shift as a negated index with scale 4
    /// at real mode, an address nothing encodes: `qb/bounds.bas` stopped in
    /// `LOCALS` with "a scaled 16-bit index at m16".
    #[test]
    fn a_subtracted_shift_is_not_a_scaled_index_in_real_mode() {
        let mut dag = TestDag::new();
        let base = dag.input(1, mvt::i16);
        let i = dag.input(2, mvt::i16);
        let two = dag.make_constant(2, mvt::i8);
        let scaled = SDValue::new(dag.node(isd::SHL, &[mvt::i16], &[i, two]), 0);
        let difference = SDValue::new(dag.node(isd::SUB, &[mvt::i16], &[base, scaled]), 0);
        let k = dag.make_constant(24, mvt::i16);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i16], &[difference, k]), 0);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag, AddrMode::Real16).match_address(address, &mut am));

        assert_eq!(am.scale, 1, "no scale in 16-bit addressing");
    }

    #[test]
    fn a_word_array_element_has_no_scaled_index_in_real_mode() {
        let mut dag = TestDag::new();
        let (address, a, _, scaled) = word_element(&mut dag);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag, AddrMode::Real16).match_address(address, &mut am));

        assert_eq!((am.base_reg, am.index_reg, am.scale), (Some(a), Some(scaled), 1), "the shl stays a register");
    }

    #[test]
    fn the_same_element_scales_at_32_bits() {
        let mut dag = TestDag::new();
        let (address, a, i, _) = word_element(&mut dag);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag, AddrMode::Flat32).match_address(address, &mut am));

        assert_eq!((am.base_reg, am.index_reg, am.scale), (Some(a), Some(i), 2));
    }

    #[test]
    fn a_displacement_beyond_sixteen_bits_stays_a_register() {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i16);
        let far = dag.make_constant(0x12345, mvt::i32);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i16], &[p, far]), 0);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag, AddrMode::Real16).match_address(address, &mut am));

        assert_eq!(am.disp, 0);
        assert_eq!((am.base_reg, am.index_reg), (Some(p), Some(far)));
    }

    #[test]
    fn a_displacement_at_the_sixteen_bit_limit_folds() {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i16);
        let max = dag.make_constant(65535, mvt::i16);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i16], &[p, max]), 0);
        let mut am = AddressMode::default();

        assert!(!matcher(&mut dag, AddrMode::Real16).match_address(address, &mut am));

        assert_eq!((am.disp, am.index_reg), (65535, None));
    }
}

mod instr_desc {
    use super::*;
    use crate::generated::ids::x86;
    use crate::instr::{desc, operand_type};

    fn opcode(name: &str) -> u16 {
        table::MACHINE_OPCODES
            .iter()
            .find(|(_, n)| n.split('|').any(|n| n == name))
            .unwrap_or_else(|| panic!("{name} is not emitted"))
            .0
    }

    #[test]
    fn add32rr_defines_one_register_ties_the_source_and_sets_flags() {
        let d = desc(opcode("ADD32rr")).unwrap();

        assert_eq!((d.num_operands, d.num_defs), (3, 1));
        assert!(d.is_commutable() && !d.may_load() && !d.is_call());
        assert_eq!(d.implicit_defs, [x86::EFLAGS]);
        assert_eq!(d.operands[1].tied_to, 0, "src1 is the destination");
        assert_eq!(d.operands[0].kind, operand_type::REGISTER);
    }

    #[test]
    fn mov32rm_loads_through_a_memory_reference_of_five_operands() {
        let d = desc(opcode("MOV32rm")).unwrap();

        assert!(d.may_load() && !d.may_store());
        assert_eq!(d.num_operands, 6);
        assert!(d.operands[1].pointer_class, "base is a pointer class");
        assert!(d.operands[1..6].iter().all(|o| o.kind == operand_type::MEMORY), "the pin tags all five as memory");
    }

    #[test]
    fn the_read_modify_write_forms_the_hand_matcher_makes_have_descriptors() {
        assert!(desc(x86::INC32m).unwrap().may_store());
        assert!(desc(x86::ADD32mi).unwrap().may_load());
    }

    #[test]
    fn a_call_is_a_call() {
        assert!(desc(opcode("CALLpcrel32")).unwrap().is_call());
    }

    #[test]
    fn an_opcode_the_tables_never_emit_has_no_descriptor() {
        assert!(desc(0xfff0).is_none());
    }
}

mod far_pointer_loads {
    use super::*;
    use crate::generated::ids::x86isd;
    use crate::generated::x86_m16;
    use crate::testing::{Node, TestDag};
    use crate::x86_addr::AddrMode;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};

    fn hooks(
        cpu: Cpu,
        mode: AddrMode,
    ) -> X86Hooks {
        X86Hooks::new(Options {
            cpu,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: mode,
        })
    }

    /// `X86ISD::LES chain, p`: the offset half of a far pointer loaded through
    /// `p`.
    fn far_load(opcode: u16) -> (TestDag, u32, SDValue) {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i16);
        let node = dag.add(Node {
            opcode,
            vts: vec![mvt::i16, mvt::Other],
            ops: vec![SDValue::new(0, 0), p],
            memory_vt: Some(mvt::i32),
            ..Node::default()
        });
        (dag, node, p)
    }

    fn name_in(
        table: &[(u16, &'static str)],
        dag: &TestDag,
        node: u32,
    ) -> &'static str {
        let opcode = dag.nodes[node as usize].machine.expect("not selected");
        table.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
    }

    #[test]
    fn a_far_load_through_a_near_pointer_is_les_with_the_pointer_as_base() {
        let (mut dag, node, p) = far_load(x86isd::LES);
        let mut hooks = hooks(Cpu::M16, AddrMode::Real16);

        assert_eq!(Selector::new(x86_m16::TABLE).select(&mut dag, &mut hooks, node), Outcome::Selected);

        assert_eq!(name_in(x86_m16::MACHINE_OPCODES, &dag, node), "LES16rm");
        assert_eq!(dag.nodes[node as usize].ops[0], p);
    }

    #[test]
    fn lds_is_selected_the_same_way() {
        let (mut dag, node, _) = far_load(x86isd::LDS);
        let mut hooks = hooks(Cpu::M16, AddrMode::Real16);

        assert_eq!(Selector::new(x86_m16::TABLE).select(&mut dag, &mut hooks, node), Outcome::Selected);

        assert_eq!(name_in(x86_m16::MACHINE_OPCODES, &dag, node), "LDS16rm");
    }

    /// The pin's `lea32addr` is typed i32 and `LEA16r` has no pattern: a frame
    /// object's address at m16 was `CannotSelect` (sieve's three
    /// `FrameIndex` nodes).
    #[test]
    fn the_address_of_a_frame_object_is_lea16r() {
        let mut dag = TestDag::new();
        let frame = dag.frame_index(0, mvt::i16);
        let mut hooks = hooks(Cpu::M16, AddrMode::Real16);

        assert_eq!(Selector::new(x86_m16::TABLE).select(&mut dag, &mut hooks, frame.node), Outcome::Selected);

        assert_eq!(name_in(x86_m16::MACHINE_OPCODES, &dag, frame.node), "LEA16r");
    }

    /// The pin moves a relocatable symbol into an i32 register; at m16 the
    /// wrapped address of a global is an i16 and had no pattern (38
    /// `GlobalAddress` nodes stopped the m16 corpus).
    #[test]
    fn the_address_of_a_global_is_mov16ri() {
        let mut dag = TestDag::new();
        let wrapped = dag.wrapped_global(1, 0, mvt::i16);
        let mut hooks = hooks(Cpu::M16, AddrMode::Real16);

        assert_eq!(Selector::new(x86_m16::TABLE).select(&mut dag, &mut hooks, wrapped.node), Outcome::Selected);

        assert_eq!(name_in(x86_m16::MACHINE_OPCODES, &dag, wrapped.node), "MOV16ri");
    }

    /// Real mode has no scaled index, so a word `lea` cannot take a multiply by
    /// 3, 5 or 9 and the combiner's `MUL_IMM` stayed unselected (8 m16 corpus
    /// functions). A dword one is the 386's 32-bit `lea`, as in the pin.
    #[test]
    fn a_multiply_by_a_lea_constant_is_imul_when_lea_cannot_take_it() {
        for (vt, want) in [(mvt::i16, "IMUL16rri"), (mvt::i32, "LEA32r")] {
            let mut dag = TestDag::new();
            let x = dag.input(1, vt);
            let three = dag.make_constant(3, vt);
            let node = dag.node(x86isd::MUL_IMM, &[vt], &[x, three]);
            let mut hooks = hooks(Cpu::M16, AddrMode::Real16);

            assert_eq!(Selector::new(x86_m16::TABLE).select(&mut dag, &mut hooks, node), Outcome::Selected);

            assert_eq!(name_in(x86_m16::MACHINE_OPCODES, &dag, node), want);
        }
    }

    #[test]
    fn the_pins_own_table_has_no_pattern_for_a_far_load() {
        let (mut dag, node, _) = far_load(x86isd::LES);
        let mut hooks = hooks(Cpu::I486, AddrMode::Flat32);

        assert_eq!(Selector::new(table::TABLE).select(&mut dag, &mut hooks, node), Outcome::CannotSelect);
    }
}

mod pointer_classes {
    use super::*;
    use crate::instr::desc;
    use crate::x86_addr::AddrMode;
    use crate::x86_classes::{Role, class_tag, memory_reference, role_of};

    fn opcode(name: &str) -> u16 {
        table::MACHINE_OPCODES
            .iter()
            .find(|(_, n)| n.split('|').any(|n| n == name))
            .unwrap_or_else(|| panic!("{name}"))
            .0
    }

    #[test]
    fn a_load_has_its_reference_after_the_destination() {
        let d = desc(opcode("MOV32rm")).unwrap();

        assert_eq!(memory_reference(d), Some(1));
        assert_eq!(role_of(d, 0), None, "the destination is not part of it");
        assert_eq!(
            (role_of(d, 1), role_of(d, 3), role_of(d, 5)),
            (Some(Role::Base), Some(Role::Index), Some(Role::Segment))
        );
    }

    #[test]
    fn a_read_modify_write_has_its_reference_first() {
        let d = desc(opcode("ADD32mr")).unwrap();

        assert_eq!((memory_reference(d), role_of(d, 4), role_of(d, 5)), (Some(0), Some(Role::Segment), None));
    }

    #[test]
    fn an_instruction_with_no_memory_operand_has_no_reference() {
        assert_eq!(memory_reference(desc(opcode("ADD32rr")).unwrap()), None);
    }

    #[test]
    fn in_real_mode_the_base_and_the_index_have_their_own_registers() {
        assert_eq!(class_tag(AddrMode::Real16, Role::Base), Some("base"));
        assert_eq!(class_tag(AddrMode::Real16, Role::Index), Some("index"));
        assert_eq!(class_tag(AddrMode::Real16, Role::Disp), None);
        assert_eq!(class_tag(AddrMode::Flat32, Role::Base), None, "any general register at 32 bits");
    }
}

mod string_instructions {
    use super::*;
    use crate::dag::Dag;
    use crate::generated::ids::{x86, x86isd};
    use crate::generated::x86_m16;
    use crate::testing::TestDag;
    use crate::x86_addr::AddrMode;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};

    fn hooks(cpu: Cpu) -> X86Hooks {
        let address_mode = if cpu == Cpu::M16 { AddrMode::Real16 } else { AddrMode::Flat32 };
        X86Hooks::new(Options {
            cpu,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode,
        })
    }

    /// `rep movs` of `unit`-typed elements, after the count register is set.
    fn rep(
        opcode: u16,
        unit: crate::matcher::Vt,
    ) -> (TestDag, u32) {
        let mut dag = TestDag::new();
        let count = dag.input(1, mvt::i16);
        let copy = dag.copy_to_reg(SDValue::new(0, 0), x86::CX, count, None);
        let vt = dag.make_value_type_node(unit);
        let node = dag.node(opcode, &[mvt::Other, mvt::Glue], &[SDValue::new(copy, 0), vt, SDValue::new(copy, 1)]);
        (dag, node)
    }

    fn name(
        table: &[(u16, &'static str)],
        dag: &TestDag,
        node: u32,
    ) -> &'static str {
        let opcode = dag.nodes[node as usize].machine.expect("not selected");
        table.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
    }

    #[test]
    fn rep_movs_of_words_uses_si_di_cx_in_real_mode() {
        let (mut dag, node) = rep(x86isd::REP_MOVS, mvt::i16);

        assert_eq!(Selector::new(x86_m16::TABLE).select(&mut dag, &mut hooks(Cpu::M16), node), Outcome::Selected);

        assert_eq!(name(x86_m16::MACHINE_OPCODES, &dag, node), "REP_MOVSW_16");
    }

    #[test]
    fn rep_stos_of_bytes_is_the_16_bit_form_too() {
        let (mut dag, node) = rep(x86isd::REP_STOS, mvt::i8);

        assert_eq!(Selector::new(x86_m16::TABLE).select(&mut dag, &mut hooks(Cpu::M16), node), Outcome::Selected);

        assert_eq!(name(x86_m16::MACHINE_OPCODES, &dag, node), "REP_STOSB_16");
    }

    #[test]
    fn the_flat_table_still_picks_the_32_bit_form() {
        let (mut dag, node) = rep(x86isd::REP_MOVS, mvt::i16);

        assert_eq!(Selector::new(table::TABLE).select(&mut dag, &mut hooks(Cpu::I486), node), Outcome::Selected);

        assert_eq!(name(table::MACHINE_OPCODES, &dag, node), "REP_MOVSW_32");
    }
}

mod segment_overrides {
    use super::*;
    use crate::dag::LoadExt;
    use crate::generated::ids::x86;
    use crate::testing::{Node, TestDag};
    use crate::x86_addr::AddrMode;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};

    fn load_through(segment: Option<u16>) -> (TestDag, u32, SDValue) {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i16);
        let off = SDValue::new(dag.node(isd::UNDEF, &[mvt::i16], &[]), 0);
        let load = dag.add(Node {
            opcode: isd::LOAD,
            vts: vec![mvt::i16, mvt::Other],
            ops: vec![SDValue::new(0, 0), p, off],
            load: Some(LoadExt::NonExt),
            memory_vt: Some(mvt::i16),
            segment,
            ..Node::default()
        });
        (dag, load, p)
    }

    fn select(
        dag: &mut TestDag,
        load: u32,
    ) {
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::M16,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: AddrMode::Real16,
        });
        assert_eq!(Selector::new(crate::generated::x86_m16::TABLE).select(dag, &mut hooks, load), Outcome::Selected);
    }

    #[test]
    fn a_load_through_es_has_es_as_its_segment_operand() {
        let (mut dag, load, _) = load_through(Some(x86::ES));
        select(&mut dag, load);

        let segment = dag.nodes[load as usize].ops[4];
        assert_eq!(dag.nodes[segment.node as usize].register, Some(x86::ES));
    }

    #[test]
    fn a_load_with_no_override_leaves_the_segment_empty() {
        let (mut dag, load, _) = load_through(None);
        select(&mut dag, load);

        let segment = dag.nodes[load as usize].ops[4];
        assert_eq!(dag.nodes[segment.node as usize].register, Some(0), "register 0 is no segment");
    }
}

mod shift_amount_mod {
    use super::*;
    use crate::testing::TestDag;
    use crate::x86_shift::try_shift_amount_mod;

    /// `x << amount(n)`.
    fn shl(
        dag: &mut TestDag,
        amount: impl FnOnce(&mut TestDag, SDValue) -> SDValue,
    ) -> (u32, SDValue) {
        let x = dag.input(1, mvt::i32);
        let n = dag.input(2, mvt::i8);
        let count = amount(dag, n);
        (dag.node(isd::SHL, &[mvt::i32], &[x, count]), n)
    }

    /// The count of the shift after the rewrite: `(n' & 31)`; returns `n'`.
    fn masked_source(
        dag: &TestDag,
        shift: u32,
    ) -> SDValue {
        let count = dag.nodes[shift as usize].ops[1];
        assert_eq!(dag.nodes[count.node as usize].opcode, isd::AND, "the count is masked");
        assert_eq!(dag.nodes[dag.nodes[count.node as usize].ops[1].node as usize].constant, Some(31));
        dag.nodes[count.node as usize].ops[0]
    }

    #[test]
    fn n_plus_32_shifts_by_n() {
        let mut dag = TestDag::new();
        let (shift, n) = shl(&mut dag, |d, n| {
            let c = d.make_constant(32, mvt::i8);
            SDValue::new(d.node(isd::ADD, &[mvt::i8], &[n, c]), 0)
        });

        assert_eq!(try_shift_amount_mod(&mut dag, shift), Some(shift));

        assert_eq!(masked_source(&dag, shift), n);
    }

    #[test]
    fn thirty_one_minus_n_shifts_by_not_n() {
        let mut dag = TestDag::new();
        let (shift, n) = shl(&mut dag, |d, n| {
            let c = d.make_constant(31, mvt::i8);
            SDValue::new(d.node(isd::SUB, &[mvt::i8], &[c, n]), 0)
        });

        assert_eq!(try_shift_amount_mod(&mut dag, shift), Some(shift));

        let not = masked_source(&dag, shift);
        assert_eq!(dag.nodes[not.node as usize].opcode, isd::XOR, "n ^ -1");
        assert_eq!(dag.nodes[not.node as usize].ops[0], n);
    }

    #[test]
    fn thirty_two_minus_n_shifts_by_minus_n() {
        let mut dag = TestDag::new();
        let (shift, n) = shl(&mut dag, |d, n| {
            let c = d.make_constant(32, mvt::i8);
            SDValue::new(d.node(isd::SUB, &[mvt::i8], &[c, n]), 0)
        });

        assert_eq!(try_shift_amount_mod(&mut dag, shift), Some(shift));

        let neg = masked_source(&dag, shift);
        assert_eq!(dag.nodes[neg.node as usize].opcode, isd::SUB, "0 - n");
        assert_eq!(dag.nodes[neg.node as usize].ops[1], n);
    }

    #[test]
    fn an_ordinary_count_is_left_alone() {
        let mut dag = TestDag::new();
        let (shift, _) = shl(&mut dag, |d, n| {
            let c = d.make_constant(5, mvt::i8);
            SDValue::new(d.node(isd::ADD, &[mvt::i8], &[n, c]), 0)
        });

        assert_eq!(try_shift_amount_mod(&mut dag, shift), None);
    }
}

mod x87_pseudos {
    use super::*;
    use crate::dag::LoadExt;
    use crate::testing::{Node, TestDag};
    use crate::x86_addr::AddrMode;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};

    fn hooks() -> X86Hooks {
        X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: AddrMode::Flat32,
        })
    }

    #[test]
    fn a_double_add_of_two_registers_is_the_pseudo_for_the_stack_machine() {
        let mut dag = TestDag::new();
        let (a, b) = (dag.input(1, mvt::f64), dag.input(2, mvt::f64));
        let sum = dag.node(isd::FADD, &[mvt::f64], &[a, b]);

        assert_eq!(Selector::new(table::TABLE).select(&mut dag, &mut hooks(), sum), Outcome::Selected);

        assert_eq!(selected(&dag, sum), "ADD_Fp64");
    }

    #[test]
    fn a_double_add_of_a_loaded_operand_folds_the_load_into_the_add() {
        let mut dag = TestDag::new();
        let a = dag.input(1, mvt::f64);
        let p = dag.input(2, mvt::i32);
        let off = SDValue::new(dag.node(isd::UNDEF, &[mvt::i32], &[]), 0);
        let load = dag.add(Node {
            opcode: isd::LOAD,
            vts: vec![mvt::f64, mvt::Other],
            ops: vec![SDValue::new(0, 0), p, off],
            load: Some(LoadExt::NonExt),
            memory_vt: Some(mvt::f64),
            ..Node::default()
        });
        let sum = dag.node(isd::FADD, &[mvt::f64], &[a, SDValue::new(load, 0)]);

        assert_eq!(Selector::new(table::TABLE).select(&mut dag, &mut hooks(), sum), Outcome::Selected);

        assert_eq!(selected(&dag, sum), "ADD_Fp64m", "fadd [m], not fld then faddp");
    }
}

#[test]
fn a_machine_opcode_is_named_for_its_instruction_not_for_a_register_that_shares_the_number() {
    // `X86::CS` (a register) and `SUBREG_TO_REG` (an instruction) are both 17;
    // the name column once read `CS|SUBREG_TO_REG|VK2RegClassID`.
    for (_, name) in table::MACHINE_OPCODES {
        assert!(
            !name.contains("RegClassID") && name.split('|').all(|n| !["CS", "DS", "ES", "FS", "GS", "SS"].contains(&n)),
            "{name}"
        );
    }
}

mod select_loop {
    use super::*;
    use crate::dag::LoadExt;
    use crate::generated::ids::{x86, x86isd};
    use crate::testing::{Node, TestDag};
    use crate::x86_addr::AddrMode;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_select::select_node;

    fn hooks() -> X86Hooks {
        X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: AddrMode::Flat32,
        })
    }

    #[test]
    fn a_store_of_a_loaded_sum_is_one_read_modify_write_instruction() {
        // `*p += r`: load, X86ISD::ADD, store.
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i32);
        let r = dag.input(2, mvt::i32);
        let off = SDValue::new(dag.node(isd::UNDEF, &[mvt::i32], &[]), 0);
        let load = dag.add(Node {
            opcode: isd::LOAD,
            vts: vec![mvt::i32, mvt::Other],
            ops: vec![SDValue::new(0, 0), p, off],
            load: Some(LoadExt::NonExt),
            memory_vt: Some(mvt::i32),
            ..Node::default()
        });
        let add = dag.node(x86isd::ADD, &[mvt::i32, mvt::i32], &[SDValue::new(load, 0), r]);
        let store = dag.add(Node {
            opcode: isd::STORE,
            vts: vec![mvt::Other],
            ops: vec![SDValue::new(load, 1), SDValue::new(add, 0), p, off],
            memory_vt: Some(mvt::i32),
            ..Node::default()
        });

        let outcome = select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks(), store);

        assert_eq!(outcome, Outcome::Selected);
        let names: Vec<_> = dag.live().filter_map(|(_, n)| n.machine.map(machine_name)).collect();
        assert_eq!(names, ["ADD32mr"], "not a load, an add and a store");
    }

    #[test]
    fn a_shift_by_n_plus_32_copies_n_itself_into_cl() {
        let mut dag = TestDag::new();
        let x = dag.input(1, mvt::i32);
        let n = dag.input(2, mvt::i8);
        let thirty_two = dag.make_constant(32, mvt::i8);
        let count = SDValue::new(dag.node(isd::ADD, &[mvt::i8], &[n, thirty_two]), 0);
        let shift = dag.node(isd::SHL, &[mvt::i32], &[x, count]);

        let outcome = select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks(), shift);

        assert_eq!(outcome, Outcome::Selected);
        let to_cl = dag
            .live()
            .find(|(_, n)| n.opcode == isd::CopyToReg && dag.nodes[n.ops[1].node as usize].register == Some(x86::CL));
        assert_eq!(to_cl.expect("the count goes to CL").1.ops[2], n, "the +32 is gone");
    }
}

mod nodes_before_the_table {
    use super::*;
    use crate::generated::ids::target_opcode;
    use crate::testing::TestDag;

    fn select(
        dag: &mut TestDag,
        node: u32,
    ) -> Outcome {
        Selector::new(table::TABLE).select(dag, &mut crate::testing::TestHooks { address: None }, node)
    }

    #[test]
    fn undef_becomes_implicit_def_not_an_unselectable_node() {
        let mut dag = TestDag::new();
        let undef = dag.node(isd::UNDEF, &[mvt::i32], &[]);

        assert_eq!(select(&mut dag, undef), Outcome::Selected);

        assert_eq!(dag.nodes[undef as usize].machine, Some(target_opcode::IMPLICIT_DEF));
    }

    #[test]
    fn freeze_becomes_a_copy_of_its_operand() {
        let mut dag = TestDag::new();
        let x = dag.input(1, mvt::i32);
        let freeze = dag.node(isd::FREEZE, &[mvt::i32], &[x]);

        assert_eq!(select(&mut dag, freeze), Outcome::Selected);

        assert_eq!(dag.nodes[freeze as usize].machine, Some(target_opcode::COPY));
        assert_eq!(dag.nodes[freeze as usize].ops, [x]);
    }

    #[test]
    fn an_assertion_is_replaced_by_the_value_it_asserts_about() {
        let mut dag = TestDag::new();
        let x = dag.input(1, mvt::i32);
        let asserted = dag.node(isd::AssertZext, &[mvt::i32], &[x]);
        let user = dag.node(isd::ADD, &[mvt::i32], &[SDValue::new(asserted, 0), x]);

        assert_eq!(select(&mut dag, asserted), Outcome::Selected);

        assert_eq!(dag.nodes[user as usize].ops[0], x, "the add reads x, not the assertion");
        assert!(dag.nodes[asserted as usize].dead);
    }
}

#[test]
fn an_x86isd_node_is_found_by_name_in_the_pins_numbers() {
    // Builders spell their own target codes and name them; the pin's number
    // comes from the name.
    use crate::generated::ids::{x86isd, x86isd_id};
    assert_eq!(x86isd_id("RET_GLUE"), Some(x86isd::RET_GLUE));
    assert_eq!(x86isd_id("CMOV"), Some(x86isd::CMOV));
    assert_eq!(x86isd_id("NO_SUCH_NODE"), None);
}

#[test]
fn a_return_glue_node_is_selected_to_ret() {
    use crate::generated::ids::x86isd;
    let mut dag = crate::testing::TestDag::new();
    let zero = dag.target_constant(0, mvt::i32);
    let ret = dag.node(x86isd::RET_GLUE, &[mvt::Other], &[SDValue::new(0, 0), zero]);

    let mut hooks = crate::x86_hooks::X86Hooks::new(crate::x86_hooks::Options {
        cpu: crate::x86_hooks::Cpu::I486,
        opt_none: false,
        opt_for_size: false,
        min_size: false,
        has_frame_pointer: true,
        address_mode: crate::x86_addr::AddrMode::Flat32,
    });

    assert_eq!(
        crate::x86_select::select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, ret),
        Outcome::Selected
    );

    assert_eq!(selected(&dag, ret), "RET");
}

mod symbol_plus_two_registers {
    use super::*;
    use crate::testing::TestDag;
    use crate::x86_addr::{AddrMode, AddressMode, Matcher, SymbolKind};

    /// `@sym + x + (y << 2)`, associated either way.
    fn address(
        dag: &mut TestDag,
        symbol_first: bool,
    ) -> (SDValue, SDValue, SDValue) {
        let sym = dag.wrapped_global(7, 0, mvt::i32);
        let (x, y) = (dag.input(1, mvt::i32), dag.input(2, mvt::i32));
        let two = dag.make_constant(2, mvt::i8);
        let scaled = SDValue::new(dag.node(isd::SHL, &[mvt::i32], &[y, two]), 0);
        let address = if symbol_first {
            let sym_x = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[sym, x]), 0);
            SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[sym_x, scaled]), 0)
        } else {
            let x_scaled = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[x, scaled]), 0);
            SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[sym, x_scaled]), 0)
        };
        (address, x, y)
    }

    #[test]
    fn a_symbol_with_a_register_and_a_scaled_register_is_one_memory_operand() {
        for symbol_first in [true, false] {
            let mut dag = TestDag::new();
            let (address, x, y) = address(&mut dag, symbol_first);
            let mut am = AddressMode::default();

            assert!(
                !Matcher { dag: &mut dag, pointer_type: mvt::i32, mode: AddrMode::Flat32, size: false }
                    .match_address(address, &mut am)
            );

            let symbol = am.symbol.expect("the symbol is the displacement");
            assert_eq!((symbol.kind, symbol.id), (SymbolKind::Global, 7), "symbol_first={symbol_first}");
            assert_eq!(
                (am.base_reg, am.index_reg, am.scale),
                (Some(x), Some(y), 4),
                "[sym + x + y*4], symbol_first={symbol_first}"
            );
        }
    }
}

mod divrem {
    use super::*;
    use crate::generated::ids::x86;
    use crate::testing::TestDag;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_select::select_node;

    /// `{q, r} = UDIVREM/SDIVREM a, b` (i32) with both results used; returns
    /// the dag and the node.
    fn divide(
        opcode: u16,
        a: SDValue,
        b: SDValue,
        dag: &mut TestDag,
    ) -> u32 {
        let node = dag.node(opcode, &[mvt::i32, mvt::i32], &[a, b]);
        dag.node(isd::ADD, &[mvt::i32], &[SDValue::new(node, 0), SDValue::new(node, 1)]);
        node
    }

    fn run(
        dag: &mut TestDag,
        node: u32,
    ) -> Outcome {
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        });
        select_node(&Selector::new(table::TABLE), dag, &mut hooks, node)
    }

    fn machine(dag: &TestDag) -> Vec<u16> {
        dag.nodes.iter().filter(|n| !n.dead).filter_map(|n| n.machine).collect()
    }

    /// Without the hand case the table has no pattern for DIVREM: a division
    /// was `CannotSelect`.
    #[test]
    fn an_unsigned_divide_is_div_after_clearing_edx() {
        let mut dag = TestDag::new();
        let (a, b) = (dag.input(1, mvt::i32), dag.input(2, mvt::i32));
        let node = divide(isd::UDIVREM, a, b, &mut dag);
        assert_eq!(run(&mut dag, node), Outcome::Selected);
        let m = machine(&dag);
        assert!(m.contains(&x86::MOV32r0) && m.contains(&x86::DIV32r) && !m.contains(&x86::CDQ), "{m:?}");
    }

    #[test]
    fn a_signed_divide_sign_extends_with_cdq() {
        let mut dag = TestDag::new();
        let (a, b) = (dag.input(1, mvt::i32), dag.input(2, mvt::i32));
        let node = divide(isd::SDIVREM, a, b, &mut dag);
        assert_eq!(run(&mut dag, node), Outcome::Selected);
        let m = machine(&dag);
        assert!(m.contains(&x86::CDQ) && m.contains(&x86::IDIV32r) && !m.contains(&x86::MOV32r0), "{m:?}");
    }

    #[test]
    fn a_signed_divide_of_a_non_negative_dividend_clears_edx() {
        let mut dag = TestDag::new();
        let (a, b) = (dag.input(1, mvt::i32), dag.input(2, mvt::i32));
        let mask = dag.make_constant(0xff, mvt::i32);
        let small = SDValue::new(dag.node(isd::AND, &[mvt::i32], &[a, mask]), 0);
        let node = divide(isd::SDIVREM, small, b, &mut dag);
        assert_eq!(run(&mut dag, node), Outcome::Selected);
        let m = machine(&dag);
        assert!(m.contains(&x86::MOV32r0) && m.contains(&x86::IDIV32r) && !m.contains(&x86::CDQ), "{m:?}");
    }
}

mod masked_operand_that_is_not_a_shift {
    use super::*;
    use crate::testing::TestDag;
    use crate::x86_addr::{AddrMode, AddressMode, Matcher};

    /// `a + (zext(b) & 0xff)` in an address: the AND's first operand has one
    /// operand, not a shift. The mask fold read operand 1 of that register
    /// before checking it was a shift, and panicked (x_primes and x_rle
    /// crashed the shadow).
    #[test]
    fn an_and_of_a_unary_node_does_not_read_a_shift_amount() {
        let mut dag = TestDag::new();
        let (a, b) = (dag.input(1, mvt::i32), dag.input(2, mvt::i8));
        let x = SDValue::new(dag.node(isd::ZERO_EXTEND, &[mvt::i32], &[b]), 0);
        let mask = dag.make_constant(0xff, mvt::i32);
        let masked = SDValue::new(dag.node(isd::AND, &[mvt::i32], &[x, mask]), 0);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[a, masked]), 0);
        let mut am = AddressMode::default();

        Matcher { dag: &mut dag, pointer_type: mvt::i32, mode: AddrMode::Flat32, size: false }
            .match_address(address, &mut am);
    }
}

mod mul_lohi {
    use super::*;
    use crate::generated::ids::x86;
    use crate::testing::TestDag;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_select::select_node;

    fn machine(dag: &TestDag) -> Vec<u16> {
        dag.nodes.iter().filter(|n| !n.dead).filter_map(|n| n.machine).collect()
    }

    /// `hi:lo = *MUL_LOHI a, b` (i32), both halves used, as the
    /// division-by-constant combine makes.
    fn multiply(opcode: u16) -> Vec<u16> {
        let mut dag = TestDag::new();
        let (a, b) = (dag.input(1, mvt::i32), dag.input(2, mvt::i32));
        let node = dag.node(opcode, &[mvt::i32, mvt::i32], &[a, b]);
        dag.node(isd::ADD, &[mvt::i32], &[SDValue::new(node, 0), SDValue::new(node, 1)]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        });
        assert_eq!(select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, node), Outcome::Selected);
        machine(&dag)
    }

    /// Without the hand case the table has no pattern for MUL_LOHI: it was
    /// `CannotSelect`.
    #[test]
    fn an_unsigned_wide_multiply_is_mul32r() {
        assert!(multiply(isd::UMUL_LOHI).contains(&x86::MUL32r));
    }

    #[test]
    fn a_signed_wide_multiply_is_imul32r() {
        assert!(multiply(isd::SMUL_LOHI).contains(&x86::IMUL32r));
    }
}

mod llrm_intrinsics {
    use super::*;
    use crate::testing::TestDag;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_llrm::{LLRM_ASM_OPCODE, LLRM_NODES, SEGMENT_OF, SEGMENT_OF_OPCODE};
    use crate::x86_select::select_node;

    /// No pattern covers `X86ISD::LLRM_ASM`: it was `CannotSelect` and stopped
    /// every function that holds an `llrm.ia16.asm` call.
    #[test]
    fn an_asm_node_becomes_the_pseudo_with_its_operands() {
        for opcode in LLRM_NODES {
            let mut dag = TestDag::new();
            let chain = dag.entry_node();
            let reg = dag.input(1, mvt::i16);
            let node = dag.node(opcode, &[mvt::Other, mvt::Glue], &[chain, reg]);
            let mut hooks = X86Hooks::new(Options {
                cpu: Cpu::M16,
                opt_none: false,
                opt_for_size: false,
                min_size: false,
                has_frame_pointer: true,
                address_mode: crate::x86_addr::AddrMode::Real16,
            });

            assert_eq!(select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, node), Outcome::Selected);

            let n = &dag.nodes[node as usize];
            assert_eq!(
                (n.machine, n.ops.clone(), n.vts.clone()),
                (Some(LLRM_ASM_OPCODE), vec![chain, reg], vec![mvt::Other, mvt::Glue])
            );
        }
    }

    /// `SEGMENT_OF` (id 24580 collided with `LLRM_CODE` in the first version)
    /// is a segment value: it becomes its own pseudo, with the global as
    /// operand or none for `DGROUP`.
    #[test]
    fn a_segment_of_a_global_and_of_dgroup_are_the_segment_pseudo() {
        for with_global in [true, false] {
            let mut dag = TestDag::new();
            let ops: Vec<SDValue> = if with_global { vec![dag.wrapped_global(1, 0, mvt::i16)] } else { vec![] };
            let node = dag.node(SEGMENT_OF, &[mvt::i16], &ops);
            let mut hooks = X86Hooks::new(Options {
                cpu: Cpu::M16,
                opt_none: false,
                opt_for_size: false,
                min_size: false,
                has_frame_pointer: true,
                address_mode: crate::x86_addr::AddrMode::Real16,
            });

            assert_eq!(select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, node), Outcome::Selected);

            let n = &dag.nodes[node as usize];
            assert_eq!((n.machine, n.ops.clone(), n.vts.clone()), (Some(SEGMENT_OF_OPCODE), ops, vec![mvt::i16]));
        }
    }

    /// `LLRM_FARCALL` is a target node with no pattern: it passes through as
    /// its own pseudo, with the call's operands and its chain and glue
    /// results.
    #[test]
    fn a_far_call_passes_through_as_its_pseudo() {
        use crate::x86_llrm::{FARCALL_OPCODE, LLRM_FARCALL};
        let mut dag = TestDag::new();
        let chain = dag.entry_node();
        let target = dag.wrapped_global(1, 0, mvt::i16);
        let node = dag.node(LLRM_FARCALL, &[mvt::Other, mvt::Glue], &[chain, target]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::M16,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Real16,
        });

        assert_eq!(select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, node), Outcome::Selected);

        let n = &dag.nodes[node as usize];
        assert_eq!(
            (n.machine, n.ops.clone(), n.vts.clone()),
            (Some(FARCALL_OPCODE), vec![chain, target], vec![mvt::Other, mvt::Glue])
        );
    }

    /// `LLRM_CODE` (24582, sdag-a's `llrm.ia16.code`) is its own pseudo, not
    /// the asm one.
    #[test]
    fn a_code_node_is_the_code_pseudo() {
        use crate::x86_llrm::{CODE_OPCODE, LLRM_CODE};
        let mut dag = TestDag::new();
        let chain = dag.entry_node();
        let node = dag.node(LLRM_CODE, &[mvt::Other, mvt::Glue], &[chain]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::M16,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Real16,
        });

        assert_eq!(select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, node), Outcome::Selected);

        assert_eq!(dag.nodes[node as usize].machine, Some(CODE_OPCODE));
    }
}

mod instruction_names {
    use crate::generated::ids::x86;

    /// The hand matchers' opcodes (`CDQ`, `DIV32r`, `MUL32r`) were not in the
    /// per-cpu name table, so the emitter refused 25 functions with "opcode
    /// 1202".
    #[test]
    fn the_hand_matchers_instructions_have_names() {
        let name = |opcode| x86::NAMES.iter().find(|(o, _)| *o == opcode).map(|(_, n)| *n);
        assert_eq!(name(x86::CDQ), Some("CDQ"));
        assert_eq!(name(x86::DIV32r), Some("DIV32r"));
        assert_eq!(name(x86::MUL32r), Some("MUL32r"));
        assert_eq!(name(x86::EAX), None, "a register is not an instruction");
    }
}

mod fcmp_without_fcomi {
    use super::*;
    use crate::generated::ids::{x86, x86isd};
    use crate::testing::TestDag;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_select::select_node;

    /// `fucom; fnstsw ax; sahf`: the table's `UCOM_FpIr64` needs `fcomi`, so an
    /// `FCMP` was `CannotSelect` on every CPU here (the qbq corpus: 4
    /// functions).
    #[test]
    fn a_double_compare_is_fucom_fnstsw_sahf() {
        let mut dag = TestDag::new();
        let (a, b) = (dag.input(1, mvt::f64), dag.input(2, mvt::f64));
        let cmp = dag.node(x86isd::FCMP, &[mvt::i32], &[a, b]);
        let user = dag.node(x86isd::SETCC, &[mvt::i8], &[SDValue::new(cmp, 0)]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        });

        assert_eq!(select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, cmp), Outcome::Selected);

        let flags = dag.nodes[user as usize].ops[0];
        assert_eq!(dag.nodes[flags.node as usize].machine, Some(x86::SAHF));
        let machine: Vec<u16> = dag.nodes.iter().filter(|n| !n.dead).filter_map(|n| n.machine).collect();
        assert!(machine.contains(&x86::UCOM_Fpr64) && machine.contains(&x86::FNSTSW16r), "{machine:?}");
    }
}

mod pop_from_x87 {
    use super::*;
    use crate::generated::ids::x86isd;
    use crate::testing::TestDag;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_select::select_node;

    /// A call's float result is `POP_FROM_X87_REG chain, $fp0, glue`: no
    /// pattern, the pin's `Select` turns it into a `CopyFromReg` (7 m32
    /// execution programs stopped at `CannotSelect`).
    #[test]
    fn a_float_call_result_becomes_a_copy_from_the_x87_register() {
        let mut dag = TestDag::new();
        let chain = dag.entry_node();
        let fp0 = dag.input(9, mvt::f64);
        let glue = dag.input(10, mvt::Glue);
        let pop = dag.node(x86isd::POP_FROM_X87_REG, &[mvt::f64, mvt::Other, mvt::Glue], &[chain, fp0, glue]);
        let user = dag.node(isd::FADD, &[mvt::f64], &[SDValue::new(pop, 0), fp0]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        });

        assert_eq!(select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, pop), Outcome::Selected);

        let read = dag.nodes[user as usize].ops[0];
        let copy = &dag.nodes[read.node as usize];
        assert_eq!((copy.opcode, copy.ops.clone()), (isd::CopyFromReg, vec![chain, fp0, glue]));
        assert_eq!(copy.vts, vec![mvt::f64, mvt::Other, mvt::Glue]);
    }
}

mod decrement_for_flags {
    use super::*;
    use crate::generated::ids::{x86, x86isd};
    use crate::testing::TestDag;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_select::select_node;

    const JCC_1: u16 = 2001;
    const COND_NE: i64 = 5;
    const COND_B: i64 = 2;

    /// `add x, -1` whose flags reach a `jcc` through `CopyToReg EFLAGS`, as
    /// selecting the branch first leaves them.
    fn selected_with_branch_on(cond: i64) -> &'static str {
        let mut dag = TestDag::new();
        let x = dag.input(1, mvt::i32);
        let minus_one = dag.make_constant(-1, mvt::i32);
        let add = dag.node(x86isd::ADD, &[mvt::i32, mvt::i32], &[x, minus_one]);
        let eflags = dag.register(x86::EFLAGS, mvt::i32);
        let copy = dag.add(Node {
            opcode: isd::CopyToReg,
            vts: vec![mvt::Other, mvt::Glue],
            ops: vec![SDValue::new(0, 0), eflags, SDValue::new(add, 1)],
            ..Node::default()
        });
        let target = dag.input(3, mvt::Other);
        let condition = dag.make_constant(cond, mvt::i8);
        dag.add(Node {
            opcode: !JCC_1,
            machine: Some(JCC_1),
            vts: vec![mvt::Other],
            ops: vec![target, condition, SDValue::new(copy, 0), SDValue::new(copy, 1)],
            ..Node::default()
        });
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        });
        assert_eq!(select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, add), Outcome::Selected);
        selected(&dag, add)
    }

    /// The loop counter of `x_iir` was `add eax, -1; test eax, eax; jne`:
    /// `hasNoCarryFlagUses` refused every use through `CopyToReg EFLAGS`
    /// (SD37), so `DEC32r` was `ADD32ri8` plus a `test` (3 of the m32 shadow's
    /// pin-only `DEC32r` rows).
    #[test]
    fn a_decrement_whose_flags_only_a_jne_reads_is_dec32r() {
        assert_eq!(selected_with_branch_on(COND_NE), "DEC32r");
    }

    #[test]
    fn a_decrement_whose_carry_a_jb_reads_stays_an_add() {
        assert_ne!(selected_with_branch_on(COND_B), "DEC32r");
    }
}

mod and_with_a_low_mask {
    use super::*;
    use crate::testing::TestDag;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_select::select_node;

    fn selected_names(mask: i64) -> Vec<&'static str> {
        let mut dag = TestDag::new();
        let x = dag.input(1, mvt::i32);
        let constant = dag.make_constant(mask, mvt::i32);
        let and = dag.node(isd::AND, &[mvt::i32], &[x, constant]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        });
        assert_eq!(select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, and), Outcome::Selected);
        dag.nodes.iter().filter_map(|n| n.machine).map(machine_name).collect()
    }

    /// `x & 255` is `movzx` of the low byte, as the pin selects it. The hook
    /// for `CheckAndImm` answered false, so `AND32ri` was made (x_dct: 8 of
    /// them where the pin has `MOVZX32rr8`).
    #[test]
    fn and_255_is_a_zero_extension_of_the_low_byte() {
        assert!(selected_names(255).contains(&"MOVZX32rr8"), "{:?}", selected_names(255));
    }

    #[test]
    fn and_65535_is_a_zero_extension_of_the_low_word() {
        assert!(selected_names(65535).contains(&"MOVZX32rr16"), "{:?}", selected_names(65535));
    }

    #[test]
    fn and_with_another_mask_stays_an_and() {
        assert_eq!(selected_names(4095), ["AND32ri"]);
    }
}

mod setcc_carry {
    use super::*;
    use crate::generated::ids::{x86, x86isd};
    use crate::testing::TestDag;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_select::select_node;

    fn selected_names(vt: crate::matcher::Vt) -> Vec<&'static str> {
        let mut dag = TestDag::new();
        let flags = dag.input(1, mvt::i32);
        let cc = dag.make_constant(2, mvt::i8);
        let node = dag.node(x86isd::SETCC_CARRY, &[vt], &[cc, flags]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        });
        assert_eq!(select_node(&Selector::new(table::TABLE), &mut dag, &mut hooks, node), Outcome::Selected);
        dag.nodes.iter().filter(|n| !n.dead).filter_map(|n| n.machine).map(machine_name).collect()
    }

    /// The carry as all ones or zero is `sbb r, r` on the zeroed register
    /// (the pin's `getSBBZero`): `CannotSelect(SETCC_CARRY)` left crc's seven
    /// as a pin-only `SBB32rr` in the shadow diff.
    #[test]
    fn a_setcc_carry_is_sbb_of_a_zero() {
        let names = selected_names(mvt::i32);
        assert!(names.contains(&"SBB32rr") && names.contains(&"MOV32r0"), "{names:?}");
        assert!(!names.contains(&"EXTRACT_SUBREG"), "{names:?}");
    }

    #[test]
    fn a_byte_setcc_carry_takes_the_low_byte() {
        let names = selected_names(mvt::i8);
        assert!(names.contains(&"SBB32rr") && names.contains(&"EXTRACT_SUBREG"), "{names:?}");
        let _ = x86::SBB32rr;
    }
}

mod x64_table {
    use super::*;
    use crate::generated::x86_x64 as x64;
    use crate::testing::TestDag;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};
    use crate::x86_select::select_node;

    fn names(opcode: u16) -> &'static str {
        x64::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
    }

    fn selected_names(vt: crate::matcher::Vt) -> Vec<&'static str> {
        let mut dag = TestDag::new();
        let (a, b) = (dag.input(1, vt), dag.input(2, vt));
        let add = dag.node(isd::ADD, &[vt], &[a, b]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::X64,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        });
        assert_eq!(select_node(&Selector::new(x64::TABLE), &mut dag, &mut hooks, add), Outcome::Selected);
        dag.nodes.iter().filter_map(|n| n.machine).map(names).collect()
    }

    /// The pin's table in 64-bit mode: an `i64` add is `ADD64rr`, and the
    /// 32-bit add is still `ADD32rr`. There was no 64-bit table at all
    /// (D64-0).
    #[test]
    fn an_i64_add_is_add64rr() {
        assert_eq!(selected_names(mvt::i64), ["ADD64rr"]);
    }

    /// `base + index*4 + 8` at 64 bits: with only the 32-bit address arms it
    /// matched no scale and came out as adds, not one `LEA64r` (D64-1).
    #[test]
    fn base_plus_scaled_index_plus_disp_is_lea64r() {
        let mut dag = TestDag::new();
        let (base, index) = (dag.input(1, mvt::i64), dag.input(2, mvt::i64));
        let two = dag.make_constant(2, mvt::i8);
        let scaled = SDValue::new(dag.node(isd::SHL, &[mvt::i64], &[index, two]), 0);
        let sum = SDValue::new(dag.node(isd::ADD, &[mvt::i64], &[base, scaled]), 0);
        let eight = dag.make_constant(8, mvt::i64);
        let address = dag.node(isd::ADD, &[mvt::i64], &[sum, eight]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::X64,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat64,
        });
        assert_eq!(select_node(&Selector::new(x64::TABLE), &mut dag, &mut hooks, address), Outcome::Selected);
        let names: Vec<_> = dag.nodes.iter().filter_map(|n| n.machine).map(names).collect();
        assert_eq!(names, ["LEA64r"]);
    }

    /// In `Flat64` the shift was left as the index register (scale 1) because
    /// the scale arms only ran for `Flat32` (D64-1).
    #[test]
    fn a_shifted_index_is_scaled_at_64_bits() {
        use crate::x86_addr::{AddrMode, Matcher};
        let mut dag = TestDag::new();
        let (base, index) = (dag.input(1, mvt::i64), dag.input(2, mvt::i64));
        let two = dag.make_constant(2, mvt::i8);
        let scaled = SDValue::new(dag.node(isd::SHL, &[mvt::i64], &[index, two]), 0);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i64], &[base, scaled]), 0);
        let ops = Matcher { dag: &mut dag, pointer_type: mvt::i64, mode: AddrMode::Flat64, size: false }
            .select_addr(address)
            .unwrap();
        assert_eq!(dag.nodes[ops.scale.node as usize].constant, Some(4));
    }

    fn rip_matcher(dag: &mut TestDag) -> crate::x86_addr::Matcher<'_, TestDag> {
        crate::x86_addr::Matcher { dag, pointer_type: mvt::i64, mode: crate::x86_addr::AddrMode::Flat64, size: false }
    }

    /// A `WrapperRIP` global had no arm: it stayed a register operand, not
    /// `[rip + sym + 12]` (D64-1).
    #[test]
    fn a_rip_wrapper_plus_a_constant_is_rip_based() {
        let mut dag = TestDag::new();
        let global = dag.wrapped_global_rip(7, 0, mvt::i64);
        let twelve = dag.make_constant(12, mvt::i64);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i64], &[global, twelve]), 0);
        let mut am = crate::x86_addr::AddressMode::default();

        assert!(!rip_matcher(&mut dag).match_address(address, &mut am));

        assert_eq!(am.disp, 12);
        assert_eq!(dag.nodes[am.base_reg.unwrap().node as usize].register, Some(crate::generated::ids::x86::RIP));
        assert!(am.index_reg.is_none());
    }

    /// `[rip + sym]` takes no register: adding `x` must not fold into it.
    #[test]
    fn a_register_is_not_folded_into_a_rip_address() {
        let mut dag = TestDag::new();
        let global = dag.wrapped_global_rip(7, 0, mvt::i64);
        let x = dag.input(1, mvt::i64);
        let address = SDValue::new(dag.node(isd::ADD, &[mvt::i64], &[global, x]), 0);
        let mut am = crate::x86_addr::AddressMode::default();

        assert!(!rip_matcher(&mut dag).match_address(address, &mut am));

        assert!(am.symbol.is_none(), "the symbol stays in its register, not in a rip address");
        let base = am.base_reg.unwrap();
        assert_ne!(dag.nodes[base.node as usize].register, Some(crate::generated::ids::x86::RIP));
    }

    /// An `i32` address sum at 64 bits is `LEA64_32r`; there is no `LEA32r` in
    /// 64-bit mode (D64-1).
    #[test]
    fn an_i32_address_sum_is_lea64_32r() {
        let mut dag = TestDag::new();
        let (base, index) = (dag.input(1, mvt::i32), dag.input(2, mvt::i32));
        let two = dag.make_constant(2, mvt::i8);
        let scaled = SDValue::new(dag.node(isd::SHL, &[mvt::i32], &[index, two]), 0);
        let sum = SDValue::new(dag.node(isd::ADD, &[mvt::i32], &[base, scaled]), 0);
        let eight = dag.make_constant(8, mvt::i32);
        let address = dag.node(isd::ADD, &[mvt::i32], &[sum, eight]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::X64,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat64,
        });
        assert_eq!(select_node(&Selector::new(x64::TABLE), &mut dag, &mut hooks, address), Outcome::Selected);
        let names: Vec<_> = dag.nodes.iter().filter_map(|n| n.machine).map(names).collect();
        assert_eq!(
            names,
            ["LEA64_32r", "IMPLICIT_DEF", "INSERT_SUBREG", "IMPLICIT_DEF", "INSERT_SUBREG"],
            "base and index widened"
        );
    }

    /// With SSE2 the x64 table selects scalar `double` arithmetic in `xmm`:
    /// `fadd` was the x87 `ADD_Fp64` (D64-4).
    #[test]
    fn a_double_add_is_addsd() {
        let mut dag = TestDag::new();
        let (a, b) = (dag.input(1, mvt::f64), dag.input(2, mvt::f64));
        let add = dag.node(isd::FADD, &[mvt::f64], &[a, b]);
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::X64,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat64,
        });
        assert_eq!(select_node(&Selector::new(x64::TABLE), &mut dag, &mut hooks, add), Outcome::Selected);
        let names: Vec<_> = dag.nodes.iter().filter_map(|n| n.machine).map(names).collect();
        assert_eq!(names, ["ADDSDrr"]);
    }

    #[test]
    fn an_i32_add_is_still_add32rr() {
        assert_eq!(selected_names(mvt::i32), ["ADD32rr"]);
    }

    #[test]
    fn a_pointer_is_an_i64_in_64_bit_mode() {
        let options = Options {
            cpu: Cpu::X64,
            opt_none: false,
            opt_for_size: false,
            min_size: false,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        };
        assert_eq!(options.pointer_type(), mvt::i64);
    }
}

mod minsize_store_immediate {
    use super::*;
    use crate::x86_hooks::{Cpu, Options, X86Hooks};

    fn selected_store(
        value: i64,
        vt: crate::matcher::Vt,
        min_size: bool,
    ) -> &'static str {
        let mut dag = TestDag::new();
        let p = dag.input(1, mvt::i32);
        let constant = dag.make_constant(value, vt);
        let off = SDValue::new(dag.node(isd::UNDEF, &[mvt::i32], &[]), 0);
        let store = dag.add(Node {
            opcode: isd::STORE,
            vts: vec![mvt::Other],
            ops: vec![SDValue::new(0, 0), constant, p, off],
            memory_vt: Some(vt),
            ..Node::default()
        });
        let mut hooks = X86Hooks::new(Options {
            cpu: Cpu::I486,
            opt_none: false,
            opt_for_size: min_size,
            min_size,
            has_frame_pointer: true,
            address_mode: crate::x86_addr::AddrMode::Flat32,
        });
        assert_eq!(Selector::new(table::TABLE).select(&mut dag, &mut hooks, store), Outcome::Selected);
        selected(&dag, store)
    }

    /// X86InstrCompiler.td:1308: at OptForMinSize a store of 0 is `and [m], 0`
    /// and of -1 `or [m], -1`, a byte immediate where `mov` has four.
    #[test]
    fn a_store_of_zero_or_minus_one_is_and_or_or_at_minsize_and_mov_otherwise() {
        assert_eq!(selected_store(0, mvt::i32, false), "MOV32mi");
        assert_eq!(selected_store(0, mvt::i32, true), "AND32mi");
        assert_eq!(selected_store(-1, mvt::i32, true), "OR32mi");
        assert_eq!(selected_store(0, mvt::i16, true), "AND16mi");
        assert_eq!(selected_store(-1, mvt::i16, true), "OR16mi");
        assert_eq!(selected_store(5, mvt::i32, true), "MOV32mi");
    }
}
