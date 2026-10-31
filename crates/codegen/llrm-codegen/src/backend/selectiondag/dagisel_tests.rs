//! The matcher interpreter over our own DAG: the C1 address and fold cases.

use llrm_dagisel::generated::x86_i486 as table;
use llrm_dagisel::interp::{Outcome, Selector};
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};
use llrm_dagisel::x86_select::select_node;

use super::dag::{MemBase, MemInfo};
use super::dagisel_adapter::DagisAdapter;
use super::isd::ISD;
use super::mvt::EVT;
use super::{DagConfig, SDValue, SelectionDAG};

fn hooks() -> X86Hooks {
    X86Hooks::new(Options {
        cpu: Cpu::I486,
        opt_none: false,
        opt_for_size: false,
        min_size: false,
        has_frame_pointer: true,
        multiply_early_out: false,
        address_mode: llrm_dagisel::x86_addr::AddrMode::Flat32,
    })
}

fn mem_info() -> MemInfo {
    MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Unknown, ext: None, space: 0, ordering: None }
}

fn input(
    dag: &mut SelectionDAG,
    reg: u32,
) -> SDValue {
    let entry = dag.entry_node();
    dag.get_copy_from_reg(entry, reg, EVT::I32)
}

/// The pin's name of the machine instruction a node became.
fn machine_name(
    dag: &SelectionDAG,
    node: u32,
) -> &'static str {
    let ISD::Machine(code) = dag.nodes()[node as usize].opcode else { panic!("node {node} is not a machine node") };
    table::MACHINE_OPCODES.iter().find(|(v, _)| u32::from(*v) == code).map_or("?", |(_, n)| n)
}

/// `a + ((i + 2) << 2)`: the address of `a[i + 2]`.
fn element_address(dag: &mut SelectionDAG) -> (SDValue, SDValue, SDValue) {
    let a = input(dag, 40);
    let i = input(dag, 41);
    let two = dag.get_constant(2, EVT::I32);
    let shift = dag.get_constant(2, EVT::I8);
    let i2 = dag.get_node(ISD::Add, EVT::I32, &[i, two]);
    let scaled = dag.get_node(ISD::Shl, EVT::I32, &[i2, shift]);
    let address = dag.get_node(ISD::Add, EVT::I32, &[a, scaled]);
    (address, a, i)
}

#[test]
fn a_load_of_a_index_plus_two_is_one_mov32rm_with_the_folded_operands() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let (address, a, i) = element_address(&mut dag);
    let entry = dag.entry_node();
    let load = dag.get_load(EVT::I32, entry, address, mem_info());
    let mut adapter = DagisAdapter::new(&mut dag);

    let outcome = Selector::new(table::TABLE).select(&mut adapter, &mut hooks(), load.node);

    assert_eq!(outcome, Outcome::Selected);
    assert_eq!(machine_name(&dag, load.node), "MOV32rm");
    let ops = &dag.nodes()[load.node as usize].ops;
    assert_eq!((ops[0], ops[2]), (a, i), "base and index");
    assert_eq!(dag.constant(ops[1]), Some(4), "scale");
    assert_eq!(dag.constant(ops[3]), Some(8), "the +2, scaled by 4");
}

#[test]
fn add_of_two_registers_is_add32rr() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let a = input(&mut dag, 40);
    let b = input(&mut dag, 41);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[a, b]);
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(Selector::new(table::TABLE).select(&mut adapter, &mut hooks(), sum.node), Outcome::Selected);

    assert_eq!(machine_name(&dag, sum.node), "ADD32rr");
}

#[test]
fn a_node_the_table_has_no_pattern_for_cannot_be_selected() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let a = input(&mut dag, 40);
    let weird = dag.get_node(ISD::Target(9999), EVT::I32, &[a]);
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(Selector::new(table::TABLE).select(&mut adapter, &mut hooks(), weird.node), Outcome::CannotSelect);
}

#[test]
fn the_select_loop_drops_a_shift_counts_plus_thirty_two_in_our_dag() {
    use llrm_dagisel::x86_select::select_node;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let x = input(&mut dag, 40);
    let entry = dag.entry_node();
    let n = dag.get_copy_from_reg(entry, 41, EVT::I8);
    let thirty_two = dag.get_constant(32, EVT::I8);
    let count = dag.get_node(ISD::Add, EVT::I8, &[n, thirty_two]);
    let shift = dag.get_node(ISD::Shl, EVT::I32, &[x, count]);
    let mut adapter = DagisAdapter::new(&mut dag);

    let outcome = select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), shift.node);

    assert_eq!(outcome, Outcome::Selected);
    let copied = dag
        .nodes()
        .iter()
        .find_map(
            |node| (node.opcode == ISD::CopyToReg
                && matches!(
                    dag.nodes()[node.ops[1].node as usize].data,
                    super::NodeData::PhysReg(llrm_lir::registers::RegId::CL)
                ))
            .then(|| node.ops[2]),
        );
    assert_eq!(copied, Some(n), "CL gets n, not n + 32");
}

#[test]
fn jump_table_external_symbol_and_register_mask_nodes_have_the_pins_numbers() {
    use llrm_dagisel::generated::ids::isd as pin;

    use super::dagisel_adapter::pin_opcode;
    assert_eq!(pin_opcode(ISD::RegisterMask), pin::RegisterMask);
    assert_eq!(pin_opcode(ISD::JumpTable), pin::JumpTable);
    assert_eq!(pin_opcode(ISD::TargetExternalSymbolNode), pin::TargetExternalSymbol);
}

#[test]
fn a_return_node_is_selected_to_ret() {
    use super::x86isd;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let zero = dag.get_constant_with(0, EVT::I32, true, false);
    let ret = dag.get_node(x86isd::RET_GLUE, EVT::OTHER, &[entry, zero]);
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), ret.node), Outcome::Selected);

    assert_eq!(machine_name(&dag, ret.node), "RET");
}

#[test]
fn a_target_code_is_named_or_already_the_pins_number() {
    use llrm_dagisel::generated::ids::x86isd as pin;

    use super::dagisel_adapter::pin_opcode;
    assert_eq!(pin_opcode(super::x86isd::CALL), pin::CALL, "sdag-a's private code, by name");
    assert_eq!(pin_opcode(ISD::Target(pin::Wrapper)), pin::Wrapper, "sdag-b's lowering writes the pin's number");
}

/// The adapter numbered the pin's registers like the builder's virtual ones, so
/// EFLAGS (28) read as the builder's 28th virtual register in the emitter.
#[test]
fn the_pins_physical_registers_are_physical_register_nodes() {
    use llrm_dagisel::dag::Dag;
    use llrm_dagisel::generated::ids::x86;
    use llrm_lir::registers::RegId;

    use super::dag::NodeData;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let mut adapter = DagisAdapter::new(&mut dag);
    let eax = adapter.register(x86::EAX, llrm_dagisel::generated::ids::mvt::i32);
    let noreg = adapter.register(0, llrm_dagisel::generated::ids::mvt::i32);
    let flags = adapter.register(x86::EFLAGS, llrm_dagisel::generated::ids::mvt::i32);

    let data = |node: llrm_dagisel::dag::SDValue| dag.nodes()[node.node as usize].data;
    assert!(matches!(data(eax), NodeData::PhysReg(r) if r == RegId::EAX));
    assert!(matches!(data(noreg), NodeData::Register(0)));
    assert!(matches!(data(flags), NodeData::PhysReg(r) if r == super::dagisel_adapter::EFLAGS_ID));
}

/// A target whose map lacks a register the selector names read it as a
/// virtual register of the pin's number (AArch64's NZCV, 7, became vreg 7).
/// The adapter now records a refusal; x86's map still reads a stranger as
/// virtual.
#[test]
fn a_register_the_target_cannot_map_is_a_refusal_not_a_virtual_register() {
    use llrm_dagisel::dag::Dag;
    use llrm_dagisel::generated::ids::mvt;
    use llrm_lir::registers::RegId;

    use super::dagisel_adapter::PinRegisters;

    struct OneRegister;
    impl PinRegisters for OneRegister {
        fn phys_reg(
            &self,
            pin: u16,
        ) -> Option<RegId> {
            (pin == 1).then_some(RegId::from_raw(1))
        }
        fn pin_register(
            &self,
            id: RegId,
        ) -> Option<u16> {
            (id == RegId::from_raw(1)).then_some(1)
        }
        fn unmapped(
            &self,
            pin: u16,
        ) -> bool {
            pin > 1
        }
    }

    let mut dag = SelectionDAG::new(DagConfig::default());
    let mut x86 = DagisAdapter::new(&mut dag);
    x86.register(199, mvt::i32);
    assert_eq!(x86.refusal(), None, "x86 keeps reading an unnamed register as virtual");

    let mut dag = SelectionDAG::new(DagConfig::default());
    let mut adapter = DagisAdapter::with_registers(&mut dag, &OneRegister);
    adapter.register(0, mvt::i32);
    adapter.register(1, mvt::i32);
    assert_eq!(adapter.refusal(), None);
    adapter.register(7, mvt::i32);
    assert!(adapter.refusal().is_some_and(|why| why.contains("pin register 7")));
}

/// The adapter said no store truncates, so a narrow store of a wide value
/// matched the plain store patterns (and `ST_Fp64m32`'s truncating pattern
/// never could).
#[test]
fn a_store_narrower_than_its_value_is_truncating() {
    use llrm_dagisel::dag::Dag;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let (value, ptr) = (input(&mut dag, 1), input(&mut dag, 2));
    let wide = dag.get_store(entry, value, ptr, mem_info());
    let narrow = dag.get_store(entry, value, ptr, MemInfo { mem_vt: EVT::I8, ..mem_info() });
    let adapter = DagisAdapter::new(&mut dag);

    assert!(!adapter.is_truncating_store(wide.node));
    assert!(adapter.is_truncating_store(narrow.node));
}

/// The adapter said no memory node had a segment override, so a load through
/// `fs:` selected as a plain `mov` (the segment operand left at `$noreg`).
#[test]
fn a_load_in_the_fs_space_addresses_through_fs() {
    use llrm_lir::registers::RegId;

    use super::dag::NodeData;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let ptr = input(&mut dag, 1);
    let load = dag.get_load(EVT::I32, entry, ptr, MemInfo { space: 257, ..mem_info() });
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), load.node), Outcome::Selected);

    let segment = dag.nodes()[load.node as usize].ops[4];
    assert!(matches!(dag.node(segment).data, NodeData::PhysReg(r) if r == RegId::FS));
}

/// The segment `les` leaves in ES is address space 259 (llrm's own, after the
/// pin's 256 to 258).
#[test]
fn a_load_in_space_259_addresses_through_es() {
    use llrm_lir::registers::RegId;

    use super::dag::NodeData;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let ptr = input(&mut dag, 1);
    let load = dag.get_load(EVT::I32, entry, ptr, MemInfo { space: 259, ..mem_info() });
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), load.node), Outcome::Selected);

    let segment = dag.nodes()[load.node as usize].ops[4];
    assert!(matches!(dag.node(segment).data, NodeData::PhysReg(r) if r == RegId::ES));
}

/// Every `f80` constant was class `Other`, so `fldz` and `fld1` never matched a
/// long double 0.0 or 1.0 and the `ConstantFP` stayed unselected
/// (long_double_variadic).
#[test]
fn an_f80_zero_and_one_are_selected_as_the_x87_constants() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let zero = dag.get_constant_fp(0, EVT::Simple(super::mvt::MVT::F80));
    let one = dag.get_constant_fp((0x3fff << 64) | (1 << 63), EVT::Simple(super::mvt::MVT::F80));
    let mut adapter = DagisAdapter::new(&mut dag);

    for (value, name) in [(zero, "LD_Fp080"), (one, "LD_Fp180")] {
        assert_eq!(
            select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), value.node),
            Outcome::Selected,
            "{name}"
        );
    }
    assert_eq!(machine_name(&dag, zero.node), "LD_Fp080");
    assert_eq!(machine_name(&dag, one.node), "LD_Fp180");
}

/// A pattern that takes a float as an immediate (`fpimm`, wasm's `f64.const`)
/// asks for the constant as a target constant; a `ConstantFP` stayed a
/// `ConstantFP`, which no machine instruction's pattern accepts.
#[test]
fn a_float_constant_converts_to_a_target_constant() {
    use llrm_dagisel::dag::{Dag, SDValue as PinValue};
    let mut dag = SelectionDAG::new(DagConfig::default());
    let value = dag.get_constant_fp(2.5f64.to_bits().into(), EVT::Simple(super::mvt::MVT::F64));
    let mut adapter = DagisAdapter::new(&mut dag);
    let target = adapter.convert_to_target(PinValue::new(value.node, value.res));
    let target = SDValue { node: target.node, res: target.res };
    assert_eq!(dag.opcode(target), ISD::TargetConstantFP);
    assert_eq!(dag.value_type(target), EVT::Simple(super::mvt::MVT::F64));
}

/// Every node of ours that the matcher passes through has an id in
/// `own_ids.txt`: `LLRM_FARCALL` had none, mapped to no opcode, and a call
/// through a function pointer was `CannotSelect(Target(24584))` (7 m16
/// programs: function_table, static_inline, static_pointers).
#[test]
fn every_own_node_has_a_pin_id() {
    use super::dagisel_adapter::pin_opcode;
    use super::x86isd::{
        LLRM_ASM, LLRM_CODE, LLRM_FARCALL, LLRM_PORT, LLRM_PTRDIFF, LLRM_TC_RETURN, LLRM_WINDOW, SEGMENT_OF,
    };
    for node in [LLRM_ASM, LLRM_CODE, LLRM_FARCALL, LLRM_PORT, LLRM_TC_RETURN, LLRM_PTRDIFF, LLRM_WINDOW, SEGMENT_OF] {
        assert_ne!(pin_opcode(node), u16::MAX, "{node:?}");
    }
}

/// Every `Target(24xxx)` of `x86isd.rs` (ours, not the pin's) is a line of
/// `own_ids.txt`: `LLRM_CALL_FLAGS` (24585) was added to the node list and not
/// to the file, so the generated ids did not know it.
#[test]
fn every_own_target_id_is_in_own_ids() {
    let nodes = include_str!("../../../../../../crates/codegen/llrm-sdag/src/x86isd.rs");
    let ids = include_str!("../../../../../../tools/dagisel/own_ids.txt");
    let listed: Vec<(&str, &str)> = ids.lines().filter_map(|line| line.split_once(' ')).collect();
    let mut missing = Vec::new();
    for line in nodes.lines().filter(|line| line.starts_with("pub const ")) {
        let Some((name, rest)) = line["pub const ".len()..].split_once(':') else { continue };
        let Some(number) = rest.split("Target(").nth(1).and_then(|tail| tail.split(')').next()) else { continue };
        if number.starts_with("24") && !listed.iter().any(|(id, n)| id.ends_with(&format!("::{name}")) && *n == number)
        {
            missing.push(format!("{name} {number}"));
        }
    }
    assert!(missing.is_empty(), "not in tools/dagisel/own_ids.txt: {missing:?}");
}

/// `log`, `exp`, `log2`, `exp2`, `sin`, `cos` and `atan` of a double had no
/// pattern (the pin calls libm): `CannotSelect` in 36 qb/nib programs under
/// `--isel=dag`. llrm's targets have no libm, so they select as the x87
/// sequences the old selector spells.
#[test]
fn a_natural_log_exp_and_log2_are_the_x87_sequences() {
    for (op, name) in [
        (ISD::FLog, "LN_Fp80"),
        (ISD::FExp, "EXP_Fp80"),
        (ISD::FLog2, "LOG2_Fp80"),
        (ISD::FExp2, "EXP2_Fp80"),
        (ISD::FSin, "SIN_Fp80"),
        (ISD::FCos, "COS_Fp80"),
        (ISD::FAtan, "ATAN_Fp80"),
        (ISD::FRint, "RNDINT_Fp80"),
    ] {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let x = {
            let entry = dag.entry_node();
            dag.get_copy_from_reg(entry, 40, EVT::Simple(super::mvt::MVT::F64))
        };
        let node = dag.get_node(op, EVT::Simple(super::mvt::MVT::F64), &[x]);
        let mut adapter = DagisAdapter::new(&mut dag);

        assert_eq!(
            select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), node.node),
            Outcome::Selected,
            "{op:?}"
        );

        let ISD::Machine(code) = dag.nodes()[node.node as usize].opcode else { panic!("{op:?} is no machine node") };
        let own = [
            (llrm_dagisel::x86_llrm::FLN_OPCODE, "LN_Fp80"),
            (llrm_dagisel::x86_llrm::FEXP_OPCODE, "EXP_Fp80"),
            (llrm_dagisel::x86_llrm::FLOG2_OPCODE, "LOG2_Fp80"),
            (llrm_dagisel::x86_llrm::FEXP2_OPCODE, "EXP2_Fp80"),
            (llrm_dagisel::x86_llrm::FSIN_OPCODE, "SIN_Fp80"),
            (llrm_dagisel::x86_llrm::FCOS_OPCODE, "COS_Fp80"),
            (llrm_dagisel::x86_llrm::FATAN_OPCODE, "ATAN_Fp80"),
            (llrm_dagisel::x86_llrm::FRINT_OPCODE, "RNDINT_Fp80"),
        ];
        assert_eq!(own.iter().find(|(v, _)| u32::from(*v) == code).map(|(_, n)| *n), Some(name), "{op:?}");
    }
}

/// `(x & 1) | (y << 1)`: no shared bit, so the `or` is an add and the address
/// `x + y*2` is one `lea` (bench `x_popcount`: the pin has `LEA32r` x2 where we
/// had `SHL32ri` + `ADD32rr_DB`, and before known bits, `OR32rr`).
#[test]
fn an_or_of_values_with_no_common_bits_is_one_lea() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let (x, y) = (input(&mut dag, 40), input(&mut dag, 41));
    let one = dag.get_constant(1, EVT::I32);
    let by = dag.get_constant(1, EVT::I8);
    let low = dag.get_node(ISD::And, EVT::I32, &[x, one]);
    let high = dag.get_node(ISD::Shl, EVT::I32, &[y, by]);
    let or = dag.get_node(ISD::Or, EVT::I32, &[low, high]);
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(Selector::new(table::TABLE).select(&mut adapter, &mut hooks(), or.node), Outcome::Selected);

    assert_eq!(machine_name(&dag, or.node), "LEA32r");
}

/// `lrint` and `rint` of an x87 value store with `fist` in the current rounding
/// mode (`X86ISD::FIST`): `CannotSelect(Target(587))` in `jpeg_single.bas`
/// under `--isel=dag`. It is `IST_Fp<float>m<int>` into the slot.
#[test]
fn an_x87_fist_to_a_stack_slot_is_an_ist_pseudo() {
    use super::dag::{MemBase, SDNodeFlags};
    for (float, int, name) in [
        (super::mvt::MVT::F64, EVT::I32, "IST_Fp32m64"),
        (super::mvt::MVT::F32, EVT::I32, "IST_Fp32m32"),
        (super::mvt::MVT::F64, EVT::I16, "IST16_64"),
    ] {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let value = {
            let entry = dag.entry_node();
            dag.get_copy_from_reg(entry, 40, EVT::Simple(float))
        };
        let slot = dag.get_frame_index(EVT::I32, 3);
        let chain = dag.entry_node();
        let info = MemInfo {
            mem_vt: int,
            align: 4,
            volatile: false,
            base: MemBase::Stack(3),
            ext: None,
            space: 0,
            ordering: None,
        };
        let fist = dag.intern(
            super::x86_lowering::x86isd::FIST,
            &[EVT::OTHER],
            &[chain, value, slot],
            SDNodeFlags::default(),
            super::dag::NodeData::Mem(info),
        );
        let mut adapter = DagisAdapter::new(&mut dag);

        let outcome = select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), fist.node);
        assert_eq!(outcome, Outcome::Selected, "{float:?} to {int:?}");
        match name.strip_prefix("IST16_") {
            Some(_) => {
                let ISD::Machine(code) = dag.nodes()[fist.node as usize].opcode else { panic!("no machine node") };
                assert_eq!(code, u32::from(llrm_dagisel::x86_llrm::IST16_OPCODES[1]), "a word fist of a double");
            }
            None => assert_eq!(machine_name(&dag, fist.node), name),
        }
    }
}

/// A float constant read from the pool: `fld [const.0]`, the pool entry in the
/// instruction's displacement. The address was a `MOV32ri` of the entry
/// into a register first (bench `x_horner`, `floats`, `recfloat`,
/// `nbody_single`, `x_nbody_f`: 9 `MOV32ri` the pin does not have).
#[test]
fn a_float_load_from_the_constant_pool_folds_the_entry_into_the_address() {
    use super::dag::{NodeData, SDNodeFlags};
    let mut dag = SelectionDAG::new(DagConfig::default());
    let f64_vt = EVT::Simple(super::mvt::MVT::F64);
    dag.constant_pool.push((0x3FF8_0000_0000_0000, f64_vt));
    let entry =
        dag.intern(ISD::TargetConstantPool, &[EVT::I32], &[], SDNodeFlags::default(), NodeData::PoolIndex(0, 0));
    let address = dag.get_node(super::x86_lowering::x86isd::WRAPPER, EVT::I32, &[entry]);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: f64_vt,
        align: 8,
        volatile: false,
        base: MemBase::ConstantPool(0),
        ext: None,
        space: 0,
        ordering: None,
    };
    let load = dag.get_load(f64_vt, chain, address, info);
    let mut adapter = DagisAdapter::new(&mut dag);

    assert_eq!(select_node(&Selector::new(table::TABLE), &mut adapter, &mut hooks(), load.node), Outcome::Selected);

    assert_eq!(machine_name(&dag, load.node), "LD_Fp64m");
    let ops = &dag.nodes()[load.node as usize].ops;
    assert!(ops.iter().any(|op| dag.opcode(*op) == ISD::TargetConstantPool), "the entry is in the address group");
    assert!(
        !ops.iter().any(|op| matches!(dag.nodes()[op.node as usize].opcode, ISD::Machine(_))),
        "no register-materialized address"
    );
}

/// The use queries read the use list, not the arena: the counts and the
/// "only user" answer are the same as a scan of every live node gives.
#[test]
fn use_queries_agree_with_a_scan_of_the_live_nodes() {
    use llrm_dagisel::dag::Dag;

    let mut dag = SelectionDAG::new(DagConfig::default());
    let a = input(&mut dag, 40);
    let twice = dag.get_node(ISD::Add, EVT::I32, &[a, a]);
    dag.set_root(twice);
    let adapter = DagisAdapter::new(&mut dag);
    let pin_a = llrm_dagisel::dag::SDValue::new(a.node, 0);
    assert!(adapter.has_live_users(a.node));
    assert!(adapter.has_n_uses_of_value(pin_a, 2));
    assert!(!adapter.has_one_use(pin_a));
    assert!(adapter.is_only_user_of(twice.node, a.node));
    assert!(adapter.use_empty(twice.node));
    assert!(!adapter.use_empty(a.node));
}

/// `pin_opcode` built a `String` (`x86isd::name` and `format!`) for each target
/// node, 16 K times on d_faces, and the use queries collected and sorted a
/// `Vec` of the users on each call (4 more did, at 512 statements 43 K calls
/// on a node many read). A query answers from the use list without
/// an allocation.
#[test]
fn pin_opcode_and_the_use_queries_allocate_nothing() {
    use llrm_dagisel::dag::Dag;
    use llrm_dagisel::x86_addr::AddrDag;

    let (_, made) = llrm_support::alloc_count::counted(|| {
        (0..1024u16)
            .map(|code| super::dagisel_adapter::pin_opcode(ISD::Target(code)))
            .fold(0u32, |sum, op| sum + u32::from(op))
    });
    assert_eq!(made, 0, "{made} allocations to number 1,024 target opcodes");

    let mut dag = SelectionDAG::new(DagConfig::default());
    let a = input(&mut dag, 40);
    let twice = dag.get_node(ISD::Add, EVT::I32, &[a, a]);
    dag.set_root(twice);
    let adapter = DagisAdapter::new(&mut dag);
    let pin_a = llrm_dagisel::dag::SDValue::new(a.node, 0);
    let (answers, made) = llrm_support::alloc_count::counted(|| {
        (
            adapter.has_live_users(a.node),
            adapter.has_n_uses_of_value(pin_a, 2),
            adapter.has_one_use(pin_a),
            adapter.is_only_user_of(twice.node, a.node),
            adapter.use_empty(twice.node),
            adapter.node_has_one_use(a.node),
            adapter.count_non_chain_uses(a.node, 3),
            adapter.glued_user(a.node),
            adapter.has_materializing_use(pin_a),
        )
    });
    assert_eq!(answers, (true, true, false, true, true, false, 2, None, false), "premise: the answers are the scan's");
    assert_eq!(made, 0, "{made} allocations to ask nine use questions");
}

/// `hasPredecessorHelper` tested membership by scanning a `Vec`, so a walk
/// over n nodes took n^2 steps: 3.5 G instructions of dag isel at 512
/// statements, 21x the work of 128.
#[test]
fn walking_a_long_chain_for_a_predecessor_is_linear() {
    use llrm_dagisel::dag::{Dag, Visited};
    let mut dag = SelectionDAG::new(DagConfig::default());
    let base = input(&mut dag, 40);
    let mut chain = base;
    for i in 0..200_000u128 {
        let k = dag.get_constant(i, EVT::I32);
        chain = dag.get_node(ISD::Add, EVT::I32, &[chain, k]);
    }
    dag.set_root(chain);
    let adapter = DagisAdapter::new(&mut dag);
    let started = std::time::Instant::now();
    let found = adapter.has_predecessor_helper(u32::MAX, &mut Visited::new(), &mut vec![chain.node], 0);
    assert!(!found);
    assert!(started.elapsed().as_secs() < 2, "{:?} to walk 200,000 nodes", started.elapsed());
}

/// `hasPredecessorHelper` ignored `TopologicalPrune`, which every caller in the
/// pin asks for: a search for a node later in the numbering than the worklist's
/// walked everything before it. `IsLegalToFold`'s walk (a cap of 8192) then
/// gave up on the store-to-xor folds past ~3000 statements of `equal`, where
/// the pin keeps folding: 1146 more tied pairs and x2.8 per doubling in
/// twoaddr, coalesce and regalloc.
#[test]
fn a_search_for_a_later_node_does_not_walk_the_earlier_ones() {
    use llrm_dagisel::dag::{Dag, Visited};
    let mut dag = SelectionDAG::new(DagConfig::default());
    let base = input(&mut dag, 40);
    let mut chain = vec![base];
    for i in 0..1000u128 {
        let k = dag.get_constant(i, EVT::I32);
        let last = *chain.last().unwrap();
        chain.push(dag.get_node(ISD::Add, EVT::I32, &[last, k]));
    }
    dag.set_root(*chain.last().unwrap());
    let adapter = DagisAdapter::new(&mut dag);

    let mut visited = Visited::new();
    let found = adapter.has_predecessor_helper(chain[900].node, &mut visited, &mut vec![chain[500].node], 0);
    assert!(!found, "a later node is no predecessor of an earlier one");
    assert_eq!(visited.len(), 0, "the earlier node's operands were walked");

    let mut visited = Visited::new();
    assert!(adapter.has_predecessor_helper(chain[100].node, &mut visited, &mut vec![chain[900].node], 0));
}

/// A replacement makes the users of its result unprunable
/// (`EnforceNodeIdInvariant`): they may now read what the numbering puts after
/// them.
#[test]
fn a_user_of_a_replacement_is_walked_through_not_pruned() {
    use llrm_dagisel::dag::{Dag, SDValue as PinValue, Visited};
    let mut dag = SelectionDAG::new(DagConfig::default());
    let base = input(&mut dag, 40);
    let one = dag.get_constant(1, EVT::I32);
    let early = dag.get_node(ISD::Add, EVT::I32, &[base, one]);
    let mut late = early;
    for i in 2..20u128 {
        let k = dag.get_constant(i, EVT::I32);
        late = dag.get_node(ISD::Add, EVT::I32, &[late, k]);
    }
    dag.set_root(late);
    let mut adapter = DagisAdapter::new(&mut dag);
    // `early` now reads `late`: its user `early + 2` is older in the numbering
    // than `late`.
    let user =
        adapter.dag.nodes().iter().position(|n| n.ops.first().map(|o| o.node) == Some(early.node)).unwrap() as u32;
    adapter.replace_uses(PinValue { node: early.node, res: 0 }, PinValue { node: late.node, res: 0 });
    let mut visited = Visited::new();
    adapter.has_predecessor_helper(late.node, &mut visited, &mut vec![user], 0);
    assert!(visited.len() > 0, "the user of a replacement was pruned");
}

/// The combiner asked `use_count(n) == 1` of a value read 4096 times (the
/// shared `a*7+b` of `equal`), reading the whole use list for every combine
/// that looked at it: 1.9 G of dag isel at 4096, x3 per doubling. A one-use
/// check reads two users, as `hasOneUse` does.
#[test]
fn a_one_use_check_of_a_widely_read_value_reads_two_users() {
    use super::dag::USERS_READ;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let shared = input(&mut dag, 40);
    for i in 1..=1000u128 {
        let k = dag.get_constant(i, EVT::I32);
        dag.get_node(ISD::Add, EVT::I32, &[shared, k]);
    }
    USERS_READ.with(|n| n.set(0));
    assert!(!dag.has_one_use(shared.node));
    assert!(USERS_READ.with(std::cell::Cell::get) <= 2, "{} users read", USERS_READ.with(std::cell::Cell::get));
    assert_eq!(dag.use_count_up_to(shared.node, 3), 3);
    assert_eq!(dag.use_count(shared.node), 1000);
}

/// Every `live_users` query read the whole use list to see whether it was
/// sorted: x3.2 per doubling at 8192 on `equal`, where one value has a use
/// list of that length. A list pushed in order is walked as it is.
#[test]
fn a_use_list_pushed_in_order_is_not_checked_or_sorted_per_query() {
    use super::dag::USERS_SORTED;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let shared = input(&mut dag, 40);
    for i in 1..=2000u128 {
        let k = dag.get_constant(i, EVT::I32);
        dag.get_node(ISD::Add, EVT::I32, &[shared, k]);
    }
    USERS_SORTED.with(|n| n.set(0));
    for _ in 0..50 {
        assert!(!dag.has_one_use(shared.node));
    }
    assert_eq!(USERS_SORTED.with(std::cell::Cell::get), 0, "the list was copied and sorted");
}

/// A node morphed in place keeps its slot but reads new operands, so its id
/// is reset (`MorphNodeTo`): left small, a search for a later node it now reads
/// was pruned at it, `IsLegalToFold` found no path, and a fold made a cycle.
#[test]
fn a_node_morphed_to_read_a_later_node_is_walked_through() {
    use llrm_dagisel::dag::{Dag, SDValue as PinValue, Visited};
    use llrm_dagisel::generated::ids::mvt;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let base = input(&mut dag, 40);
    let other = input(&mut dag, 41);
    let one = dag.get_constant(1, EVT::I32);
    let a = dag.get_node(ISD::Add, EVT::I32, &[base, one]);
    let user = dag.get_node(ISD::Add, EVT::I32, &[a, one]);
    let mut z = other;
    for i in 2..12u128 {
        let k = dag.get_constant(i, EVT::I32);
        z = dag.get_node(ISD::Add, EVT::I32, &[z, k]);
    }
    let root = dag.get_node(ISD::Add, EVT::I32, &[z, user]);
    dag.set_root(root);
    let mut adapter = DagisAdapter::new(&mut dag);
    assert!(adapter.id_of(a.node) < adapter.id_of(z.node), "the premise: the morphed node is earlier");
    let morphed = adapter.morph_node_to(a.node, 1, &[mvt::i32], &[PinValue { node: z.node, res: 0 }]).0;
    assert_eq!(morphed, a.node, "the premise: reused in place");
    let mut visited = Visited::new();
    assert!(
        adapter.has_predecessor_helper(z.node, &mut visited, &mut vec![user.node], 0),
        "the morphed node was pruned from a search for what it now reads"
    );
}

/// `setcc a, b, cc` morphed to an instruction that reads only `a` and `b` left
/// the condition-code node live, and selection then refused it
/// (`CannotSelect("CondCodeNode")`: 6 of wasm's 29 IR tests).
#[test]
fn a_morphed_node_deletes_the_operands_it_stops_reading() {
    use llrm_dagisel::dag::{Dag, SDValue as PinValue};
    use llrm_dagisel::generated::ids::mvt;

    use super::isd::CondCode;
    let mut dag = SelectionDAG::new(DagConfig::default());
    let (a, b) = (input(&mut dag, 40), input(&mut dag, 41));
    let cc = dag.get_cond_code(CondCode::SetEq);
    let setcc = dag.get_node(ISD::SetCC, EVT::I1, &[a, b, cc]);
    let root = dag.get_node(ISD::Truncate, EVT::I1, &[setcc]);
    dag.set_root(root);
    let mut adapter = DagisAdapter::new(&mut dag);
    assert!(!adapter.is_dead(cc.node), "the premise: the code is live before");
    let operands = [a, b].map(|v| PinValue { node: v.node, res: 0 });
    adapter.morph_node_to(setcc.node, 1, &[mvt::i1], &operands);
    assert!(adapter.is_dead(cc.node), "the code node outlived the node that read it");
    assert!(!adapter.is_dead(a.node), "an operand still read was deleted");
}

/// A node marked selected (`setNodeId(-1)`) ends the spread of
/// `EnforceNodeIdInvariant`: a replacement invalidates the unselected users
/// above it, not everything after the selected node.
#[test]
fn a_selected_node_stops_the_spread_of_invalidated_ids() {
    use llrm_dagisel::dag::{Dag, SDValue as PinValue};
    let mut dag = SelectionDAG::new(DagConfig::default());
    let base = input(&mut dag, 40);
    let other = input(&mut dag, 41);
    let mut nodes = vec![dag.get_node(ISD::Add, EVT::I32, &[base, other])];
    for i in 0..3u128 {
        let k = dag.get_constant(i, EVT::I32);
        let last = *nodes.last().unwrap();
        nodes.push(dag.get_node(ISD::Add, EVT::I32, &[last, k]));
    }
    dag.set_root(nodes[3]);
    let mut adapter = DagisAdapter::new(&mut dag);
    adapter.mark_selected(nodes[2].node);
    adapter.replace_uses(PinValue { node: nodes[0].node, res: 0 }, PinValue { node: other.node, res: 0 });
    assert!(adapter.id_of(nodes[1].node) < -1, "the unselected user of the replacement is invalidated");
    assert!(adapter.id_of(nodes[3].node) > 0, "invalidation ran through the selected node");
}
