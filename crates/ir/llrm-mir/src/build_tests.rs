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
    let count = module.add_variable("count", GlobalVariable { ty: i16, constant: false, initializer: Some(zero), align: None }, Linkage::Internal).unwrap();
    let binary = module.context.types.intern(Type::Function { returns: i16, parameters: vec![i16, i16], variadic: false });
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
