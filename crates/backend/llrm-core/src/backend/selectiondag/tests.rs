use super::*;

fn dag() -> SelectionDAG {
    SelectionDAG::new(DagConfig::default())
}

fn arg(dag: &mut SelectionDAG, vt: EVT, reg: u32) -> SDValue {
    dag.get_register(vt, reg)
}

/// Without folding `x + 0` stays an add node, so a later `store` of it
/// selects a needless `add`; the pin returns `x`.
#[test]
fn adding_zero_gives_the_operand() {
    let mut dag = dag();
    let x = arg(&mut dag, EVT::I32, 1);
    let zero = dag.get_constant(0, EVT::I32);
    assert_eq!(dag.get_node(ISD::Add, EVT::I32, &[x, zero]), x);
    assert_eq!(dag.get_node(ISD::Add, EVT::I32, &[zero, x]), x, "constant moves right first");
}

/// `x * 0` is the zero constant (`getNode`); `x * 1` is the combiner's, so
/// the node stays.
#[test]
fn multiplying_by_zero_folds_and_by_one_does_not() {
    let mut dag = dag();
    let x = arg(&mut dag, EVT::I32, 1);
    let zero = dag.get_constant(0, EVT::I32);
    let one = dag.get_constant(1, EVT::I32);
    assert_eq!(dag.get_node(ISD::Mul, EVT::I32, &[x, zero]), zero);
    let times_one = dag.get_node(ISD::Mul, EVT::I32, &[x, one]);
    assert_eq!(dag.opcode(times_one), ISD::Mul);
}

/// An unfolded `3 + 4` is an add of two constants, which the matcher cannot
/// select as `mov 7`.
#[test]
fn constants_fold_to_one_constant_node() {
    let mut dag = dag();
    let (three, four) = (dag.get_constant(3, EVT::I32), dag.get_constant(4, EVT::I32));
    let sum = dag.get_node(ISD::Add, EVT::I32, &[three, four]);
    assert_eq!(dag.constant(sum), Some(7));
    assert_eq!(sum, dag.get_constant(7, EVT::I32));
}

/// Building `x + y` twice gave two nodes without the CSE map, doubling the
/// instructions selected for a repeated expression.
#[test]
fn equal_requests_give_one_node() {
    let mut dag = dag();
    let (x, y) = (arg(&mut dag, EVT::I16, 1), arg(&mut dag, EVT::I16, 2));
    let first = dag.get_node(ISD::Add, EVT::I16, &[x, y]);
    let before = dag.node_count();
    let second = dag.get_node(ISD::Add, EVT::I16, &[x, y]);
    assert_eq!(first, second);
    assert_eq!(dag.node_count(), before);
    let five = dag.get_constant(5, EVT::I16);
    let right = dag.get_node(ISD::Add, EVT::I16, &[x, five]);
    assert_eq!(dag.get_node(ISD::Add, EVT::I16, &[five, x]), right, "a constant moves right");
    let other = dag.get_node(ISD::Sub, EVT::I16, &[x, y]);
    assert_ne!(first, other);
}

/// CSE keeps the flags both requests share: `add nsw` then plain `add` is
/// one node without `nsw`, or the second user would rely on a flag it never
/// had.
#[test]
fn a_shared_node_keeps_only_the_common_flags() {
    let mut dag = dag();
    let (x, y) = (arg(&mut dag, EVT::I32, 1), arg(&mut dag, EVT::I32, 2));
    let flagged = dag.get_node_flags(ISD::Add, EVT::I32, &[x, y], SDNodeFlags::NO_SIGNED_WRAP);
    let plain = dag.get_node(ISD::Add, EVT::I32, &[x, y]);
    assert_eq!(flagged, plain);
    assert!(!dag.node(plain).flags.has(SDNodeFlags::NO_SIGNED_WRAP));
}

/// A glue-typed node is never shared: two `CopyToReg`-glued chains must stay
/// apart.
#[test]
fn glue_results_are_not_shared() {
    let mut dag = dag();
    let entry = dag.entry_node();
    let a = dag.get_node_multi(ISD::CopyToReg, &[EVT::OTHER, EVT::GLUE], &[entry]);
    let b = dag.get_node_multi(ISD::CopyToReg, &[EVT::OTHER, EVT::GLUE], &[entry]);
    assert_ne!(a, b);
}

/// Folded at the node's width: a 16-bit `0xffff + 1` wrapped to 0, not 0x10000.
#[test]
fn constant_arithmetic_wraps_at_the_width() {
    let mut dag = dag();
    let (max, one) = (dag.get_constant(0xffff, EVT::I16), dag.get_constant(1, EVT::I16));
    let sum = dag.get_node(ISD::Add, EVT::I16, &[max, one]);
    assert_eq!(dag.constant(sum), Some(0));
    let four = dag.get_constant(4, EVT::I16);
    let shl = dag.get_node(ISD::Shl, EVT::I16, &[max, four]);
    assert_eq!(dag.constant(shl), Some(0xfff0));
    let sra = dag.get_node(ISD::Sra, EVT::I16, &[max, four]);
    assert_eq!(dag.constant(sra), Some(0xffff));
}

#[test]
fn division_by_zero_is_undef() {
    let mut dag = dag();
    let (x, zero) = (arg(&mut dag, EVT::I32, 1), dag.get_constant(0, EVT::I32));
    let q = dag.get_node(ISD::UDiv, EVT::I32, &[x, zero]);
    assert_eq!(dag.opcode(q), ISD::Undef);
}

#[test]
fn extensions_of_constants_and_chains() {
    let mut dag = dag();
    let minus_one = dag.get_constant(0xff, EVT::I8);
    let sext = dag.get_node(ISD::SignExtend, EVT::I32, &[minus_one]);
    assert_eq!(dag.constant(sext), Some(0xffff_ffff));
    let zext = dag.get_node(ISD::ZeroExtend, EVT::I32, &[minus_one]);
    assert_eq!(dag.constant(zext), Some(0xff));
    let x = arg(&mut dag, EVT::I8, 1);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I16, &[x]);
    let wider = dag.get_node(ISD::ZeroExtend, EVT::I32, &[wide]);
    assert_eq!(dag.operand(wider, 0), x, "zext of zext is one zext");
    let back = dag.get_node(ISD::Truncate, EVT::I8, &[wide]);
    assert_eq!(back, x, "trunc of zext to the source type");
}

#[test]
fn setcc_of_constants_and_equal_operands() {
    let mut dag = dag();
    let (a, b) = (dag.get_constant(0xff, EVT::I8), dag.get_constant(1, EVT::I8));
    let signed_lt = dag.get_setcc(EVT::I1, a, b, CondCode::SetLt);
    let unsigned_lt = dag.get_setcc(EVT::I1, a, b, CondCode::SetULt);
    assert_eq!((dag.constant(signed_lt), dag.constant(unsigned_lt)), (Some(1), Some(0)));
    let x = arg(&mut dag, EVT::I32, 1);
    let same = dag.get_setcc(EVT::I1, x, x, CondCode::SetLe);
    assert_eq!(dag.constant(same), Some(1));
    let differ = dag.get_setcc(EVT::I1, x, x, CondCode::SetNe);
    assert_eq!(dag.constant(differ), Some(0));
}

#[test]
fn select_on_a_constant_condition_and_equal_arms() {
    let mut dag = dag();
    let (x, y) = (arg(&mut dag, EVT::I32, 1), arg(&mut dag, EVT::I32, 2));
    let (t, f) = (dag.get_bool_constant(true, EVT::I1), dag.get_bool_constant(false, EVT::I1));
    assert_eq!(dag.get_select(EVT::I32, t, x, y), x);
    assert_eq!(dag.get_select(EVT::I32, f, x, y), y);
    let c = arg(&mut dag, EVT::I1, 3);
    assert_eq!(dag.get_select(EVT::I32, c, x, x), x);
}

#[test]
fn a_global_plus_a_constant_folds_into_the_symbol() {
    let mut dag = SelectionDAG::new(DagConfig { offset_folding: true, ..DagConfig::default() });
    let global = dag.get_global_address(EVT::I32, 7, 4);
    let eight = dag.get_constant(8, EVT::I32);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[global, eight]);
    assert_eq!(sum, dag.get_global_address(EVT::I32, 7, 12));
    let diff = dag.get_node(ISD::Sub, EVT::I32, &[global, eight]);
    assert_eq!(diff, dag.get_global_address(EVT::I32, 7, -4));
}

#[test]
fn condition_code_bit_tricks_match_the_pin() {
    assert_eq!(CondCode::SetLt.swapped_operands(), CondCode::SetGt);
    assert_eq!(CondCode::SetULe.swapped_operands(), CondCode::SetUGe);
    assert_eq!(CondCode::SetEq.inverse_integer(), CondCode::SetNe);
    assert_eq!(CondCode::SetLt.inverse_integer(), CondCode::SetGe);
    assert_eq!(CondCode::SetUGt.inverse_integer(), CondCode::SetULe);
    assert!(CondCode::SetLe.true_when_equal() && !CondCode::SetLt.true_when_equal());
}

#[test]
fn bit_counts_of_constants() {
    let mut dag = dag();
    let c = dag.get_constant(0b0001_0100, EVT::I8);
    let ctlz = dag.get_node(ISD::CtLz, EVT::I8, &[c]);
    let cttz = dag.get_node(ISD::CtTz, EVT::I8, &[c]);
    let ctpop = dag.get_node(ISD::CtPop, EVT::I8, &[c]);
    assert_eq!((dag.constant(ctlz), dag.constant(cttz), dag.constant(ctpop)), (Some(3), Some(2), Some(2)));
    let bswap = dag.get_constant(0x1234, EVT::I16);
    let swapped = dag.get_node(ISD::BSwap, EVT::I16, &[bswap]);
    assert_eq!(dag.constant(swapped), Some(0x3412));
}

/// Replacing a value rewrites its users; two users that become equal are one
/// node after (the pin's `AddModifiedNodeToCSEMaps`), or a later `getNode` of
/// the rewritten expression would build a second copy.
#[test]
fn replacing_a_value_rewrites_users_and_merges_equal_ones() {
    let mut dag = dag();
    let (x, y, z) = (arg(&mut dag, EVT::I32, 1), arg(&mut dag, EVT::I32, 2), arg(&mut dag, EVT::I32, 3));
    let xz = dag.get_node(ISD::Add, EVT::I32, &[x, z]);
    let yz = dag.get_node(ISD::Add, EVT::I32, &[y, z]);
    let outer_x = dag.get_node(ISD::Mul, EVT::I32, &[xz, z]);
    let outer_y = dag.get_node(ISD::Mul, EVT::I32, &[yz, z]);
    assert_eq!(dag.users(xz).collect::<Vec<_>>(), vec![outer_x.node]);
    dag.replace_all_uses_with(x, y);
    assert_eq!(dag.users(x).count(), 0);
    assert_eq!(dag.get_node(ISD::Add, EVT::I32, &[y, z]), yz, "x+z became y+z, the existing node");
    assert_eq!(dag.get_node(ISD::Mul, EVT::I32, &[yz, z]), outer_y, "and (x+z)*z the existing (y+z)*z");
    assert_eq!(dag.node(outer_x).ops[0], yz);
    assert_eq!(dag.node_ids().count(), dag.node_count());
}

mod type_actions {
    use super::*;

    fn conv(info: &TargetLoweringInfo, vt: EVT) -> (TypeAction, EVT) {
        info.type_conversion(vt)
    }

    /// An empty action table would call every type legal; `i1` and `i12` then
    /// reach the matcher as registers no instruction names.
    #[test]
    fn small_and_odd_integers_promote_and_wide_ones_expand() {
        for info in [TargetLoweringInfo::x86_32(), TargetLoweringInfo::x86_16()] {
            for vt in [EVT::I8, EVT::I16, EVT::I32] {
                assert_eq!(conv(&info, vt), (TypeAction::Legal, vt));
            }
            assert_eq!(conv(&info, EVT::I1), (TypeAction::PromoteInteger, EVT::I8));
            assert_eq!(conv(&info, EVT::Int(12)), (TypeAction::PromoteInteger, EVT::I16));
            assert_eq!(conv(&info, EVT::Int(24)), (TypeAction::PromoteInteger, EVT::I32));
            assert_eq!(conv(&info, EVT::I64), (TypeAction::ExpandInteger, EVT::I32));
            assert_eq!(conv(&info, EVT::integer(128)), (TypeAction::ExpandInteger, EVT::I64));
        }
    }

    /// An integer wider than the widest register promoted to the next power
    /// of two would loop; `i40` rounds to `i64`, which then expands.
    #[test]
    fn an_odd_wide_integer_rounds_up_then_expands() {
        let info = TargetLoweringInfo::x86_32();
        assert_eq!(conv(&info, EVT::Int(40)), (TypeAction::PromoteInteger, EVT::I64));
        assert_eq!(conv(&info, EVT::Int(256)), (TypeAction::ExpandInteger, EVT::integer(128)));
    }

    /// Floats are never softened (SD14).
    #[test]
    fn floats_are_legal() {
        let info = TargetLoweringInfo::x86_32();
        for vt in [MVT::F32, MVT::F64, MVT::F80] {
            assert!(info.is_type_legal(vt.into()));
        }
    }

    /// m16 expands the far pointer into two words; without the rule it was
    /// an unknown type.
    #[test]
    fn far_expands_to_two_words_at_m16_only() {
        assert_eq!(conv(&TargetLoweringInfo::x86_16(), MVT::Far.into()), (TypeAction::ExpandInteger, EVT::I16));
        let m32 = std::panic::catch_unwind(|| TargetLoweringInfo::x86_32().type_action(MVT::Far.into()));
        assert!(m32.is_err());
    }
}

mod action_table {
    use super::*;

    fn p5() -> TargetLowering {
        TargetLowering::x86_32(Features::default(), MVT::I32)
    }

    fn act(t: &TargetLowering, op: ISD, vt: MVT) -> Action {
        t.operation_action(op, vt.into())
    }

    /// The pin's X86 makes `select` and `setcc` Custom at every integer width
    /// on a CPU with no CMOV, and `LowerSELECT` branches; with the table empty
    /// they are Legal and a select reaches the matcher with no instruction.
    #[test]
    fn select_is_custom_without_cmov() {
        let t = p5();
        for vt in [MVT::I8, MVT::I16, MVT::I32, MVT::F32, MVT::F64, MVT::F80] {
            assert_eq!(act(&t, ISD::Select, vt), Action::Custom, "{vt:?}");
            assert_eq!(act(&t, ISD::SetCC, vt), Action::Custom, "{vt:?}");
        }
        let empty = TargetLowering::empty(TargetLoweringInfo::x86_32());
        assert_eq!(act(&empty, ISD::Select, MVT::I32), Action::Legal);
    }

    /// Wide integers: the type is expanded (B0a) and the division family is
    /// `Expand` for every width, so `SDIVREM` is the two-result form.
    #[test]
    fn i64_expands_and_division_is_expanded() {
        let t = p5();
        assert_eq!(t.types.type_action(EVT::I64), TypeAction::ExpandInteger);
        for vt in [MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
            for op in [ISD::SDiv, ISD::UDiv, ISD::SRem, ISD::URem, ISD::MulHS, ISD::MulHU] {
                assert_eq!(act(&t, op, vt), Action::Expand, "{op:?} {vt:?}");
            }
        }
        assert_eq!(act(&t, ISD::Add, MVT::I32), Action::Legal);
    }

    /// An `i8` conversion to float promotes; a table without the x87 arm left
    /// `i8 sitofp` Legal, which no instruction takes.
    #[test]
    fn byte_conversions_promote() {
        let t = p5();
        assert_eq!(act(&t, ISD::SIntToFp, MVT::I8), Action::Promote);
        assert_eq!(act(&t, ISD::UIntToFp, MVT::I16), Action::Promote);
        assert_eq!(act(&t, ISD::FpToSInt, MVT::I8), Action::Promote);
        assert_eq!(act(&t, ISD::SIntToFp, MVT::I32), Action::Custom);
        assert_eq!(act(&t, ISD::FpToUInt, MVT::I32), Action::Custom);
    }

    /// The bit counts of a CPU with no BMI, LZCNT or POPCNT: byte and word
    /// `cttz` promote to `i32`, `ctlz` and `ctpop` are Custom (the expansion
    /// `bitcounts.rs` makes today); with POPCNT the small `ctpop`s promote.
    #[test]
    fn bit_counts_follow_the_cpu() {
        let t = p5();
        assert_eq!(act(&t, ISD::CtTz, MVT::I8), Action::Promote);
        assert_eq!(t.type_to_promote_to(ISD::CtTz, MVT::I16), MVT::I32);
        assert_eq!(act(&t, ISD::CtTz, MVT::I32), Action::Custom);
        assert_eq!(act(&t, ISD::CtLz, MVT::I32), Action::Custom);
        assert_eq!(act(&t, ISD::CtPop, MVT::I32), Action::Custom);
        let pop = TargetLowering::x86_32(Features { popcnt: true, ..Features::default() }, MVT::I32);
        assert_eq!(act(&pop, ISD::CtPop, MVT::I8), Action::Promote);
        assert_eq!(pop.type_to_promote_to(ISD::CtPop, MVT::I8), MVT::I32);
    }

    /// No `br_cc` or `select_cc` node survives; `brcond` is lowered by hand;
    /// `frem` is a library call and `bswap` of a word is expanded.
    #[test]
    fn branches_and_odd_operations() {
        let t = p5();
        for vt in [MVT::F32, MVT::F64, MVT::F80, MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
            assert_eq!(act(&t, ISD::BrCC, vt), Action::Expand);
            assert_eq!(act(&t, ISD::SelectCC, vt), Action::Expand);
        }
        assert_eq!(act(&t, ISD::BrCond, MVT::Other), Action::Custom);
        assert_eq!(act(&t, ISD::BrJT, MVT::Other), Action::Expand);
        assert_eq!(act(&t, ISD::FRem, MVT::F64), Action::LibCall);
        assert_eq!(act(&t, ISD::BSwap, MVT::I16), Action::Expand);
        assert_eq!(act(&t, ISD::BSwap, MVT::I32), Action::Legal);
    }

    /// Loads, truncating stores and condition codes.
    #[test]
    fn memory_and_condition_code_actions() {
        let t = p5();
        for vt in [MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
            assert_eq!(t.load_ext_action(LoadExt::SExtLoad, vt, MVT::I1), Action::Promote);
        }
        assert_eq!(t.trunc_store_action(MVT::I32, MVT::I16), Action::Expand);
        assert_eq!(t.trunc_store_action(MVT::F64, MVT::F32), Action::Expand);
        assert_eq!(t.cond_code_action(CondCode::SetOEq, MVT::F64), Action::Expand);
        assert_eq!(t.cond_code_action(CondCode::SetOLt, MVT::F64), Action::Legal);
    }

    /// The initial defaults: an integer type with no simple form is Expand,
    /// the overflow and carry family is Expand until the constructor makes
    /// some of it Custom, and a later call overwrites an earlier one.
    #[test]
    fn defaults_and_overwriting() {
        let t = p5();
        assert_eq!(t.operation_action(ISD::Add, EVT::Int(12)), Action::Expand);
        assert_eq!(act(&t, ISD::AddC, MVT::I32), Action::Expand);
        assert_eq!(act(&t, ISD::UAddO, MVT::I32), Action::Custom);
        assert_eq!(act(&t, ISD::UAddO, MVT::I64), Action::Expand, "i64 stays expanded");
        let mut t = TargetLowering::empty(TargetLoweringInfo::x86_32());
        t.set_operation_action(ISD::Select, MVT::I32, Action::Expand);
        t.set_operation_action(ISD::Select, MVT::I32, Action::Custom);
        assert_eq!(act(&t, ISD::Select, MVT::I32), Action::Custom);
    }
}

mod builder {
    use std::collections::BTreeMap;

    use llrm_mir::datalayout::DataLayout;

    use super::super::builder::{BuilderTarget, build_block};
    use super::super::dump::normalized;
    use super::super::function_lowering::FunctionLoweringInfo;

    /// The pin's "Initial selection DAG" dumps by block name (`f:b1`).
    fn pin_blocks(text: &str) -> BTreeMap<String, String> {
        let mut blocks = BTreeMap::new();
        let mut current: Option<(String, String)> = None;
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("Initial selection DAG: ") {
                if let Some((name, body)) = current.take() {
                    blocks.insert(name, body);
                }
                let name = rest.split('\'').nth(1).expect("quoted name").to_string();
                current = Some((name, String::new()));
            } else if let Some((_, body)) = current.as_mut() {
                body.push_str(line);
                body.push('\n');
            }
        }
        blocks.extend(current);
        blocks.into_iter().map(|(name, body)| (name, normalized(&body))).collect()
    }

    /// Our dump of every block the builder takes, by the pin's block name.
    fn our_blocks(text: &str) -> (BTreeMap<String, String>, Vec<String>) {
        let text: String = text.lines().filter(|line| !line.starts_with("target triple")).map(|line| format!("{line}\n")).collect();
        let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
        let layout = module.datalayout.as_deref().map_or_else(DataLayout::default, |text| DataLayout::parse(text).expect("a datalayout"));
        let target = BuilderTarget::m32();
        let mut built = BTreeMap::new();
        let mut refused = Vec::new();
        for (id, global, function) in module.functions() {
            let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
            let mut info = FunctionLoweringInfo::new(function);
            for &block in function.layout() {
                let name = format!("{}:{}", global.name.as_deref().unwrap(), function.block(block).name.as_deref().unwrap_or(""));
                match build_block(&module, function, &layout, &target, &convention, &mut info, block) {
                    Ok(done) => {
                        built.insert(name, normalized(&done.dag.dump()));
                    }
                    Err(why) => refused.push(format!("{name}: {}", why.0)),
                }
            }
        }
        (built, refused)
    }

    /// Every block of the scalar function the builder takes has the DAG the
    /// pin builds for the same IR (ids, register numbers, memory operand text
    /// and block addresses aside). Without the builder, nothing is built.
    #[test]
    fn scalar_blocks_match_the_pins_initial_dag() {
        let ll = include_str!("golden/scalar.ll");
        let pin = pin_blocks(include_str!("golden/scalar.pin.txt"));
        let (ours, refused) = our_blocks(ll);
        assert_eq!(refused, Vec::<String>::new());
        assert_eq!(ours.len(), 4, "all four blocks are built");
        for (name, dag) in &ours {
            assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
        }
    }

    /// Entry blocks lower the arguments and `ret` copies to the return
    /// register: the same DAG as the pin's for stack arguments (`load` from
    /// `FrameIndex<-1>`, ...), an argument read in another block (`CopyToReg`
    /// of the load), and `RET_GLUE`. Without `lower_arguments` the first
    /// block is refused for its arguments.
    #[test]
    fn arguments_and_returns_match_the_pins_initial_dag() {
        let ll = include_str!("golden/args.ll");
        let pin = pin_blocks(include_str!("golden/args.pin.txt"));
        let (ours, refused) = our_blocks(ll);
        assert_eq!(refused, Vec::<String>::new());
        assert_eq!(ours.len(), 4, "the entry of f, e and n of h, the entry of v");
        for (name, dag) in &ours {
            assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
        }
    }
}

mod legalize_types {
    use super::super::legalize_types::{LegalizeError, TypeLegalizer};
    use super::*;
    use crate::backend::selectiondag::{MemBase, MemInfo};

    fn load(dag: &mut SelectionDAG, vt: EVT, slot: i32) -> SDValue {
        let ptr = dag.get_register(EVT::I32, 7);
        let chain = dag.entry_node();
        let info = MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::Stack(slot) };
        dag.get_load(vt, chain, ptr, info)
    }

    fn legalize(dag: &mut SelectionDAG, tli: &TargetLowering) -> Result<bool, LegalizeError> {
        TypeLegalizer::new(dag, tli).run()
    }

    fn all_legal(dag: &mut SelectionDAG, tli: &TargetLowering) -> bool {
        TypeLegalizer::new(dag, tli).all_legal()
    }

    /// `trunc(load i12 + load i12)` to a byte: before legalization the root
    /// reads an `i12`, which no register holds. After it the add is an `i16`
    /// add of extending loads, and the root is rewritten to read it.
    #[test]
    fn an_odd_width_add_is_promoted_and_its_user_follows() {
        let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
        let mut dag = dag();
        let i12 = EVT::Int(12);
        let (a, b) = (load(&mut dag, i12, 1), load(&mut dag, i12, 2));
        let sum = dag.get_node(ISD::Add, i12, &[a, b]);
        let byte = dag.get_node(ISD::Truncate, EVT::I8, &[sum]);
        dag.set_root(byte);
        assert!(!all_legal(&mut dag, &tli));
        assert_eq!(legalize(&mut dag, &tli), Ok(true));
        assert!(all_legal(&mut dag, &tli));
        let root = dag.get_root();
        assert_eq!(dag.opcode(root), ISD::Truncate);
        let wide = dag.operand(root, 0);
        assert_eq!((dag.opcode(wide), dag.value_type(wide)), (ISD::Add, EVT::I16));
        assert_ne!(dag.operand(wide, 0), dag.operand(wide, 1), "each operand is its own promoted load");
        for operand in [dag.operand(wide, 0), dag.operand(wide, 1)] {
            assert_eq!((dag.opcode(operand), dag.value_type(operand)), (ISD::Load, EVT::I16));
        }
    }

    /// The load's chain result stands for the promoted load's: a node that
    /// read the old chain would read a load nothing uses.
    #[test]
    fn a_promoted_loads_chain_users_follow() {
        let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
        let mut dag = dag();
        let a = load(&mut dag, EVT::Int(12), 1);
        let chain_user = dag.get_node(ISD::TokenFactor, EVT::OTHER, &[SDValue { node: a.node, res: 1 }, dag.entry_node()]);
        dag.set_root(chain_user);
        legalize(&mut dag, &tli).unwrap();
        // A token factor of one chain and the entry is that chain.
        let chain = dag.get_root();
        assert_eq!(chain.res, 1);
        assert_eq!(dag.value_type(SDValue { node: chain.node, res: 0 }), EVT::I16, "the promoted load");
    }

    /// An `i64` load truncated to a word reads the low half; the constants of
    /// an `i64` are two `i32`.
    #[test]
    fn an_i64_constant_is_two_halves_and_logic_follows() {
        let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
        let mut dag = dag();
        let k = dag.get_constant_with(0x1122_3344_5566_7788, EVT::I64, false, true);
        let word = dag.get_node(ISD::Truncate, EVT::I32, &[k]);
        dag.set_root(word);
        legalize(&mut dag, &tli).unwrap();
        assert!(all_legal(&mut dag, &tli));
        assert_eq!(dag.constant(dag.get_root()), Some(0x5566_7788), "trunc of the low half is the low half");
    }

    /// A node nothing translated handles is refused by name rather than left
    /// with an illegal type.
    #[test]
    fn an_unhandled_node_is_refused_by_name() {
        let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
        let mut dag = dag();
        let a = load(&mut dag, EVT::Int(12), 1);
        let b = load(&mut dag, EVT::Int(12), 2);
        let shifted = dag.get_node(ISD::Shl, EVT::Int(12), &[a, b]);
        let byte = dag.get_node(ISD::Truncate, EVT::I8, &[shifted]);
        dag.set_root(byte);
        let error = legalize(&mut dag, &tli).unwrap_err();
        assert!(matches!(error, LegalizeError::Unsupported { op: ISD::Shl, .. }), "{error:?}");
    }
}

mod legalize_dag {
    use super::super::legalize_dag::{DagLegalizer, Hooks, NoHooks};
    use super::super::legalize_types::LegalizeError;
    use super::*;
    use crate::backend::selectiondag::{MemBase, MemInfo};

    fn load(dag: &mut SelectionDAG, vt: EVT, slot: i32) -> SDValue {
        let ptr = dag.get_register(EVT::I32, 7);
        let chain = dag.entry_node();
        let info = MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::Stack(slot) };
        dag.get_load(vt, chain, ptr, info)
    }

    /// With `Add` on `i8` marked Promote and nothing doing it, the add
    /// reaches the matcher in a type it has no instruction for. After: a
    /// truncate of an `i16` add of any-extended operands.
    #[test]
    fn a_promoted_add_is_made_wide_and_truncated() {
        let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
        tli.set_operation_action(ISD::Add, MVT::I8, Action::Promote);
        let mut dag = dag();
        let (a, b) = (load(&mut dag, EVT::I8, 1), load(&mut dag, EVT::I8, 2));
        let sum = dag.get_node(ISD::Add, EVT::I8, &[a, b]);
        dag.set_root(sum);
        DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
        let root = dag.get_root();
        assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Truncate, EVT::I8));
        let wide = dag.operand(root, 0);
        assert_eq!((dag.opcode(wide), dag.value_type(wide)), (ISD::Add, EVT::I16));
        let (l, r) = (dag.operand(wide, 0), dag.operand(wide, 1));
        assert_eq!((dag.opcode(l), dag.opcode(r)), (ISD::AnyExtend, ISD::AnyExtend));
        assert_ne!(l, r);
    }

    /// A node the table says Expand, with no expansion translated, is refused
    /// by name and not left in the DAG.
    #[test]
    fn an_expand_with_no_case_is_refused_by_name() {
        let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
        tli.set_operation_action(ISD::Sub, MVT::I32, Action::Expand);
        let mut dag = dag();
        let (a, b) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
        let diff = dag.get_node(ISD::Sub, EVT::I32, &[a, b]);
        dag.set_root(diff);
        let error = DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap_err();
        assert!(matches!(error, LegalizeError::Unsupported { op: ISD::Sub, .. }), "{error:?}");
    }

    /// Legal nodes are left alone: no node is added.
    #[test]
    fn legal_nodes_are_untouched() {
        let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
        let mut dag = dag();
        let (a, b) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
        let sum = dag.get_node(ISD::Add, EVT::I32, &[a, b]);
        dag.set_root(sum);
        let before = dag.node_count();
        DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
        assert_eq!(dag.node_count(), before);
        assert_eq!(dag.get_root(), sum);
    }

    /// Users are legalized before their operands, so a custom lowering sees
    /// its operand as built: `mul(mul(a,b),c)` is lowered outer first and
    /// the outer still reads a `mul`. Walking operands first showed it an
    /// `add` already.
    #[test]
    fn a_custom_lowering_sees_its_operands_as_built() {
        struct MulToAdd(Vec<ISD>);
        impl Hooks for MulToAdd {
            fn lower_operation(&mut self, dag: &mut SelectionDAG, node: SDValue) -> Option<SDValue> {
                let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
                self.0.push(dag.opcode(lhs));
                Some(dag.get_node(ISD::Add, EVT::I32, &[lhs, rhs]))
            }
        }
        let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
        tli.set_operation_action(ISD::Mul, MVT::I32, Action::Custom);
        let mut dag = dag();
        let (a, b, c) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2), load(&mut dag, EVT::I32, 3));
        let inner = dag.get_node(ISD::Mul, EVT::I32, &[a, b]);
        let outer = dag.get_node(ISD::Mul, EVT::I32, &[inner, c]);
        dag.set_root(outer);
        let mut hooks = MulToAdd(Vec::new());
        DagLegalizer::new(&mut dag, &tli, &mut hooks).run().unwrap();
        assert_eq!(hooks.0, vec![ISD::Mul, ISD::Load], "outer sees the inner mul; inner sees a load");
        let root = dag.get_root();
        assert_eq!(dag.opcode(root), ISD::Add);
        assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Add, "the inner one was lowered too");
    }
}

mod expand_node {
    use super::super::legalize_dag::{DagLegalizer, NoHooks};
    use super::*;
    use crate::backend::selectiondag::{MemBase, MemInfo};

    fn load(dag: &mut SelectionDAG, vt: EVT, slot: i32) -> SDValue {
        let ptr = dag.get_register(EVT::I32, 7);
        let chain = dag.entry_node();
        let info = MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::Stack(slot) };
        dag.get_load(vt, chain, ptr, info)
    }

    fn p5() -> TargetLowering {
        TargetLowering::x86_32(Features::default(), MVT::I32)
    }

    /// `x / y` and `x % y` each expand to the two-result divide; common
    /// subexpression elimination then makes it one `idiv`. Without the
    /// two-result form a program that needs both divides twice.
    #[test]
    fn a_divide_and_a_remainder_share_one_divrem() {
        let mut dag = dag();
        let (x, y) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
        let (quotient, remainder) = (dag.get_node(ISD::SDiv, EVT::I32, &[x, y]), dag.get_node(ISD::SRem, EVT::I32, &[x, y]));
        let both = dag.get_node(ISD::Add, EVT::I32, &[quotient, remainder]);
        dag.set_root(both);
        DagLegalizer::new(&mut dag, &p5(), &mut NoHooks).run().unwrap();
        let root = dag.get_root();
        let (q, r) = (dag.operand(root, 0), dag.operand(root, 1));
        assert_eq!((dag.opcode(q), q.res, dag.opcode(r), r.res), (ISD::SDivRem, 0, ISD::SDivRem, 1));
        assert_eq!(q.node, r.node, "one divide");
    }

    /// `mulhu` is the high result of the two-result multiply.
    #[test]
    fn mulh_is_the_high_half_of_lohi() {
        let mut dag = dag();
        let (x, y) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
        let high = dag.get_node(ISD::MulHU, EVT::I32, &[x, y]);
        dag.set_root(high);
        DagLegalizer::new(&mut dag, &p5(), &mut NoHooks).run().unwrap();
        let root = dag.get_root();
        assert_eq!((dag.opcode(root), root.res), (ISD::UMulLoHi, 1));
    }

    /// With no two-result divide but a divide, `x % y` is `x - (x / y) * y`.
    #[test]
    fn a_remainder_without_divrem_is_x_minus_quotient_times_y() {
        let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
        tli.set_operation_action(ISD::URem, MVT::I32, Action::Expand);
        tli.set_operation_action(ISD::UDivRem, MVT::I32, Action::Expand);
        let mut dag = dag();
        let (x, y) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
        let rem = dag.get_node(ISD::URem, EVT::I32, &[x, y]);
        dag.set_root(rem);
        DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
        let root = dag.get_root();
        assert_eq!(dag.opcode(root), ISD::Sub);
        let product = dag.operand(root, 1);
        assert_eq!(dag.opcode(product), ISD::Mul);
        assert_eq!(dag.opcode(dag.operand(product, 0)), ISD::UDiv);
    }

    /// A word `bswap` is a rotate by 8 (the `i8` shift amount); an `i32` one is
    /// legal, so left alone.
    #[test]
    fn a_word_bswap_is_a_rotate_by_eight() {
        let mut dag = dag();
        let w = load(&mut dag, EVT::I16, 1);
        let swapped = dag.get_node(ISD::BSwap, EVT::I16, &[w]);
        dag.set_root(swapped);
        DagLegalizer::new(&mut dag, &p5(), &mut NoHooks).run().unwrap();
        let root = dag.get_root();
        assert_eq!(dag.opcode(root), ISD::Rotl);
        let amount = dag.operand(root, 1);
        assert_eq!((dag.constant(amount), dag.value_type(amount)), (Some(8), EVT::I8));
    }
}
