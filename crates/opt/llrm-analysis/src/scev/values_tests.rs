//! The expression of each value, as the pinned `opt
//! -passes='print<scalar-evolution>'` prints it (`tools/scev/values.py` runs
//! the corpus; these are the cases that found a fault).

use super::values::print_module_values;

/// A global in the far space, read through a `getelementptr`. The pinned opt
/// prints `(6 + @g)<nuw><nsw> U: [6,-9) S: [-32762,32767)`: the object is 8
/// bytes and a global's preferred alignment is the aggregate's, 8, so its
/// address is at most 65520. `pointer_alignment` used the ABI alignment of the
/// packed struct, 1, and printed `<nuw> U: [6,-2) S: [6,0)`: no `nsw`, ranges
/// 18 values too wide. Every known-bits query on such a global lost 3 bits.
#[test]
fn test_a_global_is_aligned_to_its_types_preferred_alignment() {
    let text = "target datalayout = \"e-m:e-p:16:16-p1:32:16:16:16\"\n\
        @g = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> zeroinitializer\n\
        define ptr addrspace(1) @f() {\n\
          %p = getelementptr inbounds i8, ptr addrspace(1) @g, i16 6\n\
          ret ptr addrspace(1) %p\n\
        }\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    assert_eq!(print_module_values(&module), "@f\n%p\t(6 + @g)<nuw><nsw> U: [6,-9) S: [-32762,32767)\n");
}

/// `for (i = 0; ; i++)` with a loop-invariant bound: the PHI is the add
/// recurrence `{0,+,1}`, and the post-increment `{1,+,1}`; the `mul` after
/// the loop is `{4,+,4}` and its value after the loop, `(4 * (1 umax %n))`,
/// from the trip count (opt's text).
#[test]
fn test_a_counted_loop_phi_is_an_add_recurrence() {
    let text = "define i32 @f(i32 %n) {\n\
        entry:\n  br label %loop\n\
        loop:\n\
          %i = phi i32 [ 0, %entry ], [ %inc, %loop ]\n\
          %inc = add nuw nsw i32 %i, 1\n\
          %cmp = icmp ult i32 %inc, %n\n\
          br i1 %cmp, label %loop, label %exit\n\
        exit:\n  %r = mul i32 %inc, 4\n  ret i32 %r\n}\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let printed = print_module_values(&module);
    // Up to the `Exits:` column; the value after the loop is opt's own.
    let heads: Vec<&str> = printed.lines().map(|line| line.split("\t\t").next().expect("a line")).collect();
    assert_eq!(
        heads,
        [
            "@f",
            "%i\t{0,+,1}<nuw><nsw><%loop> U: [0,-2147483648) S: [0,-2147483648)",
            "%inc\t{1,+,1}<nuw><nsw><%loop> U: [1,-2147483648) S: [1,-2147483648)",
            "%r\t{4,+,4}<%loop> U: [0,-3) S: [-2147483648,2147483645)  -->  (4 * (1 umax %n)) U: [0,-3) S: [-2147483648,2147483645)",
        ]
    );
}

/// DEVIATE 9: the intrinsics MIR lacks. A call to one is an unknown; when MIR
/// gains one, `create_scev` must translate it.
#[test]
fn test_the_intrinsics_createscev_models_and_mir_lacks_are_not_mir_intrinsics() {
    for name in
        ["llvm.abs", "llvm.usub.sat", "llvm.uadd.sat", "llvm.start.loop.iterations", "llvm.annotation", "llvm.vscale"]
    {
        assert!(llrm_mir::intrinsics::Intrinsic::named(name).is_none(), "{name} is a MIR intrinsic now: translate it");
    }
}

/// `forgetMemoizedResults` drops the value map entries of the expression and
/// of every expression with it as an operand.
#[test]
fn test_forgetting_a_recurrence_forgets_the_values_that_use_it() {
    use llrm_mir::module::Operand;

    use super::print::Labels;
    use super::{Cx, ScalarEvolution};

    let text = "define i32 @f(i32 %n) {\n\
        entry:\n  br label %loop\n\
        loop:\n\
          %i = phi i32 [ 0, %entry ], [ %inc, %loop ]\n\
          %inc = add nuw nsw i32 %i, 1\n\
          %cmp = icmp ult i32 %inc, %n\n\
          br i1 %cmp, label %loop, label %exit\n\
        exit:\n  %r = udiv i32 %inc, 3\n  ret i32 %r\n}\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let (_, _, function) = module.functions().find(|(_, _, function)| !function.layout().is_empty()).expect("a body");
    let layout = llrm_mir::datalayout::DataLayout::default();
    let tree = llrm_mir::dominators::DominatorTree::new(function);
    let loops = llrm_mir::loopinfo::LoopInfo::analyze(function, || &tree);
    let assumptions = llrm_mir::valuetracking::AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let _ = Labels::of(function);
    let mut se = ScalarEvolution::new();
    let value = |name: &str| {
        let labels = llrm_mir::print::value_labels(function);
        let (id, _) = labels.iter().find(|(_, label)| label.as_str() == name).expect("a value");
        Operand::Value(id)
    };
    let (i, inc, r) = (value("i"), value("inc"), value("r"));
    let i_scev = se.get_scev(&cx, i);
    let inc_scev = se.get_scev(&cx, inc);
    se.get_scev(&cx, r);
    assert!(se.get_existing_scev(inc).is_some() && se.get_existing_scev(r).is_some());

    // `%inc` is the separate recurrence `{1,+,1}`, so forgetting `%i` leaves
    // it; the quotient `%r` has `%inc` as an operand, so forgetting that
    // takes `%r`.
    se.forget_memoized_results(&[i_scev]);
    assert!(se.get_existing_scev(i).is_none(), "the recurrence itself");
    assert!(se.get_existing_scev(inc).is_some() && se.get_existing_scev(r).is_some(), "not users of it");

    se.forget_memoized_results(&[inc_scev]);
    assert!(se.get_existing_scev(inc).is_none(), "the post-increment itself");
    assert!(se.get_existing_scev(r).is_none(), "the quotient that uses it");
}

/// `getSCEVAtScope` of an expression recorded its answer after computing it,
/// and computing it can forget the expression (a PHI made on the way replaces
/// its symbolic name): the record was looked up with `expect("held")` and
/// panicked in 4 QCport modules (`console`, `sc`) once `exitfold` asked for
/// exit counts. LLVM re-creates the entry (`ValuesAtScopes[V]`) and records
/// nothing. This is `console`'s `pipeline.body`, reduced.
const SCOPE_FORGOTTEN_WHILE_COMPUTED: &str = "\n\
\n\
define void @f(ptr %v1) {\n\
b1:\n\
  br label %b2\n\
\n\
b2:\n\
  %v13 = phi ptr [ %v1, %b1 ]\n\
  br label %b3\n\
\n\
b3:\n\
  %v14 = phi i16 [ 0, %b2 ], [ %v30, %b7 ]\n\
  %v15 = getelementptr inbounds i8, ptr %v13, i16 %v14\n\
  %v16 = load i8, ptr %v15\n\
  %v18 = icmp ne i8 %v16, 0\n\
  br i1 %v18, label %b5, label %b4\n\
\n\
b4:\n\
  %v20 = load i8, ptr %v15\n\
  %v22 = icmp ne i8 %v20, 0\n\
  br i1 %v22, label %b9, label %b8\n\
\n\
b5:\n\
  %v24 = load i8, ptr %v15\n\
  %v25 = sext i8 %v24 to i16\n\
  %v26 = icmp ne i16 %v25, 10\n\
  br i1 %v26, label %b6, label %b4\n\
\n\
b6:\n\
  %v28 = icmp slt i16 %v14, 38\n\
  br i1 %v28, label %b7, label %b4\n\
\n\
b7:\n\
  %v30 = add nsw i16 %v14, 1\n\
  br label %b3\n\
\n\
b8:\n\
  %v31 = phi i16 [ %v14, %b4 ], [ %v14, %b9 ], [ %v14, %b10 ], [ %v38, %b13 ], [ %v14, %b17 ]\n\
  ret void\n\
\n\
b9:\n\
  %v32 = load i8, ptr %v15\n\
  %v33 = sext i8 %v32 to i16\n\
  %v34 = icmp ne i16 %v33, 10\n\
  br i1 %v34, label %b10, label %b8\n\
\n\
b10:\n\
  %v36 = icmp eq i16 %v14, 38\n\
  br i1 %v36, label %b11, label %b8\n\
\n\
b11:\n\
  br label %b12\n\
\n\
b12:\n\
  %v38 = phi i16 [ %v14, %b11 ], [ %v48, %b15 ]\n\
  %v39 = icmp sgt i16 %v38, 0\n\
  br i1 %v39, label %b14, label %b13\n\
\n\
b13:\n\
  %v41 = icmp eq i16 %v38, 0\n\
  br i1 %v41, label %b17, label %b8\n\
\n\
b14:\n\
  %v43 = getelementptr inbounds i8, ptr %v13, i16 %v38\n\
  %v44 = load i8, ptr %v43\n\
  %v45 = sext i8 %v44 to i16\n\
  %v46 = icmp ne i16 %v45, 32\n\
  br i1 %v46, label %b15, label %b13\n\
\n\
b15:\n\
  %v48 = sub nsw i16 %v38, 1\n\
  br label %b12\n\
\n\
b17:\n\
  br label %b8\n\
}";

#[test]
fn test_an_expression_forgotten_while_its_scope_value_is_computed_is_not_recorded() {
    use super::analysis::ScevSession;
    use super::backedge::ExitCountKind;

    let module = llrm_mir::parse::module(SCOPE_FORGOTTEN_WHILE_COMPUTED).expect("parses");
    let (_, _, function) = module.functions().find(|(_, _, function)| !function.layout().is_empty()).expect("a body");
    let layout = llrm_mir::datalayout::DataLayout::default();
    let outer = llrm_mir::passes::Outer::of(&module, None);
    let mut session = ScevSession::of(&module.context, function, &outer);
    let headers: Vec<_> = {
        let tree = llrm_mir::dominators::DominatorTree::new(function);
        let loops = llrm_mir::loopinfo::LoopInfo::analyze(function, || &tree);
        loops.get_loops_in_preorder().into_iter().map(|one| one.get_header()).collect()
    };
    assert_eq!(headers.len(), 2, "premise: two loops");
    for header in headers.into_iter().rev() {
        session.ask(&module.context, &layout, function, &outer, |scev, cx| {
            scev.get_backedge_taken_count(cx, header, ExitCountKind::SymbolicMaximum);
        });
    }
}
