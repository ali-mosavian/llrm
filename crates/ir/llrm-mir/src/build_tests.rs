use crate::interpret::{Val, run};
use crate::module::{GlobalVariable, Linkage, Module, Operand};
use crate::opcode::IntPredicate;
use crate::types::Type;
use crate::{parse, print, verify};

const TEXT: &str = "@count = internal global i16 0

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define i16 @f(i16 %0) {
entry:
  %slot = alloca i16
  %tmp = alloca i16
  store i16 %0, ptr %slot
  %v = load i16, ptr %slot
  %big = icmp sgt i16 %v, 10
  br i1 %big, label %then, label %join

then:
  %m = call i16 @llvm.smax.i16(i16 %v, i16 20)
  br label %join

join:
  %r = phi i16 [ %m, %then ], [ %v, %entry ]
  store i16 %r, ptr @count
  ret i16 %r
}
";

/// What the builder makes is what the parser reads from LLVM's text.
#[test]
fn a_built_module_is_the_one_its_text_describes() {
    let mut module = Module::default();
    let i16 = module.context.types.int(16);
    let zero = module.context.int(i16, 0);
    let count = module
        .add_variable(
            "count",
            GlobalVariable { ty: i16, constant: false, initializer: Some(zero), align: None },
            Linkage::Internal,
        )
        .unwrap();
    let binary =
        module.context.types.intern(Type::Function { returns: i16, parameters: vec![i16, i16], variadic: false });
    let smax = module.add_function("llvm.smax.i16", binary, Linkage::External).unwrap();
    let unary = module.context.types.intern(Type::Function { returns: i16, parameters: vec![i16], variadic: false });
    let f = module.add_function("f", unary, Linkage::External).unwrap();
    let (count, smax) = (module.reference(count), module.reference(smax));

    let mut b = module.builder(f);
    let (entry, then, join) = (b.block("entry"), b.block("then"), b.block("join"));
    b.position(entry);
    let slot = b.alloca(i16, "slot");
    let n = b.parameter(0);
    b.store(n, slot, false);
    let v = b.load(i16, slot, false, "v");
    let ten = b.int(16, 10);
    let big = b.icmp(IntPredicate::Sgt, v, ten, "big");
    b.cond_br(big, then, join);
    b.position(then);
    b.alloca(i16, "tmp");
    let twenty = b.int(16, 20);
    let m = b.call(binary, Operand::Constant(smax), &[v, twenty], "m").unwrap();
    b.br(join);
    b.position(join);
    let r = b.phi(i16, &[(m, then), (v, entry)], "r");
    b.store(r, Operand::Constant(count), false);
    b.ret(Some(r));

    assert_eq!(verify::verify(&module), Vec::<String>::new());
    assert_eq!(print::module(&module), TEXT);
    assert_eq!(print::module(&parse::module(TEXT).unwrap()), TEXT);
    let answer = |n: u128| run(&module, "f", vec![Val::Int { bits: n, width: 16 }], 100);
    assert_eq!((answer(15), answer(3)), (Ok(Val::Int { bits: 20, width: 16 }), Ok(Val::Int { bits: 3, width: 16 })));
}

/// A body that passes wrote the name of its blocks and places once for each
/// instruction it checked (d_faces -Os: 15k strings of the 3.7 M allocations):
/// a name is text a message needs, and a body that passes has no message.
#[test]
fn a_body_that_passes_writes_no_name() {
    let module = parse::module(TEXT).unwrap();
    let before = verify::names_written();
    assert_eq!(verify::verify(&module), Vec::<String>::new());
    assert_eq!(verify::names_written() - before, 0, "names were written for a body with nothing to say");
    let wrong = parse::module("define i16 @g(i16 %a) {\nentry:\n  ret void\n}\n").unwrap();
    let before = verify::names_written();
    let said = verify::verify(&wrong);
    assert!(said.iter().any(|one| one.contains("ret in %entry")), "{said:?}");
    assert!(verify::names_written() - before >= 1, "a message with a place wrote none");
}

/// The front ends made `trunc i32 40 to i16`, `sext i16 5 to i32` and the
/// arithmetic and comparisons of two numbers as instructions, each for a later
/// pass to fold (12.8 K `algebraic` edits over the corpus, 9.2 K of them on
/// instructions the front end made). The builder folds them as LLVM's
/// `ConstantFolder` does: a number comes back, no instruction is made.
#[test]
fn the_builder_folds_what_is_made_of_numbers() {
    let mut module = Module::default();
    let (i16, i32) = (module.context.types.int(16), module.context.types.int(32));
    let unary = module.context.types.intern(Type::Function { returns: i16, parameters: vec![], variadic: false });
    let f = module.add_function("f", unary, Linkage::External).unwrap();
    let mut b = module.builder(f);
    let entry = b.block("entry");
    b.position(entry);
    let (forty, minus_five) = (b.int(32, 40), b.int(16, -5));
    let cut = b.cast(crate::opcode::CastOp::Trunc, forty, i16, "cut");
    let wide = b.cast(crate::opcode::CastOp::SExt, minus_five, i32, "wide");
    let sum = b.binary(crate::opcode::BinaryOp::Add, cut, minus_five, crate::opcode::Flags::default(), "sum");
    let less = b.icmp(IntPredicate::Slt, minus_five, cut, "less");
    b.ret(Some(sum));
    let text = print::module(&module);
    assert!(
        !text.contains("trunc") && !text.contains("sext") && !text.contains("add") && !text.contains("icmp"),
        "{text}"
    );
    assert!(text.contains("ret i16 35"), "{text}");
    let number = |m: &Module, operand: Operand| match operand {
        Operand::Constant(id) => match m.context.get(id).kind {
            crate::context::ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    };
    assert_eq!(number(&module, cut), Some(40));
    assert_eq!(number(&module, wide), Some(0xFFFF_FFFB));
    assert_eq!(number(&module, less), Some(1));
}

/// `ConstantFoldBinaryInstruction`'s special cases: an identity returns the
/// other operand, `X / 0` is poison, `X % 1` is zero, a shift by the width is
/// poison, a select on a number picks an arm.
#[test]
fn the_builder_folds_as_the_constant_folder_does_at_the_edges() {
    use crate::context::ConstantKind;
    use crate::opcode::{BinaryOp, Flags};
    let mut module = Module::default();
    let i16 = module.context.types.int(16);
    let unary = module.context.types.intern(Type::Function { returns: i16, parameters: vec![i16], variadic: false });
    let f = module.add_function("f", unary, Linkage::External).unwrap();
    let mut b = module.builder(f);
    let entry = b.block("entry");
    b.position(entry);
    let x = b.parameter(0);
    let (zero, one, seven, sixteen) = (b.int(16, 0), b.int(16, 1), b.int(16, 7), b.int(16, 16));
    let plain = Flags::default();
    // A number and an identity: the number comes back, no instruction.
    assert_eq!(b.binary(BinaryOp::Add, seven, zero, plain, "a"), seven);
    assert_eq!(b.binary(BinaryOp::Mul, one, seven, plain, "m"), seven);
    let poison_of = |b: &mut crate::build::Builder, operand: Operand| match operand {
        Operand::Constant(id) => b.context.get(id).kind == ConstantKind::Poison,
        _ => false,
    };
    let by_zero = b.binary(BinaryOp::UDiv, seven, zero, plain, "d");
    assert!(poison_of(&mut b, by_zero));
    let shifted = b.binary(BinaryOp::Shl, seven, sixteen, plain, "s");
    assert!(poison_of(&mut b, shifted));
    let rem_one = b.binary(BinaryOp::SRem, seven, one, plain, "r");
    assert_eq!(rem_one, zero);
    let yes = b.int(1, 1);
    assert_eq!(b.select(yes, seven, one, "pick"), seven);
    // A parameter is not a number: its operation is an instruction.
    let kept = b.binary(BinaryOp::Add, x, seven, plain, "kept");
    assert!(matches!(kept, Operand::Value(_)));
    b.ret(Some(kept));
}
