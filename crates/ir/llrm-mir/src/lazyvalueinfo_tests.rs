//! LazyValueInfo against LLVM's answers on `tests/lvi/*.ll`, its work on
//! doubled functions, and the boundary of each DEVIATE of `lazyvalueinfo.rs`.

use crate::datalayout::DataLayout;
use crate::lazyvalueinfo::{Env, LazyValueInfo, Steps};
use crate::lvidump::dump;
use crate::module::{GlobalKind, Operand};
use crate::parse;
use crate::valuetracking::AssumptionCache;

/// `tools/llvmport/lvi_dump.sh` of the file, pasted as `.expected`.
fn check(
    ll: &str,
    expected: &str,
) {
    let got = dump(ll).expect("parses");
    let (got, expected): (Vec<_>, Vec<_>) = (got.lines().collect(), expected.lines().collect());
    assert_eq!(got.len(), expected.len(), "lines");
    for (a, b) in got.iter().zip(&expected) {
        assert_eq!(a, b);
    }
}

macro_rules! llvm_answers {
    ($($name:ident),*) => {$(
        #[test]
        fn $name() {
            check(
                include_str!(concat!("../tests/lvi/cases/", stringify!($name), ".ll")),
                include_str!(concat!("../tests/lvi/cases/", stringify!($name), ".expected")),
            );
        }
    )*};
}

llvm_answers!(
    assume, casts, chain, cond, guard, icmp, intrinsics, loops, pointers, predicates, ranges, select, switch, uses
);

/// The work of `getConstantRange` of every result, then of `%x` at the end of
/// every block.
fn steps(text: &str) -> Steps {
    let mut module = parse::module(text).expect("parses");
    let layout = DataLayout::default();
    let at = module.globals.iter().position(|g| matches!(&g.kind, GlobalKind::Function(f) if !f.is_declaration()));
    let GlobalKind::Function(function) = &module.globals[at.expect("a function")].kind else { unreachable!() };
    let assumptions = AssumptionCache::new(&module, function);
    let mut lvi = LazyValueInfo::new();
    let globals = &module.globals;
    let GlobalKind::Function(function) = &globals[at.expect("a function")].kind else { unreachable!() };
    let mut env = Env::new(&mut module.context, globals, &module.metadata, function, &layout, &assumptions);
    let x = Operand::Value(function.parameters()[0]);
    for &bb in function.layout() {
        let terminator = function.terminator(bb).expect("a terminator");
        for inst in function.block(bb).iter() {
            if let Some(result) = function.instruction(inst).result {
                lvi.get_constant_range(&mut env, Operand::Value(result), inst, false);
            }
        }
        lvi.get_constant_range(&mut env, x, terminator, false);
    }
    lvi.get_impl().expect("an impl").steps
}

/// `levels` diamonds in a row, each merging a value bumped one way or the
/// other; `%x` is tested at the top of each.
fn diamonds(levels: usize) -> String {
    let mut text = String::from("define i32 @f(i32 %x) {\nentry:\n  br label %l0\n");
    for i in 0..levels {
        let prev = if i == 0 { "0".to_owned() } else { format!("%p{}", i - 1) };
        text += &format!(
            "l{i}:\n  %c{i} = icmp ult i32 %x, {}\n  br i1 %c{i}, label %t{i}, label %e{i}\nt{i}:\n  %a{i} = add i32 {prev}, 1\n  br label %m{i}\ne{i}:\n  %b{i} = sub i32 {prev}, 1\n  br label %m{i}\nm{i}:\n  %p{i} = phi i32 [ %a{i}, %t{i} ], [ %b{i}, %e{i} ]\n  br label %l{}\n",
            1000 - i,
            i + 1
        );
    }
    text += &format!("l{levels}:\n  ret i32 %p{}\n}}\n", levels - 1);
    text
}

/// A counting loop nest, `levels` deep in sequence.
fn loop_chain(levels: usize) -> String {
    let mut text = String::from("define i32 @f(i32 %x) {\nentry:\n  br label %h0\n");
    for i in 0..levels {
        text += &format!(
            "h{i}:\n  %i{i} = phi i32 [ 0, %{}], [ %n{i}, %h{i} ]\n  %n{i} = add nuw i32 %i{i}, 1\n  %c{i} = icmp ult i32 %n{i}, %x\n  br i1 %c{i}, label %h{i}, label %h{}\n",
            if i == 0 { "entry".to_owned() } else { format!("h{}", i - 1) },
            i + 1
        );
    }
    text += &format!("h{levels}:\n  ret i32 %x\n}}\n");
    text
}

/// Doubling a function does not more than double the pairs LVI solves: the
/// per-block cache answers a pair it has seen in O(1). (A function that is
/// quadratic in the solves would show 4x.)
#[test]
fn solves_grow_linearly_with_the_function() {
    for shape in [diamonds as fn(usize) -> String, loop_chain] {
        let small = steps(&shape(60));
        let large = steps(&shape(120));
        let ratio = large.solve as f64 / small.solve as f64;
        assert!((1.5..2.5).contains(&ratio), "{small:?} -> {large:?}: {ratio}");
        let ratio = large.solved as f64 / small.solved as f64;
        assert!((1.5..2.5).contains(&ratio), "{small:?} -> {large:?}: {ratio}");
    }
}

/// DEVIATE 1 and 2: MIR has no vectors and no `undef`.
#[test]
fn mir_has_no_vectors_or_undef() {
    assert!(
        parse::module("define <2 x i32> @f(<2 x i32> %v) {\nentry:\n  %r = insertelement <2 x i32> %v, i32 1, i32 0\n  ret <2 x i32> %r\n}\n")
            .is_err()
    );
    assert!(parse::module("define i32 @f() {\nentry:\n  ret i32 undef\n}\n").is_err());
}

/// DEVIATE 13: the `range` LLVM's tables give
/// `llvm.nvvm.read.ptx.sreg.nctaid.x` (`[1, 2^31)`) is unknown to MIR; the call
/// answers the full set.
#[test]
fn an_intrinsics_builtin_range_is_unknown() {
    let text = "declare noundef i32 @llvm.nvvm.read.ptx.sreg.nctaid.x()\ndefine i32 @f() {\nentry:\n  %i = call i32 @llvm.nvvm.read.ptx.sreg.nctaid.x()\n  ret i32 %i\n}\n";
    assert!(dump(text).expect("parses").contains("val f 0 cr=i32:full-set"));
}

/// DEVIATE 11: `ptrtoint (ptr inttoptr (i32 20 to ptr) to i32)` is `i32 20` in
/// LLVM; MIR keeps the expression, so the `and` with it is not folded to a
/// range (LLVM: `[0, 21)` here).
#[test]
fn a_constant_expression_is_not_folded() {
    let text = "define i32 @f(i32 %a) {\nentry:\n  %r = and i32 ptrtoint (ptr inttoptr (i32 20 to ptr) to i32), %a\n  ret i32 %r\n}\n";
    let lines = dump(text).expect("parses");
    assert!(lines.contains("val f 1 cr=i32:full-set"), "{lines}");
}

/// the guard note: a module that declares `llvm.experimental.guard` and calls
/// it nowhere answers as one that does not.
#[test]
fn an_unused_guard_declaration_changes_nothing() {
    let body = "define i32 @f(i32 %x) {\nentry:\n  %c = icmp ult i32 %x, 10\n  br i1 %c, label %a, label %b\na:\n  %r = add i32 %x, 1\n  ret i32 %r\nb:\n  ret i32 0\n}\n";
    let with = format!("declare void @llvm.experimental.guard(i1, ...)\n{body}");
    assert_eq!(dump(&with).expect("parses"), dump(body).expect("parses"));
}

/// `ConstantFoldCompareInstOperands` of two different non-weak globals is
/// false (`areGlobalsPotentiallyEqual`); LVI's own subset of it left `%p == @b`
/// unknown on the edge where `%p == @a`.
#[test]
fn two_different_globals_are_not_equal_on_the_edge_where_one_of_them_is() {
    use crate::context::{Constant, ConstantKind, GlobalId};
    use crate::opcode::IntPredicate;
    let mut module = parse::module(
        "@a = global i32 0
@b = global i32 0

define i1 @f(ptr %p) {
entry:
  %c = icmp eq ptr %p, @a
  br i1 %c, label %t, label %e
t:
  %d = icmp eq ptr %p, @b
  ret i1 %d
e:
  ret i1 0
}
",
    )
    .expect("parses");
    let layout = DataLayout::default();
    let at = module.globals.iter().position(|g| g.name.as_deref() == Some("f")).expect("@f");
    let b = module.globals.iter().position(|g| g.name.as_deref() == Some("b")).expect("@b");
    let ptr = module.context.types.ptr(0);
    let b = module.context.constant(Constant { ty: ptr, kind: ConstantKind::Global(GlobalId(b as u32)) });
    let GlobalKind::Function(function) = &module.globals[at].kind else { unreachable!() };
    let assumptions = AssumptionCache::new(&module, function);
    let compare =
        function.walk().map(|(_, inst)| inst).filter(|&inst| function.instruction(inst).result.is_some()).nth(1);
    let compare = compare.expect("the second compare");
    let p = Operand::Value(function.parameters()[0]);
    let mut lvi = LazyValueInfo::new();
    let globals = &module.globals;
    let GlobalKind::Function(function) = &globals[at].kind else { unreachable!() };
    let mut env = Env::new(&mut module.context, globals, &module.metadata, function, &layout, &assumptions);
    assert_eq!(lvi.get_predicate_at(&mut env, IntPredicate::Eq, p, b, compare, true), Some(false));
}
