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
/// the loop is `{4,+,4}` (opt's text, with the trip count it adds after).
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
    // The text up to the exit values, which the trip counts decide.
    let heads: Vec<&str> = printed.lines().map(|line| line.split("\t\t").next().expect("a line")).collect();
    assert_eq!(
        heads,
        [
            "@f",
            "%i\t{0,+,1}<nuw><nsw><%loop> U: [0,-2147483648) S: [0,-2147483648)",
            "%inc\t{1,+,1}<nuw><nsw><%loop> U: [1,-2147483648) S: [1,-2147483648)",
            "%r\t{4,+,4}<%loop> U: [0,-3) S: [-2147483648,2147483645)",
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
    let tree = llrm_mir::domtree::DomTree::of(function);
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
