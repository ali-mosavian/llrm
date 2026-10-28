//! The expression of each value, as the pinned `opt
//! -passes='print<scalar-evolution>'` prints it (`tools/scev/values.py` runs
//! the corpus; these are the cases that found a fault).

use super::values::print_module_values;

/// A global in the far space, read through a `getelementptr`. LLVM places a
/// global at its type's preferred alignment, so the pinned opt prints
/// `(6 + @g)<nuw><nsw> U: [6,-9) S: [-32762,32767)` (an address at most
/// 65520). llrm's backend aligns a global only as stated, so a pointer to one
/// has its ABI alignment, 1 for this packed struct: assuming 8 gave known bits
/// the global does not have (a freeze of feat/parity: d_faces miscompiled),
/// and `nsw` and ranges 18 values narrower than the code can promise.
#[test]
fn test_a_global_is_aligned_only_as_llrm_places_it() {
    let text = "target datalayout = \"e-m:e-p:16:16-p1:32:16:16:16\"\n\
        @g = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> zeroinitializer\n\
        define ptr addrspace(1) @f() {\n\
          %p = getelementptr inbounds i8, ptr addrspace(1) @g, i16 6\n\
          ret ptr addrspace(1) %p\n\
        }\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    assert_eq!(print_module_values(&module), "@f\n%p\t(6 + @g)<nuw> U: [6,-2) S: [6,0)\n");
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
    for name in ["llvm.start.loop.iterations", "llvm.annotation", "llvm.vscale"] {
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

/// `forgetValue` walks the def-use chain: forgetting the header phi forgets
/// the increment that reads it and the quotient that reads that, which
/// `forgetMemoizedResults` of the phi's expression alone leaves (the test
/// above). Before `forget_value`, `rewriteLoopExitValues` could not tell
/// ScalarEvolution that an incoming value had changed.
#[test]
fn test_forgetting_a_value_forgets_everything_computed_from_it() {
    use llrm_mir::module::Operand;

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
    let mut se = ScalarEvolution::new();
    let value = |name: &str| {
        let labels = llrm_mir::print::value_labels(function);
        let (id, _) = labels.iter().find(|(_, label)| label.as_str() == name).expect("a value");
        Operand::Value(id)
    };
    let (i, inc, r) = (value("i"), value("inc"), value("r"));
    for v in [i, inc, r] {
        se.get_scev(&cx, v);
    }
    se.forget_value(&cx, i);
    for (name, v) in [("i", i), ("inc", inc), ("r", r)] {
        assert!(se.get_existing_scev(v).is_none(), "%{name} is computed from %i");
    }
}

/// `phi [poison, %i]` where `%i` is a phi of the same block, after it: `%i`
/// does not dominate the phi (the value it holds on the back edge is the
/// previous trip's), so the phi is not `%i`. `createSCEV` took it for `%i`'s
/// recurrence and the exit value of LICM's promoted store was the counter
/// after the last trip, 8 where the last store was 7.
#[test]
fn test_a_phi_of_poison_and_a_later_phi_of_its_block_is_not_that_phi() {
    let text = "define i32 @f(i32 %n) {\n\
        entry:\n  br label %loop\n\
        loop:\n\
          %a = phi i32 [ poison, %entry ], [ %i, %loop ]\n\
          %i = phi i32 [ 0, %entry ], [ %inc, %loop ]\n\
          %inc = add nuw nsw i32 %i, 1\n\
          %cmp = icmp ult i32 %inc, %n\n\
          br i1 %cmp, label %loop, label %exit\n\
        exit:\n  ret i32 %a\n}\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let printed = print_module_values(&module);
    let a = printed.lines().find(|line| line.starts_with("%a\t")).expect("a line for %a");
    assert!(!a.contains("{0,+,1}"), "{printed}");
}

/// `llvm.abs` is `getAbsExpr` (`smax(-x, x)`, the negation `nsw` where the
/// poison flag is set), as the pin's `print<scalar-evolution>` lists it: before
/// MIR gained the intrinsic a call to it was an unknown.
#[test]
fn test_llvm_abs_is_the_smax_of_the_value_and_its_negation() {
    let text = "declare i32 @llvm.abs.i32(i32, i1)\n\
        define i32 @f(i32 %x) {\nentry:\n\
          %a = call i32 @llvm.abs.i32(i32 %x, i1 false)\n\
          %b = call i32 @llvm.abs.i32(i32 %x, i1 true)\n  ret i32 %a\n}\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let printed = print_module_values(&module);
    let heads: Vec<&str> = printed.lines().map(|line| line.split("\t\t").next().expect("a line")).collect();
    assert_eq!(
        heads,
        [
            "@f",
            "%a\t((-1 * %x) smax %x) U: full-set S: full-set",
            "%b\t((-1 * %x)<nsw> smax %x) U: full-set S: full-set",
        ]
    );
}

/// A hasher that counts what it is given.
#[derive(Default)]
struct Writes(usize);

impl std::hash::Hasher for Writes {
    fn finish(&self) -> u64 {
        0
    }

    fn write(
        &mut self,
        _: &[u8],
    ) {
        self.0 += 1;
    }
}

/// The key of a node went into the hasher a field at a time, 12 writes for
/// `a + b`: 745 K of d_faces -O2's 10.3 M for `indvars` was hashing (the pin
/// hashes a precomputed node id, 102 K). The operands are one word each now.
#[test]
fn test_the_key_of_a_sum_is_hashed_in_a_few_writes() {
    use std::hash::Hash;

    use super::expr::{Scev, Type, Use};
    let key = super::Key {
        tag: Type::Add,
        ops: std::borrow::Cow::Owned(vec![Use::of(Scev(3)), Use::of(Scev(4))]),
        ty: Some(super::expr::Ty::Int(32)),
        constant: None,
        value: None,
        header: None,
    };
    let mut writes = Writes::default();
    key.hash(&mut writes);
    assert!(writes.0 <= 4, "{} writes", writes.0);
}

/// The difference of two huge pointers was an address difference
/// (`ptrtoaddr %a - ptrtoaddr %b`), and a loop up to one was counted by it:
/// their packed bits are no addresses (a 64K step wraps the offset into the
/// selector), so tests/cc/dos/huge.c summed 268 million bytes of 100000.
#[test]
fn test_the_difference_of_huge_pointers_is_not_an_address_difference() {
    let text = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p3:32:16:16:32-i32:16-ni:2:3\"\n\
        define i32 @f(ptr addrspace(3) %a, ptr addrspace(3) %b) {\n\
          %x = ptrtoint ptr addrspace(3) %a to i32\n\
          %y = ptrtoint ptr addrspace(3) %b to i32\n\
          %d = sub i32 %x, %y\n\
          ret i32 %d\n\
        }\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let printed = print_module_values(&module);
    assert!(!printed.contains("ptrtoaddr"), "{printed}");
}

/// A constant `getelementptr` was an unknown, so IVUsers (examples/mandel -Os,
/// +23 bytes) followed a load the pin leaves alone: its offset is `(6 + @g)`
/// there.
#[test]
fn test_a_constant_gep_is_an_offset_of_its_base() {
    let text = "target datalayout = \"e-p:16:16-i16:16\"\n\
        @g = constant [16 x i8] zeroinitializer\n\
        define ptr @f(i16 %x) {\n\
          %p = getelementptr inbounds nuw i8, ptr getelementptr inbounds nuw (i8, ptr @g, i16 6), i16 %x\n\
          ret ptr %p\n\
        }\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let printed = print_module_values(&module);
    let line = printed.lines().find(|l| l.starts_with("%p")).expect("%p is printed");
    assert!(line.contains("6 + %x + @g"), "{printed}");
}

/// A pointer IV cast to an address was an unknown, so LSR of x_strlen kept the
/// pointer recurrence (`mov edx,eax; lea eax,[edx+1]`) where the pin reduced
/// the address to an index.
#[test]
fn test_the_address_of_a_pointer_recurrence_is_a_recurrence() {
    let text = "target datalayout = \"e-p:32:32-i32:32\"\n\
        @buf = global [16 x i8] zeroinitializer\n\
        define i32 @f() {\n\
        entry:\n\
          br label %loop\n\
        loop:\n\
          %p = phi ptr [ @buf, %entry ], [ %n, %loop ]\n\
          %a = ptrtoint ptr %p to i32\n\
          %n = getelementptr inbounds i8, ptr %p, i32 1\n\
          %c = load i8, ptr %p\n\
          %z = icmp eq i8 %c, 0\n\
          br i1 %z, label %out, label %loop\n\
        out:\n\
          ret i32 %a\n\
        }\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let printed = print_module_values(&module);
    let line = printed.lines().find(|l| l.contains("%a")).expect("%a is printed");
    assert!(line.contains(",+,"), "{printed}");
}

/// `usub.sat` and `uadd.sat` are MIR intrinsics now; `createSCEV` models them
/// as the pinned opt prints: `X - umin(X, Y)` and `umin(X, ~Y) + Y`, the add
/// `<nuw>`. They were unknowns (DEVIATE 9) while MIR lacked them.
#[test]
fn test_the_unsigned_saturating_intrinsics_are_modelled() {
    let text = "declare i32 @llvm.usub.sat.i32(i32, i32)\n\
        declare i32 @llvm.uadd.sat.i32(i32, i32)\n\
        define i32 @f(i32 %x, i32 %y) {\n\
          %r = call i32 @llvm.usub.sat.i32(i32 %x, i32 %y)\n\
          %s = call i32 @llvm.uadd.sat.i32(i32 %x, i32 %y)\n\
          %t = add i32 %r, %s\n\
          ret i32 %t\n}\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    let printed = print_module_values(&module);
    let expressions: Vec<&str> =
        printed.lines().skip(1).map(|line| line.split(" U:").next().expect("a line")).collect();
    assert_eq!(
        expressions[..2],
        ["%r\t((-1 * (%x umin %y)) + %x)", "%s\t(((-1 + (-1 * %y)) umin %x) + %y)<nuw>"],
        "{printed}"
    );
}
