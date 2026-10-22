//! The interpreter against the generated i486 table, on hand-built DAGs.

use crate::dag::SDValue;
use crate::generated::ids::{isd, mvt};
use crate::generated::x86_i486 as table;
use crate::interp::{Outcome, Selector};
use crate::testing::{Node, TestDag, TestHooks};

fn machine_name(opcode: u16) -> &'static str {
    table::MACHINE_OPCODES.iter().find(|(v, _)| *v == opcode).map_or("?", |(_, n)| n)
}

/// The machine opcode of the node `value` now is.
fn selected(dag: &TestDag, node: u32) -> &'static str {
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

fn emits(opcodes: &[(u16, &str)], prefix: &str) -> bool {
    opcodes.iter().any(|(_, name)| name.split('|').any(|n| n.starts_with(prefix)))
}

#[test]
fn neither_cpu_emits_cmov_and_only_p5_emits_cmpxchg8b() {
    use crate::generated::x86_p5;
    // `CMOV_GR32` and kin are the pseudos that become a branch; `CMOV32rr` is the instruction.
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
        X86Hooks::new(Options { cpu: Cpu::I486, opt_none: false, opt_for_size: false, min_size: false, has_frame_pointer: true, address_mode: crate::x86_addr::AddrMode::Flat32 })
    }

    fn constant_of(dag: &TestDag, v: SDValue) -> Option<i64> {
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
        Matcher { dag, pointer_type: mvt::i32, mode: crate::x86_addr::AddrMode::Flat32 }
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
        X86Hooks::new(Options { cpu: Cpu::I486, opt_none: false, opt_for_size: false, min_size: false, has_frame_pointer: true, address_mode: crate::x86_addr::AddrMode::Flat32 })
    }

    /// `*p += rhs` as the DAG makes it: load, X86ISD::ADD, store. Returns the store, the load and
    /// the arithmetic node.
    fn add_to_memory(dag: &mut TestDag, rhs: SDValue, p: SDValue) -> (u32, u32, u32) {
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
        X86Hooks::new(Options { cpu: Cpu::I486, opt_none, opt_for_size: false, min_size: false, has_frame_pointer: true, address_mode: crate::x86_addr::AddrMode::Flat32 })
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
        Matcher { dag, pointer_type: mvt::i32, mode: crate::x86_addr::AddrMode::Flat32 }
    }

    fn masked_shift(dag: &mut TestDag, amount: i64, mask: i64) -> (SDValue, SDValue) {
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

    fn constant_of_node(dag: &TestDag, v: SDValue) -> Option<i64> {
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
        // `(x >> 1) & 0x3fc` with nothing known about x: the high bits of x may be set.
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
    fn select(cpu: Cpu, vt: crate::matcher::Vt) -> (TestDag, u32) {
        let mut dag = TestDag::new();
        let a = dag.input(1, vt);
        let b = dag.input(2, vt);
        let (t, f) = (dag.input(3, vt), dag.input(4, vt));
        let flags = SDValue::new(dag.node(x86isd::CMP, &[mvt::i32], &[a, b]), 0);
        let cc = dag.target_constant(4, mvt::i8);
        let cmov = dag.node(x86isd::CMOV, &[vt, mvt::i32], &[f, t, cc, flags]);
        let mut hooks = X86Hooks::new(Options { cpu, opt_none: false, opt_for_size: false, min_size: false, has_frame_pointer: true, address_mode: crate::x86_addr::AddrMode::Flat32 });
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

    fn matcher(dag: &mut TestDag, mode: AddrMode) -> Matcher<'_, TestDag> {
        Matcher { dag, pointer_type: mvt::i16, mode }
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
        table::MACHINE_OPCODES.iter().find(|(_, n)| n.split('|').any(|n| n == name)).unwrap_or_else(|| panic!("{name} is not emitted")).0
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

    fn hooks(cpu: Cpu, mode: AddrMode) -> X86Hooks {
        X86Hooks::new(Options { cpu, opt_none: false, opt_for_size: false, min_size: false, has_frame_pointer: true, address_mode: mode })
    }

    /// `X86ISD::LES chain, p`: the offset half of a far pointer loaded through `p`.
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

    fn name_in(table: &[(u16, &'static str)], dag: &TestDag, node: u32) -> &'static str {
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
    use crate::x86_classes::{class_tag, memory_reference, role_of, Role};

    fn opcode(name: &str) -> u16 {
        table::MACHINE_OPCODES.iter().find(|(_, n)| n.split('|').any(|n| n == name)).unwrap_or_else(|| panic!("{name}")).0
    }

    #[test]
    fn a_load_has_its_reference_after_the_destination() {
        let d = desc(opcode("MOV32rm")).unwrap();

        assert_eq!(memory_reference(d), Some(1));
        assert_eq!(role_of(d, 0), None, "the destination is not part of it");
        assert_eq!((role_of(d, 1), role_of(d, 3), role_of(d, 5)), (Some(Role::Base), Some(Role::Index), Some(Role::Segment)));
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
        X86Hooks::new(Options { cpu, opt_none: false, opt_for_size: false, min_size: false, has_frame_pointer: true, address_mode })
    }

    /// `rep movs` of `unit`-typed elements, after the count register is set.
    fn rep(opcode: u16, unit: crate::matcher::Vt) -> (TestDag, u32) {
        let mut dag = TestDag::new();
        let count = dag.input(1, mvt::i16);
        let copy = dag.copy_to_reg(SDValue::new(0, 0), x86::CX, count, None);
        let vt = dag.make_value_type_node(unit);
        let node = dag.node(opcode, &[mvt::Other, mvt::Glue], &[SDValue::new(copy, 0), vt, SDValue::new(copy, 1)]);
        (dag, node)
    }

    fn name(table: &[(u16, &'static str)], dag: &TestDag, node: u32) -> &'static str {
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

    fn select(dag: &mut TestDag, load: u32) {
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
    fn shl(dag: &mut TestDag, amount: impl FnOnce(&mut TestDag, SDValue) -> SDValue) -> (u32, SDValue) {
        let x = dag.input(1, mvt::i32);
        let n = dag.input(2, mvt::i8);
        let count = amount(dag, n);
        (dag.node(isd::SHL, &[mvt::i32], &[x, count]), n)
    }

    /// The count of the shift after the rewrite: `(n' & 31)`; returns `n'`.
    fn masked_source(dag: &TestDag, shift: u32) -> SDValue {
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
        X86Hooks::new(Options { cpu: Cpu::I486, opt_none: false, opt_for_size: false, min_size: false, has_frame_pointer: true, address_mode: AddrMode::Flat32 })
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
    // `X86::CS` (a register) and `SUBREG_TO_REG` (an instruction) are both 17; the name column once read
    // `CS|SUBREG_TO_REG|VK2RegClassID`.
    for (_, name) in table::MACHINE_OPCODES {
        assert!(!name.contains("RegClassID") && name.split('|').all(|n| !["CS", "DS", "ES", "FS", "GS", "SS"].contains(&n)), "{name}");
    }
}
