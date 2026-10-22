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
    let mut dag = dag();
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
